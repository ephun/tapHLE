#!/usr/bin/env python3
"""Read what a collection of apps *asks for*, and subtract what tapHLE provides.

`survey.py` runs every app and records where it stopped. This runs nothing. It
reads each binary's import tables directly and diffs them against the export
tables in `src/`, so the output is the backlog rather than the frontier:

    python dev-scripts/demand.py scan --apps "I:\\iOS\\ROMs\\Decrypted IPAs"
    python dev-scripts/demand.py todo
    python dev-scripts/demand.py todo --framework UIKit
    python dev-scripts/demand.py cross          # needs a survey.py catalogue too
    python dev-scripts/demand.py show _CGContextDrawImage
    python dev-scripts/demand.py app "Angry Birds"
    python dev-scripts/demand.py closest
    python dev-scripts/demand.py coverage

## Why this exists alongside the frontier survey

The two answer different questions and neither substitutes for the other.

A survey row is *the first thing that broke*, which is what to fix next but says
nothing about what is behind it. Clearing the top of that ranking reveals a new
ranking nobody could see beforehand, so the total remaining work stays unknown
until the last app runs. This measures the whole demand up front: every symbol,
class and framework the collection references, whether or not any app has
reached the code path that needs it yet.

That makes it the tool for the questions a frontier ranking cannot answer —
which framework is worth implementing next, how much of one is already done, and
which app is *closest* to running rather than merely furthest along today.

## What a count here means, and what it does not

An import is a reference, not a call. A binary that links CoreLocation and asks
for `_CLLocationCoordinate2DMake` may never execute that line, so a high count
means "widely referenced", not "widely needed". The frontier survey is what
proves a symbol actually blocks an app; this proves only that apps mention it.
Use the two together — a symbol high in both lists is unambiguous work.

Weak imports are counted separately and reported as `weak`. A weakly imported
symbol is one the app expects to be missing on older OS versions and null-checks
before use, so leaving it unimplemented is usually correct and implementing it
may even change behaviour for the worse. Do not treat weak demand as a to-do.

Selector counts are the softest number here. `__objc_selrefs` holds every
selector the binary sends, including the ones belonging to its own classes,
which this cannot distinguish from framework selectors. Ranking by *distinct
apps* is what makes the list usable: a selector sent by one app is that app's
own method, while one sent by two hundred is an API. `todo` applies
`--min-apps 3` to selectors for this reason. Class and function counts do not
have this problem — they come from the undefined-symbol table, which by
construction lists only what the binary does not define itself.

## `cross`: one priority order out of both measurements

`cross` joins this catalogue with a `survey.py` one over the same collection and
labels every gap with what the runtime actually showed:

- `BLOCKING` — some app's run stopped exactly here. Guest code demonstrably
  reaches this and cannot get past it.
- `reachable` — tapHLE tried to bind the symbol at load and had nothing to bind,
  but something else is what stopped the app. This is measured over every binary
  tapHLE loaded, including the bundled dylibs, so it can legitimately exceed the
  static demand of the app executables alone: a symbol libstdc++ imports goes
  unbound in every app that loads libstdc++. `coverage` lists that set.
- `latent` — referenced only. Nothing has reached it yet, so it is real work but
  not next work.

The `surv` column counts apps that *survived* their run and still reference the
gap: what those apps would meet next rather than now. A high `demand` with a
zero `front` is the signature of work that looks urgent statically and is in
fact behind the current frontier.

The join is conservative when the survey is older than the working tree.
Anything implemented since was already dropped from the static gap set, so stale
evidence under-reports a gap and can never invent one — treat every count as a
floor. Re-run both after a batch of fixes; the order moves, and that iteration
is the point.

## The result file is private and must never be committed

It names the apps in someone's personal collection. The default output path is
outside the repository for that reason, matching `survey.py`. Share findings —
"two hundred apps import this" — not the file.
"""

import argparse
import collections
import concurrent.futures
import json
import os
import plistlib
import re
import struct
import sys
import threading
import zipfile
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
# Outside the repository on purpose: the result lists someone's own collection.
DEFAULT_CATALOGUE = (
    Path(os.environ.get("TEMP") or os.environ.get("TMPDIR") or "/tmp")
    / "taphle-import-demand.jsonl"
)

# ---------------------------------------------------------------------------
# Mach-O, only as much as the import tables need
# ---------------------------------------------------------------------------

FAT_MAGIC = 0xCAFEBABE
MH_MAGIC = 0xFEEDFACE
CPU_TYPE_ARM = 12

LC_SEGMENT = 0x1
LC_SYMTAB = 0x2
LC_LOAD_DYLIB = 0xC
LC_LOAD_WEAK_DYLIB = 0x80000018
LC_REEXPORT_DYLIB = 0x8000001F
LC_LOAD_UPWARD_DYLIB = 0x80000023
# Ordinals in n_desc count these commands in load order, 1-based, so the set
# must match what the linker counted or every symbol is attributed to the
# wrong framework.
DYLIB_COMMANDS = (
    LC_LOAD_DYLIB,
    LC_LOAD_WEAK_DYLIB,
    LC_REEXPORT_DYLIB,
    LC_LOAD_UPWARD_DYLIB,
)

N_STAB = 0xE0
N_TYPE = 0x0E
N_UNDF = 0x00
N_PBUD = 0x0C
N_EXT = 0x01
N_WEAK_REF = 0x0040

SELF_ORDINAL = 0
DYNAMIC_LOOKUP_ORDINAL = 0xFE
EXECUTABLE_ORDINAL = 0xFF

