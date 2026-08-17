#!/usr/bin/env python3
"""Name the imported symbol whose pointer lives at a guest address.

An unbound `__nl_symbol_ptr` or `__la_symbol_ptr` slot is a null pointer sitting
in `__DATA`, and guest code dereferences it without checking — so a missing
import surfaces as an opaque `MemoryError` rather than as a link error. This
turns the address back into a name.

    python dev-scripts/nl-symbol-at.py <app.ipa> <addr>
    python dev-scripts/nl-symbol-at.py <app.ipa> --all

`--all` lists every symbol-pointer slot in the binary, which is the quickest way
to see what an app imports that tapHLE may not provide.

Pair with tapHLE's own log: it already prints
`Warning: unhandled non-lazy symbol "<name>" at <addr>` for everything it could
not bind, so grepping the log for the faulting address is cheaper still. This
script is for when you have the address but not the log, or want the full list.
"""

import argparse
import struct
import sys
import zipfile

FAT_MAGIC = 0xCAFEBABE
CPU_TYPE_ARM = 12
LC_SEGMENT = 0x1
LC_SYMTAB = 0x2
LC_DYSYMTAB = 0xB
# Section types that hold indirect symbol pointers.
S_LAZY_SYMBOL_POINTERS = 0x7
S_NON_LAZY_SYMBOL_POINTERS = 0x6
INDIRECT_SYMBOL_ABS = 0x40000000
INDIRECT_SYMBOL_LOCAL = 0x80000000


def load(ipa_path, arch_subtype=9):
    z = zipfile.ZipFile(ipa_path)
    names = [
        n
        for n in z.namelist()
        if ".app/" in n and n.count("/") == 2 and "." not in n.split("/")[-1] and n.split("/")[-1]
    ]
    for name in names:
        data = z.read(name)
        if len(data) < 8:
            continue
        if struct.unpack(">I", data[:4])[0] == FAT_MAGIC:
            count = struct.unpack(">I", data[4:8])[0]
            best = None
            for i in range(count):
                ct, cst, off, size, _a = struct.unpack(">iiIII", data[8 + i * 20 : 8 + (i + 1) * 20])
                if ct != CPU_TYPE_ARM:
                    continue
                if cst == arch_subtype:
                    return name, data[off : off + size]
                if best is None:
                    best = data[off : off + size]
            if best is not None:
                return name, best
        elif struct.unpack("<I", data[:4])[0] == 0xFEEDFACE:
            return name, data
    raise SystemExit(f"no ARM Mach-O found in {ipa_path}")


def parse(macho):
    ncmds = struct.unpack("<I", macho[16:20])[0]
    off = 28
    symtab = dysymtab = None
    pointer_sections = []
    while ncmds:
        cmd, cmdsize = struct.unpack("<II", macho[off : off + 8])
        if cmd == LC_SYMTAB:
            symoff, nsyms, stroff, strsize = struct.unpack("<IIII", macho[off + 8 : off + 24])
            symtab = (symoff, nsyms, stroff, strsize)
        elif cmd == LC_DYSYMTAB:
            indirectsymoff, nindirect = struct.unpack("<II", macho[off + 56 : off + 64])
            dysymtab = (indirectsymoff, nindirect)
        elif cmd == LC_SEGMENT:
            nsects = struct.unpack("<I", macho[off + 48 : off + 52])[0]
            so = off + 56
            for _ in range(nsects):
                sectname = macho[so : so + 16].rstrip(b"\0").decode("ascii", "replace")
                segname = macho[so + 16 : so + 32].rstrip(b"\0").decode("ascii", "replace")
                addr, size = struct.unpack("<II", macho[so + 32 : so + 40])
                flags = struct.unpack("<I", macho[so + 56 : so + 60])[0]
                reserved1 = struct.unpack("<I", macho[so + 60 : so + 64])[0]
                if flags & 0xFF in (S_LAZY_SYMBOL_POINTERS, S_NON_LAZY_SYMBOL_POINTERS):
                    pointer_sections.append((segname, sectname, addr, size, reserved1))
                so += 68
        off += cmdsize
        ncmds -= 1
    return symtab, dysymtab, pointer_sections


def symbol_name(macho, symtab, sym_index):
    symoff, _nsyms, stroff, _strsize = symtab
    n_strx = struct.unpack("<I", macho[symoff + sym_index * 12 : symoff + sym_index * 12 + 4])[0]
    end = macho.index(b"\0", stroff + n_strx)
    return macho[stroff + n_strx : end].decode("ascii", "replace")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ipa")
    ap.add_argument("addr", nargs="?", help="guest address of the pointer slot, hex")
    ap.add_argument("--all", action="store_true", help="list every symbol pointer slot")
    ap.add_argument("--arch", type=int, default=9, help="cpusubtype (9 = armv7, 6 = armv6)")
    args = ap.parse_args()
    if not args.addr and not args.all:
        ap.error("give an address or --all")

    name, macho = load(args.ipa, args.arch)
    symtab, dysymtab, sections = parse(macho)
    if symtab is None or dysymtab is None:
        raise SystemExit("binary has no symbol table")
    indirectsymoff, _nindirect = dysymtab

    def slot_symbol(section, index):
        _seg, _sect, _addr, _size, reserved1 = section
        entry = indirectsymoff + (reserved1 + index) * 4
        sym_index = struct.unpack("<I", macho[entry : entry + 4])[0]
        if sym_index & (INDIRECT_SYMBOL_ABS | INDIRECT_SYMBOL_LOCAL):
            return None
        return symbol_name(macho, symtab, sym_index)

    print(f"executable: {name}")
    if args.all:
        for section in sections:
            seg, sect, addr, size, _r1 = section
            print(f"\n{seg},{sect}  {addr:#x}..{addr + size:#x}")
            for i in range(size // 4):
                nm = slot_symbol(section, i)
                if nm:
                    print(f"   {addr + i * 4:#010x}  {nm}")
        return

    target = int(args.addr, 16)
    for section in sections:
        seg, sect, addr, size, _r1 = section
        if addr <= target < addr + size:
            nm = slot_symbol(section, (target - addr) // 4)
            print(f"{target:#x} is in {seg},{sect}")
            print(f"SYMBOL: {nm or '<local or absolute — not an import>'}")
            return
    print(f"{target:#x} is not in any symbol-pointer section. Sections:")
    for seg, sect, addr, size, _r1 in sections:
        print(f"   {seg},{sect}  {addr:#x}..{addr + size:#x}")
    sys.exit(1)


if __name__ == "__main__":
    main()
