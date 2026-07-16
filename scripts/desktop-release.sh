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
# Auto-update manifest (tesela-ejn.1) — signs the updater artifact that
# `cargo tauri build` emits (src-tauri/tauri.conf.json bundle.
# createUpdaterArtifacts=true) and writes dist/desktop/latest.json for the
# tauri-plugin-updater endpoint (GitHub Releases, see tauri.conf.json
# plugins.updater.endpoints). Signing key lives in the macOS Keychain
# (never committed) — see the private-key handling note below the flag
# parsing. Override via env:
#   TAURI_SIGNING_PRIVATE_KEY / TAURI_SIGNING_PRIVATE_KEY_PASSWORD  (win if set)
#   DESKTOP_UPDATER_TARGET   (default darwin-aarch64; darwin-x86_64 on Intel)
#   DESKTOP_GH_REPO          (default TaylorFinklea/tesela)
#
# Run:
#   scripts/desktop-release.sh                 # build/sign/zip/notarize/staple/zip/manifest
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

# Updater signing key. `TAURI_SIGNING_PRIVATE_KEY[_PASSWORD]` win if already
# exported (e.g. CI secrets); otherwise pull from the macOS Keychain items
# this repo's `tesela-ejn.1` setup created (`security add-generic-password`,
# never a file on disk). Missing either just means `cargo tauri build` won't
# emit updater artifacts — the sign/notarize/zip path is unaffected.
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]] && command -v security >/dev/null 2>&1; then
  if TAURI_SIGNING_PRIVATE_KEY="$(security find-generic-password -a "$USER" -s tesela-desktop-updater-key -w 2>/dev/null)"; then
    export TAURI_SIGNING_PRIVATE_KEY
  fi
fi
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]] && command -v security >/dev/null 2>&1; then
  if TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -a "$USER" -s tesela-desktop-updater-key-password -w 2>/dev/null)"; then
    export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  fi
fi
UPDATER_TARGET="${DESKTOP_UPDATER_TARGET:-darwin-aarch64}"
GH_REPO="${DESKTOP_GH_REPO:-TaylorFinklea/tesela}"
UPDATER_TAR_PATH="${APP_BUNDLE}.tar.gz"
UPDATER_SIG_PATH="${UPDATER_TAR_PATH}.sig"
MANIFEST_PATH="$DIST_DIR/latest.json"
RELEASE_NOTES_MD_PATH="$DIST_DIR/release-notes.md"
RELEASE_NOTES_PLAIN_PATH="$DIST_DIR/release-notes.txt"
DESKTOP_VERSION="$(node -p "require('./src-tauri/tauri.conf.json').version")"
DESKTOP_RELEASE_ID="$(node -p "require('./release-notes/releases.json').current.desktop")"

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

prepare_release_notes() {
  echo "==> validate and render desktop release notes"
  mkdir -p "$DIST_DIR"
  node scripts/changelog.mjs validate --platform desktop --version "$DESKTOP_VERSION"
  node scripts/changelog.mjs render --release "$DESKTOP_RELEASE_ID" --format markdown > "$RELEASE_NOTES_MD_PATH"
  node scripts/changelog.mjs render --release "$DESKTOP_RELEASE_ID" --format plain > "$RELEASE_NOTES_PLAIN_PATH"
  echo "    wrote $RELEASE_NOTES_MD_PATH and $RELEASE_NOTES_PLAIN_PATH"
}

zip_app() {
  echo "==> zip app bundle"
  mkdir -p "$DIST_DIR"
  /bin/rm -f "$ZIP_PATH"
  ditto -c -k --keepParent "$APP_BUNDLE" "$ZIP_PATH"
  echo "    wrote $ZIP_PATH"
}

