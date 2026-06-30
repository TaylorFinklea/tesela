#!/usr/bin/env bash
#
# Build, codesign, notarize, staple, and zip the Tesela macOS desktop app.
#
# Local release path for the Tauri v2 desktop shell (crate `tesela-desktop`).
# This is intentionally separate from scripts/release.sh, which triggers the
# date-based GitHub Actions release workflow.
#
# App bundle defaults (override via env):
#   DESKTOP_APP_PATH        (default src-tauri/target/release/bundle/macos/Tesela.app)
#   DESKTOP_DIST_DIR        (default dist/desktop)
#   DESKTOP_ZIP_PATH        (default dist/desktop/Tesela.app.zip)
#
# Codesigning (override via env):
#   DESKTOP_SIGN_IDENTITY   Developer ID Application identity name/hash
#   DESKTOP_SIGN_ENTITLEMENTS optional entitlements plist
#
# App Store Connect API auth — mirrors scripts/ios-testflight.sh. The desktop-
# specific variables win; otherwise the shared ASC_* variables are accepted.
#   DESKTOP_ASC_API_KEY_PATH   or ASC_API_KEY_PATH
#   DESKTOP_ASC_API_KEY_ID     or ASC_API_KEY_ID
#   DESKTOP_ASC_API_ISSUER_ID  or ASC_API_ISSUER_ID
#
# Run:
#   scripts/desktop-release.sh                 # build/sign/zip/notarize/staple/zip
#   scripts/desktop-release.sh --skip-notarize # plan/build-if-present/sign-if-possible/zip, no notary
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

PRODUCT_NAME="${DESKTOP_PRODUCT_NAME:-Tesela}"
BUNDLE_ID="${DESKTOP_BUNDLE_ID:-app.tesela.desktop}"
# src-tauri is a workspace member, so `cargo tauri build` emits to the WORKSPACE
# target ($REPO_ROOT/target), NOT src-tauri/target. Pointing at the latter made
# the post-build re-check fail → "no distributable" exit 0 despite a good build.
DEFAULT_APP_BUNDLE="$REPO_ROOT/target/release/bundle/macos/$PRODUCT_NAME.app"
APP_BUNDLE="${DESKTOP_APP_PATH:-$DEFAULT_APP_BUNDLE}"
DIST_DIR="${DESKTOP_DIST_DIR:-$REPO_ROOT/dist/desktop}"
ZIP_PATH="${DESKTOP_ZIP_PATH:-$DIST_DIR/$PRODUCT_NAME.app.zip}"
SIGN_IDENTITY="${DESKTOP_SIGN_IDENTITY:-}"
SIGN_ENTITLEMENTS="${DESKTOP_SIGN_ENTITLEMENTS:-}"

if [[ -n "${HOME:-}" ]]; then
  DEFAULT_ASC_KEY_PATH="$HOME/.appstoreconnect/AuthKey_J79935N6P6.p8"
else
  DEFAULT_ASC_KEY_PATH=""
fi
ASC_KEY_PATH="${DESKTOP_ASC_API_KEY_PATH:-${ASC_API_KEY_PATH:-$DEFAULT_ASC_KEY_PATH}}"
ASC_KEY_ID="${DESKTOP_ASC_API_KEY_ID:-${ASC_API_KEY_ID:-J79935N6P6}}"
ASC_ISSUER="${DESKTOP_ASC_API_ISSUER_ID:-${ASC_API_ISSUER_ID:-fe27785a-1413-46ff-bd82-111de0da024f}}"

SKIP_NOTARIZE=false
for arg in "$@"; do
  case "$arg" in
    --skip-notarize) SKIP_NOTARIZE=true ;;
    --help|-h)
      echo "Usage: scripts/desktop-release.sh [--skip-notarize]"
      echo ""
      echo "  Build/sign/zip/notarize/staple the Tauri macOS app into a ZIP."
      echo "  --skip-notarize  Do not require build/signing/notary inputs; skip notarytool/stapler."
      exit 0
      ;;
    *) echo "Unknown flag: $arg" >&2; exit 1 ;;
  esac
done

warn() {
  echo "WARNING: $*" >&2
}

zip_app() {
  echo "==> zip app bundle"
  mkdir -p "$DIST_DIR"
  /bin/rm -f "$ZIP_PATH"
  ditto -c -k --keepParent "$APP_BUNDLE" "$ZIP_PATH"
  echo "    wrote $ZIP_PATH"
}

