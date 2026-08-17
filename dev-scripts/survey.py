#!/usr/bin/env python3
"""Survey where every app in a collection stops, and rank the causes by frequency.

The database calls "where an app stops" its *frontier*. This records the frontier
of every app in a local collection at once, then ranks the causes, so work is
chosen by measurement instead of by whichever app happened to be looked at. Two
apps fixed in one session — JellyCar 3 and Scoops — turned out to die on the
*same* missing constant, found by accident hours apart. A survey surfaces that as
one row with a count of two, and across a large collection probably thirty.

    python dev-scripts/survey.py run   --apps "D:\\my ipas"
    python dev-scripts/survey.py rank
    python dev-scripts/survey.py rank --symbols
    python dev-scripts/survey.py show "selector:UIAlertView.setMessage:"
    python dev-scripts/survey.py candidates

`run` is resumable: it skips any app already surveyed for the current tapHLE
revision, so an interrupted overnight run continues where it stopped. Re-run it
after each batch of fixes; the ranking is a moving target and that iteration is
the product, not the first snapshot.

## The result file is private and must never be committed

It lists the apps in someone's personal collection, which is theirs to disclose
or not. The default output path is outside the repository for that reason, and
`.gitignore` covers the pattern in case one is written inside anyway. Share
findings — "thirty apps want this selector" — not the file.

## What this measures, and what it does not

A row is a **lower bound observed under one unattended run of a few seconds**. It
is not a compatibility rating, and must never be filed as one:

- Nothing here presses a button, so any app whose first screen needs a tap looks
  stuck even when it is fine.
- Some apps need an orientation option to render correctly at all.
- Clearing the first break says nothing about the second. Scoops' first break was
  a single missing constant, with seven more fixes behind it.

So the ranking orders *unblocking* work — the cheap fixes that clear the most
apps — rather than work-to-three-stars. `candidates` lists rows worth a closer
look; turning one into a database report means actually running that app and
reading its identity with `tapHLE --info`, per `compatibility/README.md`. This
script deliberately cannot file reports.
"""

import argparse
import collections
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
DEFAULT_EXE = REPO / "target" / "release" / "tapHLE.exe"
# Outside the repository on purpose: the result lists someone's own collection.
DEFAULT_CATALOGUE = (
    Path(os.environ.get("TEMP") or os.environ.get("TMPDIR") or "/tmp")
    / "taphle-frontier-survey.jsonl"
)

# Ordered: the first pattern that matches a run's output wins, so the more
# specific classifications must come before the general ones.
#
# `key` is what rows group by, and choosing it well is the whole design. An
# unknown selector groups by class and selector, because that names the thing to
# implement. A guest MemoryError groups as one bucket, because its faulting PC
# differs per app and would make every row unique — the address goes in `detail`
# for follow-up with dev-scripts/disasm-guest-fault.py.
CLASSIFIERS = [
    (
        "unimplemented-function",
        re.compile(r"Call to unimplemented function (?P<sym>\S+)"),
        lambda m: (f"function:{m.group('sym')}", m.group(0)),
    ),
    (
        "selector",
        re.compile(
            r'(?:Object|Class) .*? \((?:class |metaclass )?"(?P<cls>[^"]+)", .*?\)'
            r'.*? does not respond to selector "(?P<sel>[^"]+)"'
        ),
        lambda m: (f"selector:{m.group('cls')}.{m.group('sel')}", m.group(0)[:200]),
    ),
    (
        "unimplemented-class-method",
        re.compile(
            r'Class "(?P<cls>[^"]+)" \([^)]*\) is unimplemented\. '
            r'Call to \w+ method "(?P<sel>[^"]+)"'
        ),
        lambda m: (f"class-stub:{m.group('cls')}.{m.group('sel')}", m.group(0)[:200]),
    ),
    (
        "missing-class",
        re.compile(r"Missing implementation for class (?P<cls>[^!\n]+)!"),
        lambda m: (f"class-missing:{m.group('cls').strip()}", m.group(0)[:200]),
    ),
    (
        "host-object",
        re.compile(r'No host object for .*Wanted host\s+type "(?P<want>[^"]+)"', re.S),
        lambda m: (f"host-object:{m.group('want').rsplit('::', 1)[-1]}", m.group(0)[:200]),
    ),
    (
        "not-implemented",
        re.compile(r"not implemented: (?P<what>.+)"),
        lambda m: (f"unimplemented:{m.group('what').strip()[:80]}", m.group(0)),
    ),
    (
        "memory-error",
        re.compile(r"Attempted null-page access at (?P<addr>0x[0-9a-f]+)"),
        lambda m: ("guest:memory-error", f"null-page access at {m.group('addr')}"),
    ),
    (
        "cpu-error",
        re.compile(r"Error during CPU execution: (?P<kind>\w+)"),
        lambda m: (f"guest:cpu-{m.group('kind')}", m.group(0)),
    ),
    (
        "bad-bundle",
        re.compile(r"Could not open app bundle: (?P<why>.+)"),
        lambda m: ("launch:bad-bundle", m.group(0)[:200]),
    ),
    # tapHLE reports a crash in a modal dialog as well as on stderr. The dialog
    # is why liveness cannot be trusted here: it keeps the process running until
    # something dismisses it, so an unattended run sees a healthy-looking process
    # that is in fact dead in the water.
    (
        "crash-dialog",
        re.compile(r"tapHLE crashed with the following error: (?P<why>[^\n]+)"),
        lambda m: (f"crash:{m.group('why').strip()[:80]}", m.group("why").strip()[:300]),
    ),
    # Least specific: any other tapHLE panic, keyed by source location so that
    # every app hitting the same assertion lands in one bucket.
    (
        "panic",
        re.compile(r"panicked at (?P<loc>[^\s:]+:\d+):\d+:\s*\n(?P<msg>[^\n]+)"),
        lambda m: (f"panic:{m.group('loc')}", m.group("msg").strip()[:200]),
    ),
]

