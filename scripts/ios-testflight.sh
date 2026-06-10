#!/usr/bin/env bash
#
# Build the Tesela iOS/iPad app and upload it to TestFlight.
#
# ONE universal build covers iPhone AND iPad (TARGETED_DEVICE_FAMILY "1,2").
# Mirrors the seedkeep-ios / hermes-voice release pattern: an auth-key archive
# + `xcodebuild -exportArchive` with `destination: upload` (ExportOptions.plist)
# uploads straight to App Store Connect / TestFlight.
#
# App Store Connect API auth — an ACCOUNT-LEVEL key (same one the other apps
# use); override any of these via env:
#   ASC_API_KEY_PATH   (default ~/.appstoreconnect/AuthKey_J79935N6P6.p8)
#   ASC_API_KEY_ID     (default J79935N6P6)
#   ASC_API_ISSUER_ID  (default fe27785a-1413-46ff-bd82-111de0da024f)
#
# One-time: the App Store Connect app record for `app.tesela.ios` must exist
# (Apps -> + -> New App), and the Paid/Free agreements must be accepted.
#
# Run:
#   scripts/ios-testflight.sh              # build -> archive -> upload to TestFlight
#   scripts/ios-testflight.sh --no-upload  # stop after the archive (verify the build)
#
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
IOS="$ROOT/app/Tesela-iOS"
SCHEME="Tesela"
PROJECT="$IOS/Tesela-iOS.xcodeproj"
OUT="$ROOT/build/ios"
ARCHIVE="$OUT/Tesela.xcarchive"
EXPORT="$OUT/export"
INFO="$IOS/Info.plist"

ASC_KEY_PATH="${ASC_API_KEY_PATH:-$HOME/.appstoreconnect/AuthKey_J79935N6P6.p8}"
ASC_KEY_ID="${ASC_API_KEY_ID:-J79935N6P6}"
ASC_ISSUER="${ASC_API_ISSUER_ID:-fe27785a-1413-46ff-bd82-111de0da024f}"

NO_UPLOAD=0
[[ "${1:-}" == "--no-upload" ]] && NO_UPLOAD=1
if [[ "$NO_UPLOAD" == 0 && ! -f "$ASC_KEY_PATH" ]]; then
  echo "ASC API key not found at $ASC_KEY_PATH — set ASC_API_KEY_PATH (or use --no-upload)." >&2
  exit 1
fi

echo "==> 1/6  Rust FFI static lib (aarch64-apple-ios, release)"
cargo build --release -p tesela-sync-ffi --target aarch64-apple-ios

echo "==> 2/6  FFI binding drift check (regenerate + diff — stale bindings abort the release)"
scripts/check-ffi-drift.sh

echo "==> 3/6  resolve SwiftPM packages (+ heal the SwiftWhisper submodule if it flakes)"
# SwiftWhisper pulls a `whisper.cpp` git submodule that SwiftPM sometimes fails
# to clone (a CWD/tmp-pack race). If resolution fails, init the submodule by
# hand in the checkout and retry — then SwiftPM accepts it.
if ! xcodebuild -resolvePackageDependencies -project "$PROJECT" -scheme "$SCHEME" >/dev/null 2>&1; then
  SW="$(ls -d "$HOME"/Library/Developer/Xcode/DerivedData/Tesela-iOS-*/SourcePackages/checkouts/SwiftWhisper 2>/dev/null | head -1)"
  [[ -n "$SW" ]] && git -C "$SW" submodule update --init --recursive || true
  xcodebuild -resolvePackageDependencies -project "$PROJECT" -scheme "$SCHEME" >/dev/null 2>&1 || true
fi

echo "==> 4/6  unit tests (TeselaTests, iOS Simulator) — a red sync-logic test aborts the release"
# The test host builds for the simulator, so it links the SIM static lib —
# build it alongside the device one (step 1) so both stay fresh.
cargo build --release -p tesela-sync-ffi --target aarch64-apple-ios-sim
# Regenerate the project so test files added since the last `xcodegen` are
# in the test target (project.yml is the source of truth; the .xcodeproj
# is gitignored).
if command -v xcodegen >/dev/null 2>&1; then
  (cd "$IOS" && xcodegen generate >/dev/null)
fi
# First available iOS simulator (override with TESELA_TEST_SIM_UDID).
SIM_UDID="${TESELA_TEST_SIM_UDID:-$(xcrun simctl list devices available \
  | awk '/^-- iOS/{ios=1; next} /^--/{ios=0} ios' \
  | grep -Eo -m1 '[0-9A-F]{8}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{4}-[0-9A-F]{12}')}"
[[ -n "$SIM_UDID" ]] || { echo "no available iOS simulator found — create one in Xcode or set TESELA_TEST_SIM_UDID" >&2; exit 1; }
# `set -e` makes a failing test suite abort here, before the archive.
xcodebuild test \
  -project "$PROJECT" -scheme "$SCHEME" \
  -destination "platform=iOS Simulator,id=$SIM_UDID"

echo "==> 5/6  stamp a unique build number + archive (Release, generic iOS)"
BUILDNO="$(date +%Y%m%d%H%M)"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILDNO" "$INFO"
echo "         CFBundleVersion = $BUILDNO  (CFBundleShortVersionString unchanged)"
mkdir -p "$OUT"
/bin/rm -rf "$ARCHIVE"
xcodebuild archive \
  -project "$PROJECT" -scheme "$SCHEME" -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE" \
  -allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_ISSUER"
[[ -d "$ARCHIVE" ]] || { echo "archive failed — $ARCHIVE not created" >&2; exit 1; }

if [[ "$NO_UPLOAD" == 1 ]]; then
  echo "==> --no-upload: archive is at $ARCHIVE"
  exit 0
fi

echo "==> 6/6  export + upload to TestFlight"
/bin/rm -rf "$EXPORT"
xcodebuild -exportArchive \
  -archivePath "$ARCHIVE" \
  -exportPath "$EXPORT" \
  -exportOptionsPlist "$IOS/ExportOptions.plist" \
  -allowProvisioningUpdates \
  -authenticationKeyPath "$ASC_KEY_PATH" \
  -authenticationKeyID "$ASC_KEY_ID" \
  -authenticationKeyIssuerID "$ASC_ISSUER"

echo "==> done — App Store Connect is processing the build; it appears under"
echo "    TestFlight in a few minutes."