CLASS_PREFIXES = ("_OBJC_CLASS_$_", "_OBJC_METACLASS_$_", ".objc_class_name_")


class Slice:
    """One ARM slice, parsed for its imports and the selectors it sends."""

    def __init__(self, data):
        self.data = data
        self.segments = []  # (vmaddr, vmsize, fileoff, filesize)
        self.sections = []  # (segname, sectname, addr, size, offset)
        self.dylibs = []
        self.symtab = None
        self._parse_commands()

    def _parse_commands(self):
        macho = self.data
        ncmds = struct.unpack("<I", macho[16:20])[0]
        off = 28
        for _ in range(ncmds):
            if off + 8 > len(macho):
                break
            cmd, cmdsize = struct.unpack("<II", macho[off : off + 8])
            if cmdsize < 8:
                break
            if cmd == LC_SEGMENT:
                segname = macho[off + 8 : off + 24].rstrip(b"\0").decode("ascii", "replace")
                vmaddr, vmsize, fileoff, filesize = struct.unpack(
                    "<IIII", macho[off + 24 : off + 40]
                )
                self.segments.append((vmaddr, vmsize, fileoff, filesize))
                nsects = struct.unpack("<I", macho[off + 48 : off + 52])[0]
                so = off + 56
                for _s in range(nsects):
                    if so + 68 > len(macho):
                        break
                    sectname = macho[so : so + 16].rstrip(b"\0").decode("ascii", "replace")
                    addr, size, offset = struct.unpack("<III", macho[so + 32 : so + 44])
                    self.sections.append((segname, sectname, addr, size, offset))
                    so += 68
            elif cmd == LC_SYMTAB:
                self.symtab = struct.unpack("<IIII", macho[off + 8 : off + 24])
            elif cmd in DYLIB_COMMANDS:
                name_off = struct.unpack("<I", macho[off + 8 : off + 12])[0]
                start = off + name_off
                end = macho.find(b"\0", start, off + cmdsize)
                if end < 0:
                    end = off + cmdsize
                self.dylibs.append(macho[start:end].decode("utf-8", "replace"))
            off += cmdsize

    def file_offset(self, addr):
        for vmaddr, vmsize, fileoff, filesize in self.segments:
            if vmaddr <= addr < vmaddr + vmsize:
                pos = addr - vmaddr + fileoff
                if pos < fileoff + filesize:
                    return pos
        return None

    def cstring(self, addr, limit=256):
        pos = self.file_offset(addr)
        if pos is None:
            return None
        end = self.data.find(b"\0", pos, pos + limit)
        if end < 0:
            return None
        try:
            text = self.data[pos:end].decode("ascii")
        except UnicodeDecodeError:
            return None
        return text if text and text.isprintable() else None

    def imports(self):
        """Undefined external symbols, each with its library and weak flag.

        Undefined is the whole point: a symbol the binary defines itself — its
        own classes and functions, and anything it statically linked — never
        appears here, so the result is exactly what it expects the system to
        supply.
        """
        if self.symtab is None:
            return []
        symoff, nsyms, stroff, strsize = self.symtab
        out = []
        for i in range(nsyms):
            pos = symoff + i * 12
            if pos + 12 > len(self.data):
                break
            n_strx, n_type, _n_sect, n_desc, _n_value = struct.unpack(
                "<IBBHI", self.data[pos : pos + 12]
            )
            if n_type & N_STAB:
                continue
            if (n_type & N_TYPE) not in (N_UNDF, N_PBUD):
                continue
            if not n_type & N_EXT:
                continue
            start = stroff + n_strx
            if not stroff <= start < stroff + strsize:
                continue
            end = self.data.find(b"\0", start, stroff + strsize)
            if end < 0:
                continue
            name = self.data[start:end].decode("ascii", "replace")
            if not name:
                continue
            ordinal = (n_desc >> 8) & 0xFF
            if 1 <= ordinal <= len(self.dylibs):
                library = self.dylibs[ordinal - 1]
            elif ordinal == DYNAMIC_LOOKUP_ORDINAL:
                library = "<dynamic lookup>"
            elif ordinal == EXECUTABLE_ORDINAL:
                library = "<executable>"
            else:
                # Flat namespace, which every armv6-era binary uses. The symbol
                # is real; only its attribution is unavailable.
                library = "<flat>"
            out.append((name, library, bool(n_desc & N_WEAK_REF)))
        return out

    def exports(self):
        """External symbols this image defines — what a shipped dylib supplies."""
        if self.symtab is None:
            return []
        symoff, nsyms, stroff, strsize = self.symtab
        out = []
        for i in range(nsyms):
            pos = symoff + i * 12
            if pos + 12 > len(self.data):
                break
            n_strx, n_type, _n_sect, _n_desc, _n_value = struct.unpack(
                "<IBBHI", self.data[pos : pos + 12]
            )
            if n_type & N_STAB or not n_type & N_EXT:
                continue
            if (n_type & N_TYPE) in (N_UNDF, N_PBUD):
                continue
            start = stroff + n_strx
            if not stroff <= start < stroff + strsize:
                continue
            end = self.data.find(b"\0", start, stroff + strsize)
            if end >= 0 and end > start:
                out.append(self.data[start:end].decode("ascii", "replace"))
        return out

    def selectors(self):
        """Selectors the binary sends, from either Objective-C ABI.

        The modern ABI keeps them in `__objc_selrefs`; the armv6-era one keeps
        them in `__OBJC,__message_refs`. Both are arrays of pointers to the
        selector strings, so one reader serves both.
        """
        found = set()
        for segname, sectname, addr, size, offset in self.sections:
            if sectname not in ("__objc_selrefs", "__message_refs"):
                continue
            if offset == 0 or size == 0:
                continue
            for i in range(size // 4):
                pos = offset + i * 4
                if pos + 4 > len(self.data):
                    break
                ptr = struct.unpack("<I", self.data[pos : pos + 4])[0]
                if not ptr:
                    continue
                text = self.cstring(ptr)
                if text:
                    found.add(text)
        return found


def arm_slices(data):
    """Every ARM slice in a Mach-O or fat binary."""
    if len(data) < 8:
        return []
    if struct.unpack(">I", data[:4])[0] == FAT_MAGIC:
        count = struct.unpack(">I", data[4:8])[0]
        out = []
        for i in range(count):
            head = 8 + i * 20
            if head + 20 > len(data):
                break
            cputype, _sub, off, size, _align = struct.unpack(
                ">iiIII", data[head : head + 20]
            )
            if cputype == CPU_TYPE_ARM and off + size <= len(data):
                out.append(Slice(data[off : off + size]))
        return out
    if struct.unpack("<I", data[:4])[0] == MH_MAGIC:
        cputype = struct.unpack("<i", data[4:8])[0]
        if cputype == CPU_TYPE_ARM:
            return [Slice(data)]
    return []


# ---------------------------------------------------------------------------
# Reading one app
# ---------------------------------------------------------------------------


def bundle_root(names):
    """The shallowest `*.app/` prefix, so an embedded bundle is not mistaken for
    the app itself."""
    best = None
    for name in names:
        index = name.find(".app/")
        if index < 0:
            continue
        root = name[: index + 5]
        if best is None or root.count("/") < best.count("/"):
            best = root
    return best


def read_app(ipa):
    """Identity and every ARM slice of the bundle executable.

    The executable comes from `CFBundleExecutable`, never from guessing which
    zip entry looks like a binary: bundles exist whose executable name contains
    a dot, or which ship a second Mach-O beside it.
    """
    with zipfile.ZipFile(ipa) as z:
        names = z.namelist()
        root = bundle_root(names)
        if root is None:
            raise ValueError("no .app directory in the archive")
        try:
            plist = plistlib.loads(z.read(root + "Info.plist"))
        except Exception as e:
            raise ValueError(f"unreadable Info.plist: {e}")
        if not isinstance(plist, dict):
            raise ValueError("Info.plist is not a dictionary")
        executable = plist.get("CFBundleExecutable")
        if not executable:
            raise ValueError("Info.plist has no CFBundleExecutable")
        try:
            data = z.read(root + executable)
        except KeyError:
            raise ValueError(f"executable {executable!r} is not in the archive")

    family = plist.get("UIDeviceFamily")
    if isinstance(family, list):
        family = ",".join(str(f) for f in family)
    identity = {
        "display_name": str(
            plist.get("CFBundleDisplayName") or plist.get("CFBundleName") or ""
        ),
        "bundle_identifier": str(plist.get("CFBundleIdentifier") or ""),
        "bundle_version": str(plist.get("CFBundleVersion") or ""),
        "short_version": str(plist.get("CFBundleShortVersionString") or ""),
        "minimum_os_version": str(plist.get("MinimumOSVersion") or ""),
        "device_family": str(family or ""),
    }
    return identity, arm_slices(data)


def analyse(ipa):
    """One catalogue row: everything this app asks the system for.

    Slices are unioned rather than picked between. An armv6 and an armv7 slice
    of the same app import very nearly the same set, and the difference is
    compiler detail rather than a different demand on tapHLE.
    """
    identity, slices = read_app(ipa)
    if not slices:
        raise ValueError("no ARM slice in the executable")

    dylibs = set()
    functions = {}  # name -> [library, weak]
    classes = {}  # name -> weak
    selectors = set()

    for sl in slices:
        dylibs.update(sl.dylibs)
        selectors.update(sl.selectors())
        for name, library, weak in sl.imports():
            stripped = None
            for prefix in CLASS_PREFIXES:
                if name.startswith(prefix):
                    stripped = name[len(prefix) :]
                    break
            if stripped:
                # A class imported strongly in one slice and weakly in another
                # is a strong dependency of the app as a whole.
                classes[stripped] = classes.get(stripped, True) and weak
                continue
            if name in functions:
                functions[name][1] = functions[name][1] and weak
            else:
                functions[name] = [library, weak]

    return {
        "ipa": str(ipa),
        **identity,
        "dylibs": sorted(dylibs),
        "functions": {k: v for k, v in sorted(functions.items())},
        "classes": dict(sorted(classes.items())),
        "selectors": sorted(selectors),
    }


# ---------------------------------------------------------------------------
# Reading tapHLE's export tables out of the source
# ---------------------------------------------------------------------------

FUNC_RE = re.compile(r"export_c_func!\(\s*([A-Za-z_][A-Za-z0-9_]*)\s*\(")
ALIAS_RE = re.compile(r'export_c_func_aliased!\(\s*"([^"]+)"')
CONST_RE = re.compile(r'"([^"\s]+)"\s*,\s*HostConstant::')
DYLIB_PATH_RE = re.compile(r'^\s*path:\s*"([^"]+)"', re.M)
IMPL_RE = re.compile(r"^\s*@implementation\s+([A-Za-z_][A-Za-z0-9_]*)")
METHOD_RE = re.compile(r"^\s*([+-])\s*\(")


def skip_parens(text, i):
    """Index just past the balanced parenthesised group starting at `text[i]`."""
    depth = 0
    while i < len(text):
        if text[i] == "(":
            depth += 1
        elif text[i] == ")":
            depth -= 1
            if depth == 0:
                return i + 1
        i += 1
    return i


def strip_comments(line):
    """Drop Rust comments from a method signature line.

    Not cosmetic: the codebase annotates untyped parameters with the Objective-C
    type they really are, as in `compare:(id)other // NSString*`, and when
    rustfmt has wrapped that signature the comment sits *inside* the text joined
    to find the selector. Left in, it contributes `NSString` as a name part and
    yields `compare:NSStringoptions:range:` — a selector no app will ever send,
    silently making a method look unimplemented.
    """
    line = re.sub(r"/\*.*?\*/", " ", line)
    cut = line.find("//")
    return line if cut < 0 else line[:cut]


def parse_selector(signature):
    """Build the selector from an `objc_classes!` method signature.

    `initWithFrame:(CGRect)frame andFoo:(id)f` is `initWithFrame:andFoo:`. The
    return type must already have been removed; what remains alternates name
    parts with parenthesised argument types.
    """
    # `, ...rest` is the variadic marker, and its name is not a selector part.
    # The whitespace is a real regex rather than a literal because rustfmt wraps
    # after the comma, so `stringWithFormat:(id)format, ...args` reaches here
    # with a newline's worth of indentation between the two — and matching a
    # literal ", ..." silently yielded `stringWithFormat:args`, which no app
    # sends, hiding an implemented method behind a selector that does not exist.
    cut = re.search(r",\s*\.\.\.", signature)
    if cut:
        signature = signature[: cut.start()]

    parts = []
    i = 0
    n = len(signature)
    while i < n:
        c = signature[i]
        if c == "(":
            i = skip_parens(signature, i)
            while i < n and signature[i].isspace():
                i += 1
            while i < n and (signature[i].isalnum() or signature[i] == "_"):
                i += 1
        elif c == ":":
            parts.append(":")
            i += 1
        elif c.isalnum() or c == "_":
            j = i
            while j < n and (signature[j].isalnum() or signature[j] == "_"):
                j += 1
            parts.append(signature[i:j])
            i = j
        else:
            i += 1
    return "".join(parts)


def parse_classes(text, path, classes):
    """Collect `@implementation` blocks and the selectors they define."""
    lines = text.splitlines()
    current = None
    i = 0
    while i < len(lines):
        line = lines[i]
        match = IMPL_RE.match(line)
        if match:
            current = match.group(1)
            classes.setdefault(
                current, {"file": path, "instance": set(), "class": set()}
            )
            i += 1
            continue
        if current and line.strip() == "@end":
            current = None
            i += 1
            continue
        if current and METHOD_RE.match(line):
            # rustfmt wraps long signatures, so join forward to the brace that
            # opens the body. The bound stops a false positive inside a method
            # body from swallowing the rest of the file.
            buf = strip_comments(line)
            j = i
            while "{" not in buf and j + 1 < len(lines) and j - i < 8:
                j += 1
                buf += " " + strip_comments(lines[j])
            if "{" not in buf:
                i += 1
                continue
            kind = "class" if buf.lstrip()[0] == "+" else "instance"
            signature = buf[: buf.index("{")]
            signature = signature.lstrip()[1:].lstrip()  # drop the +/-
            if not signature.startswith("("):
                i += 1
                continue
            selector = parse_selector(signature[skip_parens(signature, 0) :])
            if selector:
                classes[current][kind].add(selector)
            i = j + 1
            continue
        i += 1


# Symbols the linker itself resolves, so they are provided without appearing in
# any export table. Verified against `src/dyld.rs` rather than assumed:
# `do_non_lazy_linking` names the first two explicitly, and `setup_lazy_linking`
# rewrites every symbol stub to trap into tapHLE's own linker, which means the
# real binder is replaced wholesale and is never called. Listing them here keeps
# three entries off every app's to-do list that no one could ever action.
LINKER_PROVIDED = {
    "___CFConstantStringClassReference": "src/dyld.rs (special-cased)",
    "_OBJC_IVAR_$_NSObject.isa": "src/dyld.rs (special-cased)",
    "dyld_stub_binder": "src/dyld.rs (stubs rewritten by setup_lazy_linking)",
}


def host_coverage():
    """What `src/` exports: symbols, classes, selectors, and provided dylibs.

    Parsed from the source rather than from a built binary so this works without
    a toolchain, and so every entry keeps the file it came from — which is the
    difference between "NSScanner is missing" and "add it next to the file that
    already has NSString".
    """
    symbols = dict(LINKER_PROVIDED)  # symbol -> file
    classes = {}  # class -> {file, instance, class}
    dylibs = {}  # provided path -> file

    for path in sorted((REPO / "src").rglob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="replace")
        rel = str(path.relative_to(REPO)).replace("\\", "/")
        for name in FUNC_RE.findall(text):
            symbols.setdefault("_" + name, rel)
        for alias in ALIAS_RE.findall(text):
            symbols.setdefault("_" + alias, rel)
        for constant in CONST_RE.findall(text):
            symbols.setdefault(constant, rel)
        if "@implementation" in text:
            parse_classes(text, rel, classes)
        if "dyld.rs" not in rel:  # its doc comment shows the example path
            for provided in DYLIB_PATH_RE.findall(text):
                dylibs.setdefault(provided, rel)

    # Not everything tapHLE provides is emulated. It also ships real armv6/armv7
    # dylibs in tapHLE_dylibs/ — zlib, sqlite3, libxml2, and the C++ and GCC
    # runtimes — which the guest links normally. Their exports are supplied just
    # as surely as a host function is, and omitting them would put every
    # `_sqlite3_*` and C++ ABI symbol on the to-do list as work already done.
    # A bundled dylib is a guest binary like any other, so it has imports of its
    # own that tapHLE must satisfy. They are collected here and filtered below,
    # once every export is known.
    dylib_imports = {}  # symbol -> dylib that needs it
    for path in sorted((REPO / paths_dylibs_dir()).glob("*.dylib")):
        rel = f"{paths_dylibs_dir()}/{path.name}"
        try:
            data = path.read_bytes()
        except OSError:
            continue
        for sl in arm_slices(data):
            for symbol in sl.exports():
                symbols.setdefault(symbol, rel)
            for symbol, _library, weak in sl.imports():
                if not weak:
                    dylib_imports.setdefault(symbol, path.name)
        dylibs.setdefault("/usr/lib/" + path.name, rel)

    # These are invisible to a scan of app executables, and they are unresolved
    # for *every* app that loads the dylib rather than for the few that import
    # them directly — which is why a symbol can show far more runtime evidence
    # than static demand. `___mb_cur_max` is imported by thirteen executables in
    # one collection and went unbound in seven hundred and forty-four apps.
    dylib_needs = {s: lib for s, lib in dylib_imports.items() if s not in symbols}

    selectors = set()
    for entry in classes.values():
        selectors |= entry["instance"] | entry["class"]
    return {
        "symbols": symbols,
        "classes": classes,
        "selectors": selectors,
        "dylibs": dylibs,
        "dylib_needs": dylib_needs,
    }


def paths_dylibs_dir():
    """The directory name from `src/paths.rs`, so the two cannot drift apart."""
    text = (REPO / "src" / "paths.rs").read_text(encoding="utf-8", errors="replace")
    match = re.search(r'DYLIBS_DIR:\s*&str\s*=\s*"([^"]+)"', text)
    return match.group(1) if match else "tapHLE_dylibs"


def framework_of(dylib_path):
    """`.../UIKit.framework/UIKit` -> `UIKit`; `/usr/lib/libz.1.dylib` -> `libz`."""
    if ".framework/" in dylib_path:
        return dylib_path.split(".framework/")[0].rsplit("/", 1)[-1]
    base = dylib_path.rsplit("/", 1)[-1]
    return base.split(".")[0] or dylib_path


# ---------------------------------------------------------------------------
# Catalogue
# ---------------------------------------------------------------------------


def load_catalogue(path):
    rows = []
    if path.exists():
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    rows.append(json.loads(line))
    return rows


def app_key(row):
    """One identity per app, so eight versions of one game count once.

    Ranking by file would let a well-represented app outrank a cause that blocks
    many different ones — the same reason `survey.py` groups this way.
    """
    return row.get("bundle_identifier") or row["ipa"]


def cmd_scan(args):
    catalogue = Path(args.catalogue)
    catalogue.parent.mkdir(parents=True, exist_ok=True)
    done = {row["ipa"] for row in load_catalogue(catalogue)}

    apps = sorted(p.resolve() for p in Path(args.apps).rglob("*.ipa") if p.is_file())
    if args.limit:
        apps = apps[: args.limit]
    todo = [p for p in apps if str(p) not in done]
    print(f"{len(apps)} IPAs found, {len(todo)} to read ({len(done)} already done)")

    lock = threading.Lock()
    counter = {"n": 0, "failed": 0}

    def work(ipa):
        try:
            row = analyse(ipa)
        except Exception as e:
            row = {"ipa": str(ipa), "error": f"{type(e).__name__}: {e}"}
        with lock:
            counter["n"] += 1
            with open(catalogue, "a", encoding="utf-8") as f:
                f.write(json.dumps(row) + "\n")
            if "error" in row:
                counter["failed"] += 1
                print(f"[{counter['n']}/{len(todo)}] {ipa.name}: {row['error']}", flush=True)
            elif args.verbose:
                name = row.get("display_name") or ipa.stem
                print(
                    f"[{counter['n']}/{len(todo)}] {name}: "
                    f"{len(row['functions'])} symbols, {len(row['classes'])} classes, "
                    f"{len(row['selectors'])} selectors",
                    flush=True,
                )
            elif counter["n"] % 25 == 0:
                print(f"  {counter['n']}/{len(todo)}", flush=True)

    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as pool:
        list(pool.map(work, todo))
    print(f"done: {counter['n'] - counter['failed']} read, {counter['failed']} failed")


def cmd_coverage(args):
    """What the source exports. Run this after changing the export macros."""
    host = host_coverage()
    print(f"{len(host['symbols'])} exported symbols (functions and constants)")
    print(f"{len(host['classes'])} Objective-C classes")
    print(f"{len(host['selectors'])} distinct selectors")
    print(f"{len(host['dylibs'])} provided dylibs:")
    for path, rel in sorted(host["dylibs"].items()):
        print(f"    {framework_of(path):24s} {rel}")

    needs = host["dylib_needs"]
    if needs:
        print(
            f"\n{len(needs)} symbols the bundled dylibs import and nothing "
            "provides.\nThese are unresolved for every app that loads the "
            "dylib, not only for apps\nthat import them directly:\n"
        )
        by_lib = collections.defaultdict(list)
        for symbol, lib in needs.items():
            by_lib[lib].append(symbol)
        for lib in sorted(by_lib):
            print(f"    {lib}")
            for symbol in sorted(by_lib[lib]):
                print(f"        {symbol}")
    if args.classes:
        print()
        for name, entry in sorted(host["classes"].items()):
            total = len(entry["instance"]) + len(entry["class"])
            print(f"  {name:36s} {total:4d} methods   {entry['file']}")


def gaps(rows, host, include_weak=False):
    """Per-category demand that tapHLE does not satisfy, counted by distinct app."""
    functions = collections.defaultdict(set)
    function_library = {}
    classes = collections.defaultdict(set)
    selectors = collections.defaultdict(set)
    frameworks = collections.defaultdict(set)
    provided = {framework_of(p) for p in host["dylibs"]}

    for row in rows:
        if "error" in row:
            continue
        key = app_key(row)
        for symbol, (library, weak) in row["functions"].items():
            if weak and not include_weak:
                continue
            if symbol in host["symbols"]:
                continue
            functions[symbol].add(key)
            function_library.setdefault(symbol, framework_of(library))
        for name, weak in row["classes"].items():
            if weak and not include_weak:
                continue
            if name in host["classes"]:
                continue
            classes[name].add(key)
        for selector in row["selectors"]:
            if selector in host["selectors"]:
                continue
            selectors[selector].add(key)
        for dylib in row["dylibs"]:
            framework = framework_of(dylib)
            if framework not in provided:
                frameworks[framework].add(key)

    return {
        "functions": functions,
        "function_library": function_library,
        "classes": classes,
        "selectors": selectors,
        "frameworks": frameworks,
    }


def cmd_todo(args):
    rows = load_catalogue(Path(args.catalogue))
    good = [r for r in rows if "error" not in r]
    if not good:
        sys.exit("catalogue is empty; run `demand.py scan --apps ...` first")
    host = host_coverage()
    result = gaps(good, host, include_weak=args.include_weak)
    distinct = len({app_key(r) for r in good})

    if args.framework:
        wanted = args.framework.lower()
        keep = {
            s
            for s, lib in result["function_library"].items()
            if lib.lower() == wanted
        }
        result["functions"] = {
            s: a for s, a in result["functions"].items() if s in keep
        }

    def dump(title, mapping, minimum, annotate=None):
        ranked = sorted(mapping.items(), key=lambda kv: (-len(kv[1]), kv[0]))
        ranked = [(k, a) for k, a in ranked if len(a) >= minimum]
        print(f"\n{title} ({len(ranked)})\n")
        for key, apps in ranked[: args.top]:
            extra = f"   {annotate[key]}" if annotate and key in annotate else ""
            print(f"{len(apps):5d}  {key}{extra}")

    kind = "including weak imports" if args.include_weak else "strong imports only"
    print(
        f"{distinct} distinct apps ({len(good)} files, {len(rows) - len(good)} "
        f"unreadable) - {kind}"
    )
    if not args.framework:
        dump(
            "FRAMEWORKS linked but not provided by tapHLE, by apps linking them",
            result["frameworks"],
            args.min_apps,
        )
        dump(
            "CLASSES referenced but not implemented, by apps referencing them",
            result["classes"],
            args.min_apps,
        )
    dump(
        "SYMBOLS imported but not exported, by apps importing them",
        result["functions"],
        args.min_apps,
        annotate=result["function_library"],
    )
    if not args.framework:
        dump(
            "SELECTORS sent but implemented on no tapHLE class, by apps sending "
            "them (see the caveat in this script's docstring)",
            result["selectors"],
            max(args.min_apps, 3),
        )


DEFAULT_SURVEY = (
    Path(os.environ.get("TEMP") or os.environ.get("TMPDIR") or "/tmp")
    / "taphle-frontier-survey.jsonl"
)


def survey_evidence(survey_rows):
    """Turn frontier rows into per-gap runtime evidence.

    Three signals, in descending order of strength:

    - `frontier` — an app's run stopped exactly here. Proof that guest code
      reaches this and cannot proceed without it.
    - `unbound` — tapHLE tried to bind this symbol at load and had nothing to
      bind. Proof it is genuinely absent at runtime, though something else may
      be what actually stopped the app.
    - `survivors` — the app got through its run and still references this, so
      this is what it would meet next rather than now.
    """
    evidence = {
        "frontier_symbol": collections.defaultdict(set),
        "frontier_class": collections.defaultdict(set),
        "frontier_selector": collections.defaultdict(set),
        "unbound": collections.defaultdict(set),
        "survived": set(),
        "broken": set(),
    }
    for row in survey_rows:
        key = app_key(row)
        reason = row.get("reason_key") or ""
        if reason == "ok:survived":
            evidence["survived"].add(key)
        else:
            evidence["broken"].add(key)
        for symbol in row.get("missing_symbols") or []:
            evidence["unbound"][symbol].add(key)

        head, _, rest = reason.partition(":")
        if head == "function":
            evidence["frontier_symbol"][rest].add(key)
        elif head == "class-missing":
            evidence["frontier_class"][rest].add(key)
        elif head == "class-stub":
            # "class-stub:NSPersistentStoreCoordinator.alloc" — the class is a
            # placeholder, so the class is the gap, not the method.
            evidence["frontier_class"][rest.partition(".")[0]].add(key)
        elif head == "selector":
            # "selector:NSURLCache.initWith...:" — the class exists and answered
            # to something; the method is what is missing.
            cls, _, sel = rest.partition(".")
            if sel:
                evidence["frontier_selector"][sel].add(key)
    return evidence


def cmd_cross(args):
    """Rank static gaps by whether the runtime survey saw them block an app.

    Neither list can produce this order alone. `todo` knows what the collection
    references but not whether anything reaches it; `survey.py rank` knows what
    stopped each app but not what waits behind it. Joining them separates a gap
    that is *proven* to block from one that is merely widely referenced.

    The join is conservative when the survey is older than the working tree.
    Anything implemented since was already dropped from the static gap set, so
    stale evidence can only under-report a gap, never invent one — the verdicts
    are a floor, not an estimate.
    """
    demand_rows = [r for r in load_catalogue(Path(args.catalogue)) if "error" not in r]
    if not demand_rows:
        sys.exit("import catalogue is empty; run `demand.py scan --apps ...` first")
    survey_path = Path(args.survey)
    if not survey_path.exists():
        sys.exit(
            f"no frontier survey at {survey_path}. Run `python dev-scripts/survey.py "
            "run --apps ...` first, or pass --survey with its path."
        )
    survey_rows = load_catalogue(survey_path)
    host = host_coverage()
    result = gaps(demand_rows, host, include_weak=args.include_weak)
    evidence = survey_evidence(survey_rows)

    demand_apps = {app_key(r) for r in demand_rows}
    survey_apps = {app_key(r) for r in survey_rows}
    shared = demand_apps & survey_apps
    revisions = sorted({r.get("taphle_version", "?") for r in survey_rows})
    print(
        f"{len(demand_apps)} apps scanned, {len(survey_apps)} surveyed, "
        f"{len(shared)} in both. Survey revision(s): {', '.join(revisions)}."
    )
    if len(shared) < len(demand_apps):
        print(
            f"  note: {len(demand_apps) - len(shared)} scanned apps were never "
            "surveyed, so they contribute demand but no runtime evidence."
        )
    print(
        "  Evidence is a floor: a survey older than the working tree under-"
        "reports, never over-reports.\n"
    )

    def rank(mapping, frontier, unbound=None):
        rows = []
        for name, apps in mapping.items():
            f = len(frontier.get(name, ()) )
            u = len(unbound.get(name, ())) if unbound is not None else 0
            s = len(apps & evidence["survived"])
            if f:
                tier, verdict = 0, "BLOCKING"
            elif u:
                tier, verdict = 1, "reachable"
            else:
                tier, verdict = 2, "latent"
            rows.append((tier, len(apps), f, u, s, verdict, name))
        if args.sort == "demand":
            rows.sort(key=lambda r: (-r[1], r[6]))
        elif args.sort == "frontier":
            rows.sort(key=lambda r: (-r[2], -r[1], r[6]))
        else:
            rows.sort(key=lambda r: (r[0], -r[2], -r[3], -r[1], r[6]))
        return rows

    def dump(title, rows, show_unbound):
        blocking = sum(1 for r in rows if r[0] == 0)
        reachable = sum(1 for r in rows if r[0] == 1)
        print(
            f"{title} - {len(rows)} gaps: {blocking} blocking, {reachable} "
            f"reachable, {len(rows) - blocking - reachable} latent\n"
        )
        if show_unbound:
            print(f"  {'demand':>7} {'front':>6} {'unbnd':>6} {'surv':>5}  verdict     name")
        else:
            print(f"  {'demand':>7} {'front':>6} {'surv':>5}  verdict     name")
        for tier, demand, f, u, s, verdict, name in rows[: args.top]:
            if show_unbound:
                print(f"  {demand:7d} {f:6d} {u:6d} {s:5d}  {verdict:10s}  {name}")
            else:
                print(f"  {demand:7d} {f:6d} {s:5d}  {verdict:10s}  {name}")
        print()

    symbol_rows = rank(
        result["functions"], evidence["frontier_symbol"], evidence["unbound"]
    )
    class_rows = rank(result["classes"], evidence["frontier_class"])
    selector_rows = rank(result["selectors"], evidence["frontier_selector"])

    dump("SYMBOLS", symbol_rows, True)
    dump("CLASSES", class_rows, False)
    if not args.no_selectors:
        dump(
            "SELECTORS (softest signal - see this script's docstring)",
            [r for r in selector_rows if r[0] < 2 or r[1] >= 3],
            False,
        )

    # How much of the frontier this static list actually accounts for. A low
    # number is not a defect: most apps stop on a panic or a guest fault, which
    # is a bug rather than a missing import and has no entry here by design.
    explained = set()
    for rows_, frontier in (
        (symbol_rows, evidence["frontier_symbol"]),
        (class_rows, evidence["frontier_class"]),
        (selector_rows, evidence["frontier_selector"]),
    ):
        for _t, _d, _f, _u, _s, _v, name in rows_:
            explained |= frontier.get(name, set())
    broken = evidence["broken"] & shared
    print(
        f"{len(explained & broken)} of {len(broken)} apps that broke stopped on a "
        f"gap listed above; the rest stopped on a panic, a guest fault, or "
        f"something already implemented."
    )


def cmd_show(args):
    rows = load_catalogue(Path(args.catalogue))
    wanted = args.name
    hits = []
    for row in rows:
        if "error" in row:
            continue
        weak = None
        if wanted in row.get("functions", {}):
            weak = row["functions"][wanted][1]
        elif wanted in row.get("classes", {}):
            weak = row["classes"][wanted]
        elif wanted in row.get("selectors", []):
            weak = False
        else:
            continue
        hits.append((row, weak))
    print(f"{len(hits)} files reference {wanted}\n")
    for row, weak in sorted(hits, key=lambda h: (h[0].get("display_name") or "")):
        name = row.get("display_name") or Path(row["ipa"]).stem
        mark = "  (weak)" if weak else ""
        print(f"  {name}  [{row.get('bundle_identifier', '?')}]{mark}")


def cmd_app(args):
    rows = load_catalogue(Path(args.catalogue))
    host = host_coverage()
    needle = args.name.lower()
    matches = [
        r
        for r in rows
        if "error" not in r
        and (
            needle in (r.get("display_name") or "").lower()
            or needle in (r.get("bundle_identifier") or "").lower()
            or needle in Path(r["ipa"]).stem.lower()
        )
    ]
    if not matches:
        sys.exit(f"no app in the catalogue matches {args.name!r}")

    for row in matches[: args.limit]:
        print(f"\n{row.get('display_name') or Path(row['ipa']).stem}  "
              f"[{row.get('bundle_identifier', '?')}]  "
              f"{row.get('short_version') or row.get('bundle_version')}  "
              f"min iOS {row.get('minimum_os_version') or '?'}")
        provided = {framework_of(p) for p in host["dylibs"]}
        missing_fw = sorted(
            {framework_of(d) for d in row["dylibs"]} - provided
        )
        missing_cls = sorted(
            n for n, weak in row["classes"].items()
            if n not in host["classes"] and (args.include_weak or not weak)
        )
        missing_sym = sorted(
            s for s, (_lib, weak) in row["functions"].items()
            if s not in host["symbols"] and (args.include_weak or not weak)
        )
        print(f"  frameworks not provided ({len(missing_fw)}): "
              f"{', '.join(missing_fw) or 'none'}")
        print(f"  classes missing ({len(missing_cls)}):")
        for name in missing_cls:
            print(f"      {name}")
        print(f"  symbols missing ({len(missing_sym)}):")
        for name in missing_sym:
            print(f"      {name}")


def cmd_closest(args):
    """Apps with the least unimplemented demand — the cheapest next targets.

    This ranks by what is *left*, which no frontier survey can do: an app that
    dies immediately on one missing constant and an app that needs a whole
    framework look equally broken when run, and look nothing alike here.
    """
    rows = load_catalogue(Path(args.catalogue))
    host = host_coverage()
    provided = {framework_of(p) for p in host["dylibs"]}

    best = {}
    for row in rows:
        if "error" in row:
            continue
        total = len(row["functions"]) + len(row["classes"])
        if total < args.min_imports:
            continue  # a binary with almost no imports is a stub, not a game
        missing_fw = {framework_of(d) for d in row["dylibs"]} - provided
        missing = sum(
            1 for n, weak in row["classes"].items()
            if n not in host["classes"] and not weak
        ) + sum(
            1 for s, (_lib, weak) in row["functions"].items()
            if s not in host["symbols"] and not weak
        )
        key = app_key(row)
        score = (missing, len(missing_fw))
        if key not in best or score < best[key][0]:
            best[key] = (score, row, missing, sorted(missing_fw))

    ranked = sorted(best.values(), key=lambda v: v[0])
    print(f"{len(ranked)} apps ranked by remaining strong-import demand "
          f"(fewest first)\n")
    for _score, row, missing, missing_fw in ranked[: args.top]:
        name = row.get("display_name") or Path(row["ipa"]).stem
        fw = f"  +{len(missing_fw)} frameworks: {', '.join(missing_fw)}" if missing_fw else ""
        print(f"{missing:5d}  {name}  [{row.get('bundle_identifier', '?')}]{fw}")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument(
        "--catalogue",
        default=str(DEFAULT_CATALOGUE),
        help="result file. Defaults outside the repository, because it lists "
        "the apps in your collection. Never commit it.",
    )
    sub = ap.add_subparsers(dest="command", required=True)

    scan = sub.add_parser("scan", help="read the imports of a directory of IPAs")
    scan.add_argument("--apps", required=True, help="directory to search recursively")
    scan.add_argument("--limit", type=int, default=0, help="stop after N apps")
    scan.add_argument("--jobs", type=int, default=8, help="parallel readers")
    scan.add_argument("--verbose", action="store_true")
    scan.set_defaults(func=cmd_scan)

    cov = sub.add_parser("coverage", help="what tapHLE's source exports")
    cov.add_argument("--classes", action="store_true", help="list every class")
    cov.set_defaults(func=cmd_coverage)

    todo = sub.add_parser("todo", help="demand tapHLE does not satisfy, ranked")
    todo.add_argument("--top", type=int, default=40)
    todo.add_argument("--min-apps", type=int, default=1)
    todo.add_argument("--framework", help="only symbols from this framework")
    todo.add_argument(
        "--include-weak",
        action="store_true",
        help="count weak imports, which apps null-check and usually do not need",
    )
    todo.set_defaults(func=cmd_todo)

    cross = sub.add_parser(
        "cross", help="rank gaps by whether the frontier survey saw them block"
    )
    cross.add_argument("--survey", default=str(DEFAULT_SURVEY))
    cross.add_argument("--top", type=int, default=25)
    cross.add_argument(
        "--sort",
        choices=("priority", "demand", "frontier"),
        default="priority",
        help="priority puts proven blockers first, then reachable, then latent",
    )
    cross.add_argument("--no-selectors", action="store_true")
    cross.add_argument("--include-weak", action="store_true")
    cross.set_defaults(func=cmd_cross)

    show = sub.add_parser("show", help="which apps reference one symbol or selector")
    show.add_argument("name")
    show.set_defaults(func=cmd_show)

    app = sub.add_parser("app", help="everything one app is missing")
    app.add_argument("name", help="substring of the name or bundle identifier")
    app.add_argument("--limit", type=int, default=3)
    app.add_argument("--include-weak", action="store_true")
    app.set_defaults(func=cmd_app)

    close = sub.add_parser("closest", help="apps with the least missing")
    close.add_argument("--top", type=int, default=40)
    close.add_argument("--min-imports", type=int, default=50)
    close.set_defaults(func=cmd_closest)

    args = ap.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