MISSING_SYMBOL = re.compile(r'unhandled non-lazy symbol "(?P<name>[^"]+)"')
INFO_FIELD = re.compile(r"^- (?P<key>[^:]+):\s*(?P<value>.*)$", re.M)


def taphle_revision(exe):
    """The build's own version string, so a catalogue row records what produced it."""
    try:
        out = subprocess.run(
            [str(exe), "--help"],
            capture_output=True,
            text=True,
            errors="replace",
            timeout=60,
        )
        # "tapHLE <revision> — https://github.com/...". Split on whitespace
        # rather than the em dash, which does not survive every console encoding.
        first = (out.stdout or out.stderr).splitlines()[0]
        parts = first.split()
        if len(parts) >= 2 and parts[0] == "tapHLE":
            return parts[1]
    except Exception:
        pass
    return "unknown"


def read_identity(exe, ipa, workdir):
    """Identity from the bundle, never from the filename. See compatibility/README.md."""
    try:
        out = subprocess.run(
            [str(exe), "--info", str(ipa)],
            capture_output=True,
            text=True,
            timeout=120,
            cwd=str(workdir),
        )
    except subprocess.TimeoutExpired:
        return {}
    fields = {}
    for m in INFO_FIELD.finditer((out.stdout or "") + (out.stderr or "")):
        fields[m.group("key").strip().lower()] = m.group("value").strip()
    return {
        "display_name": fields.get("display name", ""),
        "bundle_identifier": fields.get("identifier", ""),
        "bundle_version": fields.get("version", ""),
        "minimum_os_version": fields.get("minimum os version", ""),
        "device_family": fields.get("device family", ""),
    }


def classify(output):
    for _name, pattern, build in CLASSIFIERS:
        m = pattern.search(output)
        if m:
            return build(m)
    return None