# `cargo tauri build` only emits `$APP_BUNDLE.tar.gz` + `.sig` when
# TAURI_SIGNING_PRIVATE_KEY[_PASSWORD] were set at build time (bundle.
# createUpdaterArtifacts=true in tauri.conf.json makes it try). This step
# just packages whatever it produced into dist/desktop/latest.json — it does
# NOT re-sign or regenerate the tarball, so it's a no-op (with a warning) on
# any build that ran without the signing env set, including a bare
# --skip-notarize dry run with no fresh build.
#
# NOTE: the tarball is the pre-notarize/pre-staple build artifact (`cargo
# tauri build` produces it in step 1, before notarization even runs). An
# app updated from it is still notarized (Apple's servers have the record)
# but not stapled, so Gatekeeper does one online check on first launch of
# the updated app instead of reading a stapled ticket offline. Re-tarring +
# re-signing post-staple to close that gap needs verifying Tauri's exact
# bundler tar layout against a real signed build — left as a follow-up
# rather than guessed at here.
emit_updater_manifest() {
  echo "==> 6/6  updater manifest (latest.json)"
  if [[ ! -f "$UPDATER_TAR_PATH" || ! -f "$UPDATER_SIG_PATH" ]]; then
    warn "no updater artifact at $UPDATER_TAR_PATH (and/or its .sig) — cargo tauri build only" \
         "writes these when TAURI_SIGNING_PRIVATE_KEY[_PASSWORD] are set at build time; skipping manifest"
    return 0
  fi
  mkdir -p "$DIST_DIR"
  local tar_dest="$DIST_DIR/$(basename "$UPDATER_TAR_PATH")"
  cp -f "$UPDATER_TAR_PATH" "$tar_dest"
  local pub_date
  pub_date="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  local asset_name
  asset_name="$(basename "$tar_dest")"
  local download_url="https://github.com/${GH_REPO}/releases/download/v${DESKTOP_VERSION}/${asset_name}"
  node scripts/changelog.mjs updater-manifest \
    --version "$DESKTOP_VERSION" \
    --notes-file "$RELEASE_NOTES_PLAIN_PATH" \
    --pub-date "$pub_date" \
    --target "$UPDATER_TARGET" \
    --signature-file "$UPDATER_SIG_PATH" \
    --url "$download_url" > "$MANIFEST_PATH"
  echo "    wrote $MANIFEST_PATH (target=$UPDATER_TARGET, version=$DESKTOP_VERSION)"
  echo "    publish (creates/updates the GitHub release + attaches assets):"
  echo "      gh release create v${DESKTOP_VERSION} \"$ZIP_PATH\" \"$tar_dest\" \"$MANIFEST_PATH\" --title v${DESKTOP_VERSION} --notes-file \"$RELEASE_NOTES_MD_PATH\""
  echo "      # or, if v${DESKTOP_VERSION} already exists:"
  echo "      gh release edit v${DESKTOP_VERSION} --notes-file \"$RELEASE_NOTES_MD_PATH\""
  echo "      gh release upload v${DESKTOP_VERSION} \"$ZIP_PATH\" \"$tar_dest\" \"$MANIFEST_PATH\" --clobber"
}

prepare_release_notes

echo "=== Tesela Desktop Release ==="
echo "    product:      $PRODUCT_NAME"
echo "    bundle id:    $BUNDLE_ID"
echo "    app bundle:   $APP_BUNDLE"
echo "    zip:          $ZIP_PATH"
echo "    notarization: $([[ "$SKIP_NOTARIZE" == true ]] && echo skipped || echo enabled)"

APP_AVAILABLE=false
echo "==> 1/6  Tauri app bundle"
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
echo "==> 2/6  codesign hardened runtime"
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
echo "==> 3/6  create distributable ZIP"
zip_app
emit_updater_manifest

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

echo "==> 4/6  submit ZIP to Apple notary service"
xcrun notarytool submit "$ZIP_PATH" \
  --wait \
  --key "$ASC_KEY_PATH" \
  --key-id "$ASC_KEY_ID" \
  --issuer "$ASC_ISSUER"

echo "==> 5/6  staple ticket and refresh ZIP"
xcrun stapler staple "$APP_BUNDLE"
zip_app
# NOT re-run here: the updater tarball/signature/manifest emitted in step 3/6
# are pre-staple (see emit_updater_manifest's doc comment) — the ZIP above is
# the only artifact that carries the stapled ticket.

echo "==> done — notarized desktop ZIP is at $ZIP_PATH"
