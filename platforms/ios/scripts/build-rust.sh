#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH= cd -- "$ROOT/../.." && pwd)
TARGET=${1:-x86_64-apple-ios}
CONFIGURATION=${2:-Debug}
case "$TARGET" in
    x86_64-apple-ios) SDK=iphonesimulator; ARCH=x86_64 ;;
    aarch64-apple-ios) SDK=iphoneos; ARCH=arm64 ;;
    *) echo "Usage: $0 [x86_64-apple-ios|aarch64-apple-ios] [Debug|Release]" >&2; exit 2 ;;
esac
case "$CONFIGURATION" in Debug|Release) ;; *) exit 2 ;; esac
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode-16.4.0.app/Contents/Developer}
export SDKROOT=$(xcrun --sdk "$SDK" --show-sdk-path)
export CARGO_TARGET_DIR="$REPO/build/rust-ios"
export TAPHLE_BOOST_ROOT=${TAPHLE_BOOST_ROOT:-"$REPO/vendor/boost"}
export TAPHLE_IOS_ARCH="$ARCH"
export CMAKE_TOOLCHAIN_FILE="$ROOT/cmake/TapHLEiOS.cmake"
export CMAKE_GENERATOR=Ninja
export CMAKE="$ROOT/scripts/cmake-ios.sh"
export IPHONEOS_DEPLOYMENT_TARGET=17.0
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }-C link-arg=-Wl,-undefined,dynamic_lookup"
if [ ! -d "$TAPHLE_BOOST_ROOT/boost" ]; then
    echo "Boost headers were not found at $TAPHLE_BOOST_ROOT" >&2
    exit 1
fi
set -- build --locked --package tapHLE_gui --lib --target "$TARGET" --no-default-features --features ios
if [ "$CONFIGURATION" = Release ]; then set -- "$@" --release; fi
cargo "$@"