echo "=== Tesela Desktop Release ==="
echo "    product:      $PRODUCT_NAME"
echo "    bundle id:    $BUNDLE_ID"
echo "    app bundle:   $APP_BUNDLE"
echo "    zip:          $ZIP_PATH"
echo "    notarization: $([[ "$SKIP_NOTARIZE" == true ]] && echo skipped || echo enabled)"

APP_AVAILABLE=false
echo "==> 1/5  Tauri app bundle"
if [[ -d "$APP_BUNDLE" ]]; then
  echo "    using pre-built app bundle"
  APP_AVAILABLE=true
elif [[ "$SKIP_NOTARIZE" == true ]]; then
  warn "app bundle not found; --skip-notarize mode does not require a real build, so build/sign/zip are skipped"
else
  echo "    app bundle not found; building web frontend then cargo tauri build"
  # cargo tauri build does NOT rebuild the web (frontendDist=../web/build, no
  # beforeBuildCommand) — rebuild it first or we bundle stale web. See build-desktop.sh.
  ( cd "$REPO_ROOT/web" && npm run build )
  cargo tauri build
  if [[ -d "$APP_BUNDLE" ]]; then
    APP_AVAILABLE=true
  else
    warn "cargo tauri build finished but $APP_BUNDLE was not found; skipping sign/zip/notarize"
  fi
fi

if [[ "$APP_AVAILABLE" != true ]]; then
  echo "==> done — plan validated; no distributable was produced"
  exit 0
fi

SIGNED=false
echo "==> 2/5  codesign hardened runtime"
if [[ -z "$SIGN_IDENTITY" ]]; then
  warn "DESKTOP_SIGN_IDENTITY is unset; skipping codesign"
  if codesign --verify --deep --strict "$APP_BUNDLE" >/dev/null 2>&1; then
    echo "    existing signature verifies"
    SIGNED=true
  else
    warn "existing signature does not verify; notarization will be skipped unless --skip-notarize was requested"
  fi
else
  SIGN_ARGS=(--force --options runtime --timestamp)
  if [[ -n "$SIGN_ENTITLEMENTS" ]]; then
    SIGN_ARGS+=(--entitlements "$SIGN_ENTITLEMENTS")
  fi
  SIGN_ARGS+=(--sign "$SIGN_IDENTITY" "$APP_BUNDLE")
  codesign "${SIGN_ARGS[@]}"
  codesign --verify --deep --strict "$APP_BUNDLE"
  SIGNED=true
fi

# notarytool submits a ZIP, so create it before submission. If stapling succeeds,
# the ZIP is refreshed so the final distributable contains the stapled ticket.
echo "==> 3/5  create distributable ZIP"
zip_app

if [[ "$SKIP_NOTARIZE" == true ]]; then
  echo "==> --skip-notarize: notarytool and stapler skipped"
  echo "==> done — ZIP is at $ZIP_PATH"
  exit 0
fi

NOTARY_READY=true
if [[ "$SIGNED" != true ]]; then
  warn "app is not signed; skipping notarization"
  NOTARY_READY=false
fi
if [[ -z "$ASC_KEY_PATH" || ! -f "$ASC_KEY_PATH" ]]; then
  warn "ASC API key not found; set DESKTOP_ASC_API_KEY_PATH or ASC_API_KEY_PATH"
  NOTARY_READY=false
fi
if [[ -z "$ASC_KEY_ID" ]]; then
  warn "ASC API key id is empty; set DESKTOP_ASC_API_KEY_ID or ASC_API_KEY_ID"
  NOTARY_READY=false
fi
if [[ -z "$ASC_ISSUER" ]]; then
  warn "ASC issuer id is empty; set DESKTOP_ASC_API_ISSUER_ID or ASC_API_ISSUER_ID"
  NOTARY_READY=false
fi

if [[ "$NOTARY_READY" != true ]]; then
  warn "notary credentials/signature incomplete; leaving unsigned or unnotarized ZIP at $ZIP_PATH"
  echo "==> done — ZIP is at $ZIP_PATH"
  exit 0
fi

echo "==> 4/5  submit ZIP to Apple notary service"
xcrun notarytool submit "$ZIP_PATH" \
  --wait \
  --key "$ASC_KEY_PATH" \
  --key-id "$ASC_KEY_ID" \
  --issuer "$ASC_ISSUER"

echo "==> 5/5  staple ticket and refresh ZIP"
xcrun stapler staple "$APP_BUNDLE"
zip_app

echo "==> done — notarized desktop ZIP is at $ZIP_PATH"
