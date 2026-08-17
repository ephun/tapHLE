#!/usr/bin/env python3
"""Name the import that a guest null-dereference fault was reading.

An unbound `__nl_symbol_ptr` slot holds a null pointer, and guest code loads
through it without checking, so a missing import surfaces as an opaque
`MemoryError` at address 0. Turning that back into a name is a fixed chain:

    ldr  r5, [pc, #imm]     ; a PC-relative offset from the literal pool
    add  r5, pc             ; r5 = address of the symbol pointer slot
    ldr  ip, [r5]           ; ip = the import's value, which is null
    ldr  r2, [ip]           ; <== faults at 0

This walks that chain automatically. Rather than doing precise dataflow, it
evaluates every PC-relative address computed in a short window before the fault
and keeps the ones landing inside a symbol-pointer section — the pattern is
stereotyped enough that this is reliable, and reporting several candidates is
more useful than reporting none.

    python dev-scripts/attribute-fault.py <app.ipa> <pc> [--known sym,sym,...]

`--known` is the list of symbols tapHLE reported as unbound for this app. A
candidate appearing in it is *confirmed*: two independent routes agree that this
exact symbol had no value. Without it a candidate is only plausible.

Exit status is 0 when at least one candidate was found, 1 otherwise, so a
caller can distinguish "attributed" from "needs a human".
"""

import argparse
import json
import re
import struct
import sys
import zipfile

try:
    from capstone import CS_ARCH_ARM, CS_MODE_ARM, CS_MODE_THUMB, Cs
except ImportError:
    sys.exit("capstone is required: pip install capstone")

FAT_MAGIC = 0xCAFEBABE
CPU_TYPE_ARM = 12
LC_SEGMENT = 0x1
LC_SYMTAB = 0x2
LC_DYSYMTAB = 0xB
S_LAZY_SYMBOL_POINTERS = 0x7
S_NON_LAZY_SYMBOL_POINTERS = 0x6
INDIRECT_SYMBOL_ABS = 0x40000000
INDIRECT_SYMBOL_LOCAL = 0x80000000

# Capstone operand strings for the forms that build a PC-relative address.
LDR_LITERAL = re.compile(r"^(?P<rd>\w+), \[pc, #(?P<imm>-?0x[0-9a-f]+|-?\d+)\]$")
ADD_PC = re.compile(r"^(?P<rd>\w+), pc$")
ADD_PC_3 = re.compile(r"^(?P<rd>\w+), (?P<rn>\w+), pc$")
LDR_PC_REG = re.compile(r"^(?P<rd>\w+), \[pc, (?P<rm>\w+)\]$")
LDR_INDIRECT = re.compile(r"^(?P<rd>\w+), \[(?P<rn>\w+)\]$")


