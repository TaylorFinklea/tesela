#!/usr/bin/env bash
#
# Build the Tesela iOS/iPad app and upload it to TestFlight.
#
# ONE universal build covers iPhone AND iPad (TARGETED_DEVICE_FAMILY "1,2").
#
# ── One-time setup (needs your Apple account) ───────────────────────────────
#   1. In App Store Connect, create the app record for bundle id
#      `app.tesela.ios` (Apps → + → New App), and accept any agreements.
#   2. Create an App Store Connect API key:
#        App Store Connect → Users and Access → Integrations → App Store
#        Connect API → "+", role "App Manager" (or Admin). Download the
#        `AuthKey_XXXXXXXXXX.p8` ONCE, and note the Key ID + the Issuer ID
#        (shown at the top of that page).
#   3. Put the key where altool looks for it:
#        mkdir -p ~/.appstoreconnect/private_keys
#        mv ~/Downloads/AuthKey_XXXXXXXXXX.p8 ~/.appstoreconnect/private_keys/
#
# ── Run ─────────────────────────────────────────────────────────────────────
#   ASC_KEY_ID=XXXXXXXXXX ASC_ISSUER_ID=xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx \
#     scripts/ios-testflight.sh
#
#   Pass --no-upload to stop after producing the .ipa (build/ios/export/),
#   e.g. to verify the build without account upload credentials.
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

NO_UPLOAD=0
[[ "${1:-}" == "--no-upload" ]] && NO_UPLOAD=1

echo "==> 1/5  Rust FFI static lib (aarch64-apple-ios, release)"
cargo build --release -p tesela-sync-ffi --target aarch64-apple-ios

echo "==> 2/5  stamp a unique build number"
BUILDNO="$(date +%Y%m%d%H%M)"
/usr/libexec/PlistBuddy -c "Set :CFBundleVersion $BUILDNO" "$INFO"
echo "         CFBundleVersion = $BUILDNO  (CFBundleShortVersionString unchanged)"

echo "==> 3/5  archive (Release, generic iOS device)"
mkdir -p "$OUT"
rm -rf "$ARCHIVE"
xcodebuild archive \
  -project "$PROJECT" -scheme "$SCHEME" -configuration Release \
  -destination 'generic/platform=iOS' \
  -archivePath "$ARCHIVE" \
  -allowProvisioningUpdates

echo "==> 4/5  export for App Store Connect (re-sign with the distribution cert)"
rm -rf "$EXPORT"
xcodebuild -exportArchive \
  -archivePath "$ARCHIVE" \
  -exportOptionsPlist "$IOS/ExportOptions.plist" \
  -exportPath "$EXPORT" \
  -allowProvisioningUpdates
IPA="$(ls "$EXPORT"/*.ipa | head -1)"
echo "         exported: $IPA"

if [[ "$NO_UPLOAD" == 1 ]]; then
  echo "==> 5/5  --no-upload: stopping. The .ipa is at $IPA"
  exit 0
fi

echo "==> 5/5  upload to TestFlight"
: "${ASC_KEY_ID:?set ASC_KEY_ID — the App Store Connect API Key ID}"
: "${ASC_ISSUER_ID:?set ASC_ISSUER_ID — the App Store Connect Issuer ID}"
xcrun altool --upload-app -f "$IPA" -t ios \
  --apiKey "$ASC_KEY_ID" --apiIssuer "$ASC_ISSUER_ID"

echo "==> done — App Store Connect is processing the build; it appears under"
echo "    TestFlight in a few minutes. Add testers / yourself there."
