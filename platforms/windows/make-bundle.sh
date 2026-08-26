#!/bin/sh
set -e

# Inputs are found relative to this script rather than to wherever it was
# run from, so moving the script does not silently change what it copies.
REPO=$(cd "$(dirname "$0")/../.." && pwd)

# Assemble the complete redistributable Windows directory used for CI previews
# and numbered releases.
#
# The first argument is the emulator executable. The desktop frontend is taken
# from beside it, since both are built into the same profile directory; it is
# not required, so a build that has not produced one still yields a working
# bundle.

if [ "$#" -eq 1 ]; then
    PATH_TO_BINARY="$1"
    shift

    rm -rf tapHLE_windows_bundle
    mkdir tapHLE_windows_bundle
    cp "$PATH_TO_BINARY" tapHLE_windows_bundle/

    # Rust GNU builds need these MinGW runtime libraries when launched from
    # Explorer, whose PATH does not include Git for Windows.
    for library in libgcc_s_seh-1.dll libstdc++-6.dll libwinpthread-1.dll; do
        if [ ! -f "/mingw64/bin/$library" ]; then
            echo "Required Windows runtime library is missing: /mingw64/bin/$library" >&2
            exit 1
        fi
        cp "/mingw64/bin/$library" tapHLE_windows_bundle/
    done

    cp -r "$REPO"/runtime/dylibs tapHLE_windows_bundle/
    cp -r "$REPO"/runtime/fonts tapHLE_windows_bundle/
    mkdir tapHLE_windows_bundle/apps
    cp "$REPO"/runtime/apps/README.txt tapHLE_windows_bundle/apps/
    # The frontend looks for the window icon beside itself.
    mkdir tapHLE_windows_bundle/res
    cp "$REPO"/runtime/res/icon.png tapHLE_windows_bundle/res/
    cp "$REPO"/README.md tapHLE_windows_bundle/
    cp "$REPO"/CHANGELOG.md tapHLE_windows_bundle/
    cp "$REPO"/dev-scripts/gpl-3.0.txt tapHLE_windows_bundle/COPYING.txt
    cp "$REPO"/runtime/OPTIONS_HELP.txt tapHLE_windows_bundle/
    cp "$REPO"/runtime/default_options.txt tapHLE_windows_bundle/
    cp "$REPO"/runtime/options.txt tapHLE_windows_bundle/
else
    echo "Incorrect usage."
    exit 1
fi
