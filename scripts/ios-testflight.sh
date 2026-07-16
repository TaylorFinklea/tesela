#!/usr/bin/env bash
#
# Build the Tesela iOS/iPad app and upload it to TestFlight.
#
# ONE universal build covers iPhone AND iPad (TARGETED_DEVICE_FAMILY "1,2").
# Mirrors the seedkeep-ios / hermes-voice release pattern: an auth-key archive
# + `xcodebuild -exportArchive` with `destination: upload` (ExportOptions.plist)
# uploads straight to App Store Connect / TestFlight.
#
# App Store Connect API auth comes from Tesela's Bitwarden Secrets Manager
# mapping. Run this script through `bws-project run` so these are injected:
#   TESELA_ASC_API_PRIVATE_KEY
#   TESELA_ASC_API_KEY_ID
#   TESELA_ASC_API_ISSUER_ID
# The private key is materialized only in a mode-0700 temporary directory for
# xcodebuild, then removed by the EXIT trap.
#
# One-time: the App Store Connect app record for `app.tesela.ios` must exist
# (Apps -> + -> New App), and the Paid/Free agreements must be accepted.
#
# Run:
#   bws-project run -- scripts/ios-testflight.sh
#   bws-project run -- scripts/ios-testflight.sh --no-upload
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
RELEASE_NOTES="$OUT/release-notes.txt"

NO_UPLOAD=0
case "${1:-}" in
  --no-upload) NO_UPLOAD=1 ;;
  --help|-h)
    echo "Usage: scripts/ios-testflight.sh [--no-upload]"
    echo ""
    echo "  Full release: bws-project run -- scripts/ios-testflight.sh"
    echo "  --no-upload  Build and archive without uploading to TestFlight."
    exit 0
    ;;
  "") ;;
  *) echo "Unknown flag: $1" >&2; exit 1 ;;
esac

for name in \
  TESELA_ASC_API_PRIVATE_KEY \
  TESELA_ASC_API_KEY_ID \
  TESELA_ASC_API_ISSUER_ID
do
  if [[ -z "${!name:-}" ]]; then
    echo "$name is missing — run through: bws-project run -- scripts/ios-testflight.sh" >&2
    exit 1
  fi
done

ASC_KEY_ID="$TESELA_ASC_API_KEY_ID"
ASC_ISSUER="$TESELA_ASC_API_ISSUER_ID"
ASC_CREDENTIAL_DIR="$(mktemp -d /private/tmp/tesela-asc.XXXXXX)"
chmod 700 "$ASC_CREDENTIAL_DIR"
ASC_KEY_PATH="$ASC_CREDENTIAL_DIR/AuthKey_${ASC_KEY_ID}.p8"
printf '%s\n' "$TESELA_ASC_API_PRIVATE_KEY" > "$ASC_KEY_PATH"
chmod 600 "$ASC_KEY_PATH"
unset TESELA_ASC_API_PRIVATE_KEY TESELA_ASC_API_KEY_ID TESELA_ASC_API_ISSUER_ID
cleanup_asc_credentials() {
  rm -rf "$ASC_CREDENTIAL_DIR"
}
trap cleanup_asc_credentials EXIT

echo "==> 1/6  Rust FFI static lib (aarch64-apple-ios, release)"
cargo build --release -p tesela-sync-ffi --target aarch64-apple-ios

echo "==> 2/6  FFI binding drift check (regenerate + diff — stale bindings abort the release)"
scripts/check-ffi-drift.sh
scripts/check-release-notes-drift.sh

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

echo "==> 5/6  stamp the next build number + archive (Release, generic iOS)"
# Plain counter (Taylor, 2026-06-10): previous+1, no timestamps. The
# generated Info.plist carries project.yml's CFBundleVersion (xcodegen ran
# in step 4), so read it, bump it, and persist the bump in BOTH places —
# project.yml is the source of truth that survives the next regen.
PREV="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$INFO")"
BUILDNO=$((PREV + 1))
APP_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$INFO")"
node scripts/changelog.mjs validate --platform ios --version "$APP_VERSION" --build "$BUILDNO"
IOS_RELEASE_ID="$(node -p "require('./release-notes/releases.json').current.ios")"
mkdir -p "$OUT"
node scripts/changelog.mjs render --release "$IOS_RELEASE_ID" --format plain > "$RELEASE_NOTES"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILDNO" "$INFO"
/usr/bin/sed -i '' -E "s/^([[:space:]]*CFBundleVersion:).*/\1 \"$BUILDNO\"/" "$IOS/project.yml"
echo "         version $APP_VERSION (build $BUILDNO)  — commit project.yml + Info.plist after the upload"
echo "         release notes: $RELEASE_NOTES"
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