def run_one(exe, ipa, workdir, timeout):
    """Launch the app, wait, and report how it ended plus what it could not bind."""
    log = []
    started = time.monotonic()
    proc = subprocess.Popen(
        [str(exe), str(ipa)],
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        errors="replace",
        cwd=str(workdir),
    )
    try:
        out, _ = proc.communicate(timeout=timeout)
        log.append(out or "")
        exit_code = proc.returncode
        survived = False
    except subprocess.TimeoutExpired:
        proc.kill()
        try:
            out, _ = proc.communicate(timeout=30)
            log.append(out or "")
        except Exception:
            pass
        exit_code = None
        survived = True
    elapsed = round(time.monotonic() - started, 1)
    output = "".join(log)

    missing = sorted(set(MISSING_SYMBOL.findall(output)))

    # Classify before considering the process's fate, never after. tapHLE shows
    # a modal dialog when it crashes, and a modal dialog keeps the process alive
    # forever, so "was still running at the timeout" is true of a healthy app and
    # of a dead one alike. The output is the evidence; being alive is not.
    found = classify(output)

    if found is None and survived:
        # Nothing fatal in the output and still going. That is the best outcome
        # this harness can observe — it means nothing broke in the first few
        # seconds, not that the app is playable, because nothing here presses a
        # button.
        return {
            "reason_key": "ok:survived",
            "detail": f"still running after {elapsed}s",
            "exit_code": None,
            "seconds": elapsed,
            "missing_symbols": missing,
        }

    if found is None:
        # Exited without a recognised failure. A clean exit usually means the app
        # asked to quit; a non-zero one is a failure this taxonomy has no name for
        # yet, and the tail is kept so the gap can be closed.
        key = "exit:clean" if exit_code == 0 else "exit:unclassified"
        detail = " | ".join(line.strip() for line in output.strip().splitlines()[-3:])
        found = (key, detail[:300])
    return {
        "reason_key": found[0],
        "detail": found[1],
        "exit_code": exit_code,
        "seconds": elapsed,
        "missing_symbols": missing,
        # Kept so `reclassify` can re-key an existing survey when the taxonomy
        # improves. It will: the first version bucketed the four most common
        # causes as bare tapHLE source locations, throwing away the function,
        # class and selector names that say what to actually implement. Nobody
        # should have to re-run a ten-hour survey to benefit from that.
        "raw_tail": output[-4000:],
    }


def load_catalogue(path):
    rows = []
    if path.exists():
        with open(path, encoding="utf-8") as f:
            for line in f:
                line = line.strip()
                if line:
                    rows.append(json.loads(line))
    return rows


def cmd_run(args):
    exe = Path(args.exe)
    if not exe.exists():
        sys.exit(f"no tapHLE build at {exe} — run `cargo build --release` first")
    catalogue = Path(args.catalogue)
    catalogue.parent.mkdir(parents=True, exist_ok=True)
    workdir = Path(args.workdir)
    for needed in ("tapHLE_dylibs", "tapHLE_fonts"):
        if not (workdir / needed).is_dir():
            sys.exit(
                f"{workdir} has no {needed}/. tapHLE resolves it relative to its "
                "working directory, and without it every app dies on a missing "
                "dylib rather than on anything worth cataloguing."
            )

    revision = taphle_revision(exe)
    if revision.endswith("-dirty") and not args.allow_dirty:
        sys.exit(
            f"build is {revision}; a catalogue from a dirty worktree cannot be "
            "reproduced. Commit first, or pass --allow-dirty for a scratch run."
        )

    done = {
        (row["ipa"], row["taphle_version"])
        for row in load_catalogue(catalogue)
    }

    # Absolute, because tapHLE runs with its cwd set to the work directory and
    # would resolve a relative path against that instead of against here.
    apps = sorted(
        p.resolve() for p in Path(args.apps).rglob("*.ipa") if p.is_file()
    )
    if args.limit:
        apps = apps[: args.limit]
    print(f"{len(apps)} IPAs found, tapHLE {revision}, timeout {args.timeout}s")

    for index, ipa in enumerate(apps, 1):
        key = (str(ipa), revision)
        if key in done:
            continue
        identity = read_identity(exe, ipa, workdir)
        result = run_one(exe, ipa, workdir, args.timeout)
        row = {
            "ipa": str(ipa),
            "taphle_version": revision,
            **identity,
            **result,
        }
        with open(catalogue, "a", encoding="utf-8") as f:
            f.write(json.dumps(row) + "\n")
        name = identity.get("display_name") or ipa.stem
        print(f"[{index}/{len(apps)}] {name}: {result['reason_key']}", flush=True)


def cmd_reclassify(args):
    """Re-key an existing survey against the current taxonomy, without re-running.

    Surveys are expensive and the taxonomy is not finished, so improving the
    classifiers must not invalidate work already done. Rows keep a bounded tail
    of their output for exactly this, and older rows can usually still be re-keyed
    from the panic message kept in `detail`.
    """
    path = Path(args.catalogue)
    rows = load_catalogue(path)
    if not rows:
        sys.exit("catalogue is empty")
    changed = 0
    for row in rows:
        found = classify(row.get("raw_tail") or row.get("detail") or "")
        if found and found[0] != row["reason_key"]:
            if args.verbose:
                print(f"  {row['reason_key']}  ->  {found[0]}")
            row["reason_key"], row["detail"] = found
            changed += 1
    with open(path, "w", encoding="utf-8") as f:
        for row in rows:
            f.write(json.dumps(row) + "\n")
    print(f"re-keyed {changed} of {len(rows)} rows")