class Image:
    """One architecture slice of a Mach-O, with the lookups this needs."""

    def __init__(self, data, name):
        self.data = data
        self.name = name
        self.segments = []
        self.pointer_sections = []
        self.symtab = None
        self.indirectsymoff = None
        self._parse()

    def _parse(self):
        macho = self.data
        ncmds = struct.unpack("<I", macho[16:20])[0]
        off = 28
        for _ in range(ncmds):
            cmd, cmdsize = struct.unpack("<II", macho[off : off + 8])
            if cmd == LC_SEGMENT:
                vmaddr, vmsize, fileoff, filesize = struct.unpack(
                    "<IIII", macho[off + 24 : off + 40]
                )
                self.segments.append((vmaddr, vmsize, fileoff, filesize))
                nsects = struct.unpack("<I", macho[off + 48 : off + 52])[0]
                so = off + 56
                for _s in range(nsects):
                    addr, size = struct.unpack("<II", macho[so + 32 : so + 40])
                    flags = struct.unpack("<I", macho[so + 56 : so + 60])[0]
                    reserved1 = struct.unpack("<I", macho[so + 60 : so + 64])[0]
                    if flags & 0xFF in (
                        S_LAZY_SYMBOL_POINTERS,
                        S_NON_LAZY_SYMBOL_POINTERS,
                    ):
                        self.pointer_sections.append((addr, size, reserved1))
                    so += 68
            elif cmd == LC_SYMTAB:
                self.symtab = struct.unpack("<IIII", macho[off + 8 : off + 24])
            elif cmd == LC_DYSYMTAB:
                self.indirectsymoff = struct.unpack("<I", macho[off + 56 : off + 60])[0]
            off += cmdsize

    def file_offset(self, addr):
        for vmaddr, vmsize, fileoff, filesize in self.segments:
            if vmaddr <= addr < vmaddr + vmsize:
                pos = addr - vmaddr + fileoff
                if pos + 4 <= fileoff + filesize:
                    return pos
        return None

    def read32(self, addr):
        pos = self.file_offset(addr)
        if pos is None:
            return None
        return struct.unpack("<I", self.data[pos : pos + 4])[0]

    def symbol_at_slot(self, addr):
        """The import whose pointer lives at `addr`, if any."""
        if self.symtab is None or self.indirectsymoff is None:
            return None
        for start, size, reserved1 in self.pointer_sections:
            if not start <= addr < start + size:
                continue
            index = reserved1 + (addr - start) // 4
            entry = self.indirectsymoff + index * 4
            sym_index = struct.unpack("<I", self.data[entry : entry + 4])[0]
            if sym_index & (INDIRECT_SYMBOL_ABS | INDIRECT_SYMBOL_LOCAL):
                return None
            symoff, _nsyms, stroff, _strsize = self.symtab
            n_strx = struct.unpack(
                "<I", self.data[symoff + sym_index * 12 : symoff + sym_index * 12 + 4]
            )[0]
            end = self.data.index(b"\0", stroff + n_strx)
            return self.data[stroff + n_strx : end].decode("ascii", "replace")
        return None


def load_slices(ipa_path):
    """Every ARM slice in the bundle executable, best-guess order."""
    z = zipfile.ZipFile(ipa_path)
    for entry in z.namelist():
        parts = entry.split("/")
        if ".app/" not in entry or len(parts) != 3 or "." in parts[-1] or not parts[-1]:
            continue
        data = z.read(entry)
        if len(data) < 8:
            continue
        if struct.unpack(">I", data[:4])[0] == FAT_MAGIC:
            count = struct.unpack(">I", data[4:8])[0]
            out = []
            for i in range(count):
                ct, cst, off, size, _a = struct.unpack(
                    ">iiIII", data[8 + i * 20 : 8 + (i + 1) * 20]
                )
                if ct == CPU_TYPE_ARM:
                    out.append((cst, Image(data[off : off + size], parts[-1])))
            return out
        if struct.unpack("<I", data[:4])[0] == 0xFEEDFACE:
            return [(0, Image(data, parts[-1]))]
    return []


