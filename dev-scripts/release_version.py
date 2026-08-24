#!/usr/bin/env python3
"""Validate tapHLE release tags against the workspace Cargo version."""

from __future__ import annotations

import argparse
from datetime import date
from pathlib import Path
import re
import sys


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
TAG_PREFIX = "taphle-v"
CHANGELOG_PATH = REPOSITORY_ROOT / "CHANGELOG.md"


def workspace_version(cargo_toml: Path = REPOSITORY_ROOT / "Cargo.toml") -> str:
    in_workspace_package = False
    for line in cargo_toml.read_text(encoding="utf-8").splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            in_workspace_package = stripped == "[workspace.package]"
            continue
        if not in_workspace_package:
            continue
        match = re.fullmatch(r'version\s*=\s*"([^"]+)"\s*', stripped)
        if match is not None:
            return match.group(1)
    raise ValueError(f"could not find [workspace.package] version in {cargo_toml}")


SEMVER_NUMBER = r"(?:0|[1-9]\d*)"
DEVELOPMENT_VERSION = re.compile(
    rf"^{SEMVER_NUMBER}\.{SEMVER_NUMBER}\.{SEMVER_NUMBER}"
    r"-dev\.[1-9]\d*(?:\+g[0-9A-Fa-f]{7,40})?$"
)
RELEASE_VERSION = re.compile(
    rf"^{SEMVER_NUMBER}\.{SEMVER_NUMBER}\.{SEMVER_NUMBER}$"
)
ARTIFACT_SHAPES = {
    ("Windows", "x86_64", "zip"),
    ("Linux", "x86_64", "tar.gz"),
    ("macOS", "x86_64", "dmg"),
    ("Android", "arm64-v8a", "apk"),
    ("iOS", "arm64", "ipa"),
}


def validate_development_version(version: str) -> None:
    if DEVELOPMENT_VERSION.fullmatch(version) is None:
        raise ValueError(
            f"development version {version!r} must use X.Y.Z-dev.N with N >= 1 "
            "and optional +g<commit> metadata"
        )


def validate_release_version(version: str) -> None:
    if RELEASE_VERSION.fullmatch(version) is None:
        raise ValueError(
            f"numbered release {version!r} must use X.Y.Z with no alpha, beta, "
            "release-candidate, development, or build-metadata suffix"
        )


def release_tag(version: str) -> str:
    validate_release_version(version)
    return f"{TAG_PREFIX}{version}"


def artifact_name(version: str, host: str, architecture: str, extension: str) -> str:
    validate_release_version(version)
    shape = (host, architecture, extension)
    if shape not in ARTIFACT_SHAPES:
        raise ValueError(f"unsupported release artifact shape: {shape!r}")
    return f"tapHLE-v{version}-{host}-{architecture}.{extension}"


def windows_archive_name(version: str) -> str:
    return artifact_name(version, "Windows", "x86_64", "zip")


def windows_installer_name(version: str) -> str:
    validate_release_version(version)
    return f"tapHLE-v{version}-Windows-x86_64-setup.exe"


def release_artifact_names(version: str) -> list[str]:
    names = [
        artifact_name(version, host, architecture, extension)
        for host, architecture, extension in sorted(ARTIFACT_SHAPES)
    ]
    names.append(windows_installer_name(version))
    return sorted(names)


def validate_tag(tag: str, version: str) -> None:
    validate_release_version(version)
    expected = release_tag(version)
    if tag != expected:
        raise ValueError(
            f"release tag {tag!r} does not exactly match Cargo version; "
            f"expected {expected!r}"
        )


def validate_changelog(
    version: str, changelog: Path = CHANGELOG_PATH
) -> None:
    heading = re.compile(
        rf"^## {re.escape(version)} - (\d{{4}}-\d{{2}}-\d{{2}})$"
    )
    for line in changelog.read_text(encoding="utf-8").splitlines():
        match = heading.fullmatch(line)
        if match is None:
            continue
        try:
            date.fromisoformat(match.group(1))
        except ValueError as error:
            raise ValueError(f"invalid release date in changelog heading: {line!r}") from error
        return
    raise ValueError(
        f"{changelog} needs an exact release heading: "
        f"'## {version} - YYYY-MM-DD'"
    )


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    subparsers = parser.add_subparsers(dest="command", required=True)

    check_parser = subparsers.add_parser("check-tag")
    check_parser.add_argument("tag")

    subparsers.add_parser("version")
    subparsers.add_parser("check-version")
    subparsers.add_parser("archive-name")
    subparsers.add_parser("artifact-names")

    args = parser.parse_args(argv)
    version = workspace_version()
    if args.command == "check-tag":
        try:
            validate_tag(args.tag, version)
            validate_changelog(version)
        except ValueError as error:
            print(f"Error: {error}", file=sys.stderr)
            return 1
        print(f"Validated tapHLE release tag {args.tag} and changelog heading")
    elif args.command == "version":
        print(version)
    elif args.command == "check-version":
        try:
            validate_release_version(version)
        except ValueError as error:
            print(f"Error: {error}", file=sys.stderr)
            return 1
        print(f"Validated numbered release version {version}")
    elif args.command == "archive-name":
        print(windows_archive_name(version))
    elif args.command == "artifact-names":
        for name in release_artifact_names(version):
            print(name)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
