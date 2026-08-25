#!/bin/sh
set -eu

REPO=$(cd "$(dirname "$0")/../.." && pwd)

if [ "$#" -ne 1 ]; then
    echo "Usage: $0 /path/to/tapHLE" >&2
    exit 1
fi

PATH_TO_BINARY=$1
if [ ! -f "$PATH_TO_BINARY" ] || [ ! -x "$PATH_TO_BINARY" ]; then
    echo "tapHLE executable is missing or not executable: $PATH_TO_BINARY" >&2
    exit 1
fi

BUNDLE=tapHLE_linux_bundle
STAGING=${BUNDLE}.tmp
rm -rf "$STAGING"
trap 'rm -rf "$STAGING"' EXIT HUP INT TERM
mkdir "$STAGING"
cp "$PATH_TO_BINARY" "$STAGING/tapHLE"
chmod +x "$STAGING/tapHLE"

cp -r "$REPO/runtime/dylibs" "$STAGING/"
cp -r "$REPO/runtime/fonts" "$STAGING/"
mkdir "$STAGING/apps"
cp "$REPO/runtime/apps/README.txt" "$STAGING/apps/"
mkdir "$STAGING/res"
cp "$REPO/runtime/res/icon.png" "$STAGING/res/"
cp "$REPO/README.md" "$STAGING/"
cp "$REPO/CHANGELOG.md" "$STAGING/"
cp "$REPO/dev-scripts/gpl-3.0.txt" "$STAGING/COPYING.txt"
cp "$REPO/runtime/OPTIONS_HELP.txt" "$STAGING/"
cp "$REPO/runtime/default_options.txt" "$STAGING/"
cp "$REPO/runtime/options.txt" "$STAGING/"

rm -rf "$BUNDLE"
mv "$STAGING" "$BUNDLE"
trap - EXIT HUP INT TERM