def candidates_for(image, pc, thumb, window=48):
    # Do not widen this hoping to attribute more faults; it was tried at 256 and
    # attributed exactly the same twelve of seventy, while adding false
    # candidates. The unattributed ones are not missing imports whose load was
    # simply out of reach — they are null pointers from somewhere else entirely
    # (an uninitialised object field, a subsystem that failed to start), and no
    # amount of looking for a symbol-pointer load will name them. Diagnosing
    # those means reading each one, which is why they are a long tail rather
    # than one shared cause.
    """PC-relative addresses computed shortly before `pc` that name an import."""
    file_pos = image.file_offset(pc)
    if file_pos is None:
        return []
    md = Cs(CS_ARCH_ARM, CS_MODE_THUMB if thumb else CS_MODE_ARM)
    step = 2 if thumb else 4

    # Same resynchronisation problem as disasm-guest-fault.py: a fixed step back
    # can land mid-instruction, so try each aligned start and keep one that
    # decodes cleanly onto the faulting address.
    instructions = None
    for back in range(window - (window % step), -1, -step):
        start = file_pos - back
        if start < 0:
            continue
        run = list(md.disasm(image.data[start : file_pos + 4], pc - back))
        if any(i.address == pc for i in run) and sum(
            i.size for i in run if i.address < pc
        ) == back:
            instructions = run
            break
    if instructions is None:
        return []

    registers = {}
    found = []

    def note(addr):
        if addr is not None and image.symbol_at_slot(addr) is not None:
            found.append(addr)

    for insn in instructions:
        mnemonic, ops = insn.mnemonic, insn.op_str
        if mnemonic.startswith("ldr"):
            m = LDR_LITERAL.match(ops)
            if m:
                imm = int(m.group("imm"), 0)
                base = (insn.address & ~3) + 4 if thumb else insn.address + 8
                value = image.read32(base + imm)
                if value is not None:
                    registers[m.group("rd")] = value
                    # Deliberately not treated as an address. These binaries are
                    # position independent, so a literal is an *offset* that only
                    # becomes an address after the following `add rd, pc`. Testing
                    # the raw offset produced confident nonsense — offsets fall in
                    # the same numeric range as the pointer sections, so one in
                    # twelve apps was blamed on _NSBuddhistCalendar.
                continue
            m = LDR_PC_REG.match(ops)
            if m and m.group("rm") in registers:
                addr = insn.address + (4 if thumb else 8) + registers[m.group("rm")]
                note(addr)
                value = image.read32(addr)
                if value is not None:
                    registers[m.group("rd")] = value
                continue
            m = LDR_INDIRECT.match(ops)
            if m and m.group("rn") in registers:
                note(registers[m.group("rn")])
                value = image.read32(registers[m.group("rn")])
                registers[m.group("rd")] = value if value is not None else 0
                continue
        elif mnemonic.startswith("add"):
            m = ADD_PC.match(ops) or ADD_PC_3.match(ops)
            if m and m.group("rd") in registers:
                addr = registers[m.group("rd")] + insn.address + (4 if thumb else 8)
                registers[m.group("rd")] = addr
                note(addr)

    # Nearest computation to the fault first: that is the one it read through.
    ordered = []
    for addr in reversed(found):
        if addr not in ordered:
            ordered.append(addr)
    return ordered


def attribute(ipa_path, pc, known=()):
    pc &= ~1
    results = []
    for _subtype, image in load_slices(ipa_path):
        for thumb in (True, False):
            for addr in candidates_for(image, pc, thumb):
                symbol = image.symbol_at_slot(addr)
                if symbol and not any(r["symbol"] == symbol for r in results):
                    results.append(
                        {
                            "symbol": symbol,
                            "slot": hex(addr),
                            "mode": "thumb" if thumb else "arm",
                            "confirmed": symbol in known,
                        }
                    )
        if results:
            break
    # Ordered by proximity to the fault, which `candidates_for` already does and
    # which is the stronger signal. Confirmation is reported but deliberately
    # does not reorder: a typical binary leaves twenty-odd imports unbound, so
    # "this symbol was also unbound" is weak corroboration on its own and will
    # happily promote a coincidence over the instruction that actually faulted.
    return results


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ipa")
    ap.add_argument("pc")
    ap.add_argument("--known", default="", help="comma-separated unbound symbols")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    known = {s.strip() for s in args.known.split(",") if s.strip()}
    results = attribute(args.ipa, int(args.pc, 16), known)
    if args.json:
        print(json.dumps(results))
    else:
        if not results:
            print("no symbol-pointer load found near the fault")
        for r in results:
            mark = "CONFIRMED" if r["confirmed"] else "candidate"
            print(f"  {mark}: {r['symbol']}  (slot {r['slot']}, {r['mode']})")
    sys.exit(0 if results else 1)


if __name__ == "__main__":
    main()
