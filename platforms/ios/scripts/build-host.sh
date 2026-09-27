#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
REPO=$(CDPATH= cd -- "$ROOT/../.." && pwd)
SDK=${1:-iphonesimulator}
CONFIGURATION=${2:-Debug}
PACKAGE=${3:-}
if [ -n "$PACKAGE" ] && [ "$PACKAGE" != --ipa ]; then
    echo "Usage: $0 [iphonesimulator|iphoneos] [Debug|Release] [--ipa]" >&2
    exit 2
fi
if [ "$PACKAGE" = --ipa ] && [ "$SDK" != iphoneos ]; then
    echo "An IPA requires the iphoneos SDK" >&2
    exit 2
fi
case "$SDK" in
    iphonesimulator) DESTINATION="generic/platform=iOS Simulator"; RUST_TARGET=x86_64-apple-ios ;;
    iphoneos) DESTINATION="generic/platform=iOS"; RUST_TARGET=aarch64-apple-ios ;;
    *) exit 2 ;;
esac
export DEVELOPER_DIR=${DEVELOPER_DIR:-/Applications/Xcode-16.4.0.app/Contents/Developer}
sh "$ROOT/scripts/build-rust.sh" "$RUST_TARGET" "$CONFIGURATION"
xcodebuild -project "$ROOT/TapHLE.xcodeproj" -scheme TapHLE -configuration "$CONFIGURATION" -sdk "$SDK" -destination "$DESTINATION" -derivedDataPath "$REPO/build/ios-$SDK" CODE_SIGNING_ALLOWED=NO build

if [ "$PACKAGE" = --ipa ]; then
    # Carry requested entitlements in the IPA for the sideloading signer.
    # This ad-hoc signature is not a device provisioning profile.
    codesign --force --sign - --entitlements "$ROOT/Config/TapHLE.entitlements" \
        "$REPO/build/ios-iphoneos/Build/Products/$CONFIGURATION-iphoneos/tapHLE.app"
    python3 - "$REPO" "$CONFIGURATION" <<'PYTHON'
import hashlib
import json
import pathlib
import subprocess
import sys
import zipfile

repo = pathlib.Path(sys.argv[1])
profile = sys.argv[2]
app = repo / "build/ios-iphoneos/Build/Products" / (profile + "-iphoneos") / "tapHLE.app"
output = repo / "build/ios-iphoneos" / ("tapHLE-iOS-arm64-" + profile + ".ipa")
with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
    for path in sorted(app.rglob("*")):
        if path.is_file():
            archive.write(path, pathlib.Path("Payload/tapHLE.app") / path.relative_to(app))

def command(*args):
    return subprocess.check_output(args, cwd=repo, text=True).strip()

manifest = {
    "commit": command("git", "rev-parse", "HEAD"),
    "dirty": bool(command("git", "status", "--porcelain")),
    "architecture": "arm64",
    "sdk": command("xcrun", "--sdk", "iphoneos", "--show-sdk-version"),
    "xcode": command("xcodebuild", "-version"),
    "rust": command("rustc", "--version"),
    "host_os": command("sw_vers", "-productVersion"),
    "profile": profile,
    "signing": "ad-hoc; re-sign when sideloading, preserving get-task-allow",
    "sha256": hashlib.file_digest(output.open("rb"), "sha256").hexdigest(),
}
output.with_suffix(".json").write_text(json.dumps(manifest, indent=2) + "\n")
print(output)
print("SHA-256: " + manifest["sha256"])
PYTHON
fi
