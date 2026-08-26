#!/bin/sh
set -e

# Creates the .app bundle containing the basic set of files needed for tapHLE
# to run. Also adds an icon and standard application metadata.

REPO=$(CDPATH= cd -- "$(dirname "$0")/../.." && pwd)

if [ "$#" -eq 3 ]; then
    PATH_TO_BINARY="$1"
    VERSION="$2"
    BRANDING="$3"
    shift 3

    if [ -z "$BRANDING" ]; then
        APP_NAME=tapHLE
        ICON_NAME=icon
    else
        APP_NAME="tapHLE $BRANDING"
        ICON_NAME="icon_$(echo "$BRANDING" | tr 'A-Z' 'a-z')"
        VERSION="$VERSION $BRANDING"
    fi
    rm -rf "$ICON_NAME.icns" "$ICON_NAME.iconset"
    mkdir "$ICON_NAME.iconset"
    cp "$REPO/runtime/res/$ICON_NAME.png" "$ICON_NAME.iconset/icon_512x512.png"
    iconutil -c icns -o "$ICON_NAME.icns" "$ICON_NAME.iconset"

    rm -rf "$APP_NAME.app"
    mkdir -p "$APP_NAME.app"/Contents/MacOS "$APP_NAME.app"/Contents/Resources
    cp $PATH_TO_BINARY "$APP_NAME.app"/Contents/MacOS/tapHLE
    cp -r "$REPO/runtime/dylibs" "$APP_NAME.app"/Contents/Resources/
    cp -r "$REPO/runtime/fonts" "$APP_NAME.app"/Contents/Resources/
    cp "$REPO/runtime/default_options.txt" "$APP_NAME.app"/Contents/Resources/
    cp "$ICON_NAME.icns" "$APP_NAME.app"/Contents/Resources/

    plutil -create xml1 "$APP_NAME.app"/Contents/Info.plist
    plutil -insert CFBundleName -string "$APP_NAME" "$APP_NAME.app"/Contents/Info.plist
    plutil -insert CFBundleDisplayName -string "$APP_NAME" "$APP_NAME.app"/Contents/Info.plist
    plutil -insert CFBundleShortVersionString -string "$VERSION" "$APP_NAME.app"/Contents/Info.plist
    plutil -insert CFBundleExecutable -string tapHLE "$APP_NAME.app"/Contents/Info.plist
    plutil -insert CFBundleIconFile -string "$ICON_NAME" "$APP_NAME.app"/Contents/Info.plist
else
    echo "Incorrect usage."
    exit 1
fi