FAULT_PC = re.compile(r"\bPC: (0x[0-9a-f]+)")
FAULT_ADDR = re.compile(r"null-page access at (0x[0-9a-f]+)")


def _load_attributor():
    """Import attribute-fault.py, whose hyphenated name is not a module name."""
    import importlib.util

    path = Path(__file__).with_name("attribute-fault.py")
    spec = importlib.util.spec_from_file_location("attribute_fault", path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def cmd_attribute(args):
    """Name the import behind each null dereference, and rank by apps killed.

    This is the step that makes the symbol ranking mean something. `rank
    --symbols` counts how often an import is *referenced*, which over-weights
    symbols that every binary mentions and never uses. This counts how often one
    actually killed an app, by resolving the faulting instruction back to the
    slot it read through.

    A result is `confirmed` when the symbol it names also appears in the list
    tapHLE reported as unbound for that same app — two independent routes
    agreeing that this exact import had no value.
    """
    rows = load_catalogue(Path(args.catalogue))
    attributor = _load_attributor()

    faulted = [r for r in rows if r["reason_key"] == "guest:memory-error"]
    by_symbol = collections.defaultdict(set)
    unattributed = []
    null_deref = 0

    for row in faulted:
        tail = row.get("raw_tail") or ""
        where = FAULT_ADDR.search(tail) or FAULT_ADDR.search(row.get("detail") or "")
        # A fault at exactly 0 is a null import being read through. A fault at a
        # small non-zero offset is a field of a null object, which is a failed
        # subsystem rather than a missing symbol, and is not attributable here.
        if not where or int(where.group(1), 16) != 0:
            continue
        null_deref += 1
        pc = FAULT_PC.search(tail)
        if not pc:
            unattributed.append((row, "no PC in the retained output"))
            continue
        try:
            results = attributor.attribute(
                row["ipa"], int(pc.group(1), 16), set(row.get("missing_symbols") or [])
            )
        except Exception as e:  # a malformed or unusual binary must not stop the pass
            unattributed.append((row, f"{type(e).__name__}: {e}"))
            continue
        if not results:
            unattributed.append((row, f"no symbol-pointer load near {pc.group(1)}"))
            continue
        best = results[0]
        if args.confirmed_only and not best["confirmed"]:
            unattributed.append((row, f"unconfirmed guess {best['symbol']}"))
            continue
        by_symbol[best["symbol"]].add(row.get("bundle_identifier") or row["ipa"])

    print(
        f"{len(faulted)} apps died on a guest MemoryError; {null_deref} of those "
        f"read through a null pointer at address 0.\n"
    )
    ranked = sorted(by_symbol.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    print("imports that actually killed apps:\n")
    for symbol, apps in ranked[: args.top]:
        print(f"{len(apps):5d}  {symbol}")
    attributed = sum(len(a) for a in by_symbol.values())
    print(f"\nattributed {attributed}, unattributed {len(unattributed)}")
    if args.verbose:
        for row, why in unattributed:
            name = row.get("display_name") or Path(row["ipa"]).stem
            print(f"  {name}: {why}")


def cmd_rank(args):
    rows = load_catalogue(Path(args.catalogue))
    if not rows:
        sys.exit("catalogue is empty; run `triage.py run` first")
    # Count distinct apps, not rows. A collection typically holds several
    # versions of the same app — this sample had eight Angry Birds and five Air
    # Videos — and ranking by rows lets one app that happens to be well
    # represented outrank a cause that blocks many different ones. Identity comes
    # from the bundle identifier; rows without one fall back to their path so
    # they are still counted once each.
    def app_key(row):
        return row.get("bundle_identifier") or row["ipa"]

    groups = collections.defaultdict(set)
    if args.symbols:
        for row in rows:
            for symbol in row.get("missing_symbols") or []:
                groups[symbol].add(app_key(row))
        title = "unbound imports, by how many distinct apps reference them"
    else:
        for row in rows:
            groups[row["reason_key"]].add(app_key(row))
        title = "first break, by how many distinct apps hit it"

    distinct = len({app_key(row) for row in rows})
    ranked = sorted(groups.items(), key=lambda kv: (-len(kv[1]), kv[0]))
    print(f"{distinct} distinct apps ({len(rows)} files) — {title}\n")
    for key, apps in ranked[: args.top]:
        print(f"{len(apps):5d}  {key}")


def cmd_show(args):
    rows = load_catalogue(Path(args.catalogue))
    for row in rows:
        if row["reason_key"] == args.reason or args.reason in (
            row.get("missing_symbols") or []
        ):
            name = row.get("display_name") or Path(row["ipa"]).stem
            print(f"{name}  [{row.get('bundle_identifier', '?')}]")
            print(f"    {row['detail']}")


def cmd_candidates(args):
    """Rows a human or agent might want to turn into a real report.

    Deliberately a list, not a submission. A row here is a 30-second unattended
    observation; a database report requires actually running the app and reading
    its identity with `tapHLE --info`. See the module docstring.
    """
    rows = load_catalogue(Path(args.catalogue))
    broken = [r for r in rows if r["reason_key"].split(":")[0] in
              ("panic", "guest", "launch", "host-object", "unimplemented", "selector")]
    counter = collections.Counter(r["reason_key"] for r in rows)
    # Rank by how rare the cause is: a break that only one app hits is a
    # genuinely app-specific problem, whereas a common one is better fixed than
    # reported.
    broken.sort(key=lambda r: counter[r["reason_key"]])
    print(f"{len(broken)} apps did not survive. Rarest causes first "
          f"(these are the app-specific ones):\n")
    for row in broken[: args.top]:
        name = row.get("display_name") or Path(row["ipa"]).stem
        print(f"{name}  [{row.get('bundle_identifier', '?')}]")
        print(f"    {row['reason_key']}  (shared with {counter[row['reason_key']] - 1} others)")
    print("\nNothing here is a compatibility rating. To file one, run the app "
          "yourself and follow compatibility/README.md.")


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument(
        "--catalogue",
        default=str(DEFAULT_CATALOGUE),
        help="result file. Defaults outside the repository, because it lists "
        "the apps in your collection. Never commit it.",
    )
    sub = ap.add_subparsers(dest="command", required=True)

    run = sub.add_parser("run", help="survey a directory of IPAs")
    run.add_argument("--apps", required=True, help="directory to search recursively")
    run.add_argument("--exe", default=str(DEFAULT_EXE))
    run.add_argument("--timeout", type=int, default=30)
    run.add_argument("--limit", type=int, default=0, help="stop after N apps")
    run.add_argument(
        "--workdir",
        default=str(REPO),
        help="tapHLE's base path. Must contain tapHLE_dylibs and tapHLE_fonts, "
        "which it resolves relative to its working directory — pointing this "
        "elsewhere makes every app die on a missing dylib. It writes only "
        "tapHLE_sandbox and tapHLE_log.txt, both already gitignored.",
    )
    run.add_argument("--allow-dirty", action="store_true")
    run.set_defaults(func=cmd_run)

    recl = sub.add_parser(
        "reclassify", help="re-key an existing survey against the current taxonomy"
    )
    recl.add_argument("--verbose", action="store_true")
    recl.set_defaults(func=cmd_reclassify)

    attr = sub.add_parser(
        "attribute", help="name the import behind each null dereference"
    )
    attr.add_argument("--top", type=int, default=30)
    attr.add_argument("--verbose", action="store_true", help="list what could not be attributed")
    attr.add_argument(
        "--confirmed-only",
        action="store_true",
        help="count only symbols tapHLE also reported unbound for that app",
    )
    attr.set_defaults(func=cmd_attribute)

    rank = sub.add_parser("rank", help="rank causes by how many apps hit them")
    rank.add_argument("--symbols", action="store_true", help="rank unbound imports instead")
    rank.add_argument("--top", type=int, default=30)
    rank.set_defaults(func=cmd_rank)

    show = sub.add_parser("show", help="list the apps behind one cause or symbol")
    show.add_argument("reason")
    show.set_defaults(func=cmd_show)

    cand = sub.add_parser("candidates", help="rows worth a closer look (never auto-filed)")
    cand.add_argument("--top", type=int, default=40)
    cand.set_defaults(func=cmd_candidates)

    args = ap.parse_args()
    args.func(args)


if __name__ == "__main__":
    main()
