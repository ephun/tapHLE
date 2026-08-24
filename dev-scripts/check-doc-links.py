#!/usr/bin/env python3
"""Fail when tracked text refers to a repository file that does not exist.

The tapHLE README pointed twice at `dev-docs/packaging.md` during a period when
that file did not exist, and nothing noticed. This is the check that would have.

It reads only the tracked tree and touches no network. Two kinds of reference
are checked:

  * a repo-relative path inside backticks or a Markdown link target, under one
    of the directories tapHLE actually has; and
  * a backticked root-level document name such as `AGENTS.md`.

Placeholders are skipped: a path containing `<` or `>` is a template
(`compatibility/notes/<app-slug>.md`), not a claim that a file exists.
"""

import pathlib
import re
import subprocess
import sys

TEXT_SUFFIXES = {
    ".md", ".rs", ".sh", ".ps1", ".py", ".yml", ".yaml", ".toml", ".txt",
}

TRACKED_DIRS = (
    "docs", "dev-scripts", "compatibility", "tests", "src", "res", "android",
    r"\.github",
)

PATH_RE = re.compile(
    r"(?:`|\]\()((?:" + "|".join(TRACKED_DIRS) + r")"
    r"/[A-Za-z0-9_./<>-]*?\.(?:md|json|txt|rs|sh|ps1|py|yml|iss))(?:`|\))"
)

ROOT_DOC_RE = re.compile(
    r"`(AGENTS|CLAUDE|README|CONTRIBUTING|CHANGELOG|CODE_OF_CONDUCT"
    r"|OPTIONS_HELP|options|default_options)\.(md|txt)`"
)


def tracked_text_files():
    listing = subprocess.run(
        ["git", "ls-files"], capture_output=True, text=True, check=True
    ).stdout
    for name in listing.split("\n"):
        name = name.strip()
        if not name:
            continue
        path = pathlib.Path(name)
        if path.suffix.lower() in TEXT_SUFFIXES and path.exists():
            yield path


def references(text):
    for match in PATH_RE.finditer(text):
        target = match.group(1)
        # A templated path is a shape, not a file.
        if "<" not in target and ">" not in target:
            yield target
    for match in ROOT_DOC_RE.finditer(text):
        yield f"{match.group(1)}.{match.group(2)}"


# docs/platforms.md owns the per-platform status matrix. It lived in README.md,
# AGENTS.md and dev-docs/packaging.md at the same time, and the three disagreed
# about whether the first release was Windows-only or all-platform. A second
# copy is not a formatting preference; it is how that happens again.
MATRIX_OWNER = "docs/platforms.md"

MATRIX_MARKERS = (
    "Linux x86_64",
    "inherited source only",
)


def duplicate_matrices():
    """Report any file other than the owner carrying a platform status table."""
    offenders = {}
    for path in tracked_text_files():
        name = str(path).replace("\\", "/")
        if name == MATRIX_OWNER or name == "dev-scripts/check-doc-links.py":
            continue
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        # A table needs both a row label and the pipe syntax to be a matrix
        # rather than prose that happens to mention a platform.
        hits = [m for m in MATRIX_MARKERS if m in text]
        if hits and "| --- |" in text:
            offenders[name] = hits
    return offenders


def main():
    broken = {}
    for path in tracked_text_files():
        try:
            text = path.read_text(encoding="utf-8")
        except (UnicodeDecodeError, OSError):
            continue
        for target in references(text):
            if not pathlib.Path(target).exists():
                broken.setdefault(target, set()).add(str(path))

    duplicates = duplicate_matrices()

    if not broken and not duplicates:
        print("Documentation references: OK")
        print(f"Platform status matrix: only in {MATRIX_OWNER}")
        return 0

    for target in sorted(broken):
        print(f"Missing referenced file: {target}", file=sys.stderr)
        for source in sorted(broken[target]):
            print(f"    referenced by {source}", file=sys.stderr)

    for name in sorted(duplicates):
        print(
            f"Platform status matrix duplicated in {name} "
            f"(matched: {', '.join(duplicates[name])})",
            file=sys.stderr,
        )
        print(f"    {MATRIX_OWNER} owns it. Link there instead.", file=sys.stderr)

    if broken:
        print(
            f"\n{len(broken)} referenced file(s) do not exist.",
            file=sys.stderr,
        )
    return 1


if __name__ == "__main__":
    sys.exit(main())
