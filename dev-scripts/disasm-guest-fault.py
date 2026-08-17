#!/usr/bin/env python3
"""Disassemble a guest binary around the PC of a tapHLE fault.

When the guest faults, tapHLE prints a register dump and a stack trace of guest
addresses, and that is where most investigations stop, because the addresses are
opaque without the instructions behind them. This turns them back into code.

Usage:

    python dev-scripts/disasm-guest-fault.py <app.ipa> <pc> [--before N] [--after N]
                                             [--arch armv7|armv6] [--arm]

`pc` is the value tapHLE printed, in hex, with or without `0x`. The low bit of a
guest address is the Thumb flag, so `0x30191` and `0x30190` name the same
instruction; either is accepted and Thumb is assumed unless `--arm` is given.

Example, for the fault JellyCar 1 dies on:

    python dev-scripts/disasm-guest-fault.py "tapHLE_apps/JellyCar 1.5.4 (Decrypted).ipa" 0x30190

Requires `capstone` (pip install capstone).
"""

import argparse
import struct
import sys
import zipfile

try:
    from capstone import (
        CS_ARCH_ARM,
        CS_MODE_ARM,
        CS_MODE_THUMB,
        Cs,
    )
except ImportError:
    sys.exit("capstone is required: pip install capstone")

MH_MAGIC_32 = 0xFEEDFACE
FAT_MAGIC = 0xCAFEBABE
LC_SEGMENT = 0x1
CPU_TYPE_ARM = 12
SUBTYPE = {"armv6": 6, "armv7": 9}


def find_executable(ipa_path):
    """The Mach-O inside `Payload/<Name>.app/<Name>`."""
    z = zipfile.ZipFile(ipa_path)
    candidates = [
        n
        for n in z.namelist()
        if ".app/" in n and n.count("/") == 2 and "." not in n.split("/")[-1] and n.split("/")[-1]
    ]
    for name in candidates:
        data = z.read(name)
        if len(data) >= 4 and struct.unpack("<I", data[:4])[0] in (MH_MAGIC_32,) or (
            len(data) >= 4 and struct.unpack(">I", data[:4])[0] == FAT_MAGIC
        ):
            return name, data
    raise SystemExit(f"no Mach-O executable found in {ipa_path}")


def slice_for_arch(data, arch):
    """Pick one architecture out of a fat binary, or pass a thin one through."""
    if struct.unpack(">I", data[:4])[0] != FAT_MAGIC:
        return data
    count = struct.unpack(">I", data[4:8])[0]
    wanted = SUBTYPE[arch]
    fallback = None
    for i in range(count):
        cputype, cpusubtype, offset, size, _align = struct.unpack(
            ">iiIII", data[8 + i * 20 : 8 + (i + 1) * 20]
        )
        if cputype != CPU_TYPE_ARM:
            continue
        if cpusubtype == wanted:
            return data[offset : offset + size]
        if fallback is None:
            fallback = data[offset : offset + size]
    if fallback is None:
        raise SystemExit("fat binary contains no ARM slice")
    print(f"note: no {arch} slice, using the first ARM slice instead", file=sys.stderr)
    return fallback


def segments(macho):
    """(name, vmaddr, vmsize, fileoff) for each 32-bit segment."""
    ncmds = struct.unpack("<I", macho[16:20])[0]
    out = []
    off = 28  # 32-bit mach_header
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack("<II", macho[off : off + 8])
        if cmd == LC_SEGMENT:
            name = macho[off + 8 : off + 24].rstrip(b"\0").decode("ascii", "replace")
            vmaddr, vmsize, fileoff, _filesize = struct.unpack(
                "<IIII", macho[off + 24 : off + 40]
            )
            out.append((name, vmaddr, vmsize, fileoff))
        off += cmdsize
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ipa")
    ap.add_argument("pc")
    ap.add_argument("--before", type=int, default=12, help="instructions before the PC")
    ap.add_argument("--after", type=int, default=12, help="instructions after the PC")
    ap.add_argument("--arch", choices=list(SUBTYPE), default="armv7")
    ap.add_argument("--arm", action="store_true", help="decode as ARM rather than Thumb")
    args = ap.parse_args()

    pc = int(args.pc, 16)
    thumb = not args.arm
    pc &= ~1  # the low bit is the Thumb flag, not part of the address

    name, data = find_executable(args.ipa)
    macho = slice_for_arch(data, args.arch)
    segs = segments(macho)

    hit = None
    for seg_name, vmaddr, vmsize, fileoff in segs:
        if vmaddr <= pc < vmaddr + vmsize:
            hit = (seg_name, vmaddr, fileoff)
            break
    if hit is None:
        print(f"executable: {name}")
        print(f"PC {pc:#x} is not inside any segment. Segments:")
        for seg_name, vmaddr, vmsize, fileoff in segs:
            print(f"  {seg_name:<16} vm {vmaddr:#010x}..{vmaddr + vmsize:#010x} file {fileoff:#x}")
        raise SystemExit(1)

    seg_name, vmaddr, fileoff = hit
    file_pos = pc - vmaddr + fileoff

    md = Cs(CS_ARCH_ARM, CS_MODE_THUMB if thumb else CS_MODE_ARM)
    md.detail = False

    # Thumb instructions are 2 or 4 bytes, so stepping back a fixed number of
    # bytes can land mid-instruction and produce a plausible-looking but
    # completely wrong listing. There is no way to know the correct alignment
    # from the PC alone, so try every 2-byte-aligned start in the lead-in and
    # keep the candidates that decode cleanly all the way onto the PC. The
    # earliest such start is the one that stayed in step the longest.
    step = 2 if thumb else 4
    lead = args.before * 4
    tail = macho[file_pos : file_pos + args.after * 4 + 8]

    instructions = None
    for back in range(lead - (lead % step), -1, -step):
        start_file = file_pos - back
        if start_file < 0:
            continue
        candidate = list(md.disasm(macho[start_file:file_pos] + tail, pc - back))
        # A clean run reaches the PC on an instruction boundary and has no gaps
        # before it, which is what capstone signals by stopping early.
        if any(i.address == pc for i in candidate) and sum(
            i.size for i in candidate if i.address < pc
        ) == back:
            instructions = candidate
            break

    if instructions is None:
        print("note: could not resynchronise before the PC; showing from the PC only",
              file=sys.stderr)
        instructions = list(md.disasm(tail, pc))

    print(f"executable : {name}")
    print(f"arch       : {args.arch} ({'thumb' if thumb else 'arm'})")
    print(f"segment    : {seg_name} vmaddr {vmaddr:#x} fileoff {fileoff:#x}")
    print(f"PC         : {pc:#x} -> file offset {file_pos:#x}")
    print()
    for insn in instructions:
        marker = " <== FAULT" if insn.address == pc else ""
        print(f"  {insn.address:#010x}  {insn.mnemonic:<8} {insn.op_str}{marker}")


if __name__ == "__main__":
    main()
