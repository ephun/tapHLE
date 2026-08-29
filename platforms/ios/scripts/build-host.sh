#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH= cd -- "$ROOT/../.." && pwd)
SDK=${1:-iphonesimulator}
CONFIGURATION=${2:-Debug}
case "$SDK" in
    iphonesimulator) DESTINATION="generic/platform=iOS Simulator"; RUST_TARGET=x86_64-apple-ios ;;
    iphoneos) DESTINATION="generic/platform=iOS"; RUST_TARGET=aarch64-apple-ios ;;
    *) exit 2 ;;
esac
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode-16.4.0.app/Contents/Developer}
sh "$ROOT/scripts/build-rust.sh" "$RUST_TARGET" "$CONFIGURATION"
xcodebuild -project "$ROOT/TapHLE.xcodeproj" -scheme TapHLE -configuration "$CONFIGURATION" -sdk "$SDK" -destination "$DESTINATION" -derivedDataPath "$REPO/build/ios-$SDK" CODE_SIGNING_ALLOWED=NO build
