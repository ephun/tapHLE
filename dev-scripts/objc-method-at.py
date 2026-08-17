#!/usr/bin/env python3
"""Name the Objective-C method whose implementation starts at a guest address.

tapHLE's fault dumps give you a guest PC, and `disasm-guest-fault.py` turns that
into instructions — but a listing does not tell you which method you are looking
at, and the surrounding narrative (who called it, with what) usually depends on
knowing the selector. This searches the binary's `method_t` records for one
whose `imp` matches.

    python dev-scripts/objc-method-at.py <app.ipa> <addr> [--arch 6|9]

A `method_t` in a 32-bit image is three words — name, types, imp — so every
occurrence of the address is checked as if it were the third word, and accepted
when the first word points at a plausible selector string. Walking
`__objc_classlist` properly would also give the owning class, but the selector
and type encoding are what identify a call site, and this finds them in images
whose class layout varies.
"""

import argparse
import struct
import sys
import zipfile

FAT_MAGIC = 0xCAFEBABE
CPU_TYPE_ARM = 12
LC_SEGMENT = 0x1


def load(ipa, subtype):
    z = zipfile.ZipFile(ipa)
    for n in z.namelist():
        if ".app/" in n and n.count("/") == 2 and "." not in n.split("/")[-1] and n.split("/")[-1]:
            data = z.read(n)
            if len(data) < 8:
                continue
            if struct.unpack(">I", data[:4])[0] == FAT_MAGIC:
                count = struct.unpack(">I", data[4:8])[0]
                best = None
                for i in range(count):
                    ct, cst, off, size, _a = struct.unpack(
                        ">iiIII", data[8 + i * 20 : 8 + (i + 1) * 20]
                    )
                    if ct != CPU_TYPE_ARM:
                        continue
                    if cst == subtype:
                        return n, data[off : off + size]
                    if best is None:
                        best = data[off : off + size]
                if best is not None:
                    return n, best
            elif struct.unpack("<I", data[:4])[0] == 0xFEEDFACE:
                return n, data
    raise SystemExit(f"no ARM Mach-O found in {ipa}")


def segments(macho):
    ncmds = struct.unpack("<I", macho[16:20])[0]
    off, out = 28, []
    for _ in range(ncmds):
        cmd, cmdsize = struct.unpack("<II", macho[off : off + 8])
        if cmd == LC_SEGMENT:
            name = macho[off + 8 : off + 24].rstrip(b"\0").decode("ascii", "replace")
            vmaddr, vmsize, fileoff, filesize = struct.unpack("<IIII", macho[off + 24 : off + 40])
            out.append((name, vmaddr, vmsize, fileoff, filesize))
        off += cmdsize
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ipa")
    ap.add_argument("addr")
    ap.add_argument("--arch", type=int, default=9, help="cpusubtype (9 = armv7, 6 = armv6)")
    args = ap.parse_args()

    target = int(args.addr, 16) & ~1  # low bit is the Thumb flag
    name, macho = load(args.ipa, args.arch)
    segs = segments(macho)

    def to_file(addr):
        for _n, vmaddr, vmsize, fileoff, filesize in segs:
            if vmaddr <= addr < vmaddr + vmsize:
                pos = addr - vmaddr + fileoff
                if pos < fileoff + filesize:
                    return pos
        return None

    def cstring(addr):
        pos = to_file(addr)
        if pos is None:
            return None
        end = macho.find(b"\0", pos, pos + 256)
        if end < 0:
            return None
        try:
            text = macho[pos:end].decode("ascii")
        except UnicodeDecodeError:
            return None
        return text if text and text.isprintable() else None

    print(f"executable: {name}")
    found = False
    # Both the plain address and the Thumb-flagged one appear as imps.
    wanted = {target, target | 1}
    for value in wanted:
        needle = struct.pack("<I", value)
        start = 0
        while True:
            hit = macho.find(needle, start)
            if hit < 0:
                break
            start = hit + 1
            if hit < 8:
                continue
            sel_ptr, types_ptr = struct.unpack("<II", macho[hit - 8 : hit])
            selector = cstring(sel_ptr)
            if selector is None:
                continue
            types = cstring(types_ptr) or "?"
            print(f"  SELECTOR: {selector}")
            print(f"  types   : {types}")
            print(f"  imp     : {value:#x}  (method_t at file offset {hit - 8:#x})")
            found = True
    if not found:
        print(f"  no method_t found with imp {target:#x}")
        sys.exit(1)


if __name__ == "__main__":
    main()
