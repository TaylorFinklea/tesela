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
# Codesigning comes from Tesela's Bitwarden Secrets Manager mapping:
#   TESELA_DEVELOPER_ID_P12_BASE64
#   TESELA_DEVELOPER_ID_P12_PASSWORD
# The identity is imported into a disposable keychain for this run only.
# `DESKTOP_SIGN_ENTITLEMENTS` remains an optional non-secret plist override.
#
# App Store Connect API auth also comes from Bitwarden:
#   TESELA_ASC_API_PRIVATE_KEY
#   TESELA_ASC_API_KEY_ID
#   TESELA_ASC_API_ISSUER_ID
#
# Auto-update manifest (tesela-ejn.1) — signs the updater artifact that
# `cargo tauri build` emits (src-tauri/tauri.conf.json bundle.
# createUpdaterArtifacts=true) and writes dist/desktop/latest.json for the
# tauri-plugin-updater endpoint (GitHub Releases, see tauri.conf.json
# plugins.updater.endpoints). The private key and password live in Bitwarden:
#   TESELA_TAURI_SIGNING_PRIVATE_KEY
#   TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD
#   DESKTOP_UPDATER_TARGET   (default darwin-aarch64; darwin-x86_64 on Intel)
#   DESKTOP_GH_REPO          (default TaylorFinklea/tesela)
#
# Run:
#   bws-project run -- scripts/desktop-release.sh
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
SIGN_ENTITLEMENTS="${DESKTOP_SIGN_ENTITLEMENTS:-$REPO_ROOT/src-tauri/Entitlements.plist}"

# Ambient Tauri credentials are deliberately ignored. Full releases map only
# the namespaced entries injected by `bws-project`.
unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
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
      echo "  Full release: bws-project run -- scripts/desktop-release.sh"
      echo "  --skip-notarize  Do not require build/signing/notary inputs; skip notarytool/stapler."
      exit 0
      ;;
    *) echo "Unknown flag: $arg" >&2; exit 1 ;;
  esac
done

warn() {
  echo "WARNING: $*" >&2
}

ASC_KEY_PATH=""
ASC_KEY_ID=""
ASC_ISSUER=""
RELEASE_CREDENTIAL_DIR=""
RELEASE_KEYCHAIN=""
RELEASE_KEYCHAIN_PASSWORD=""
SIGN_IDENTITY=""

cleanup_release_credentials() {
  if [[ -n "$RELEASE_KEYCHAIN" ]]; then
    security delete-keychain "$RELEASE_KEYCHAIN" >/dev/null 2>&1 || true
  fi
  if [[ -n "$RELEASE_CREDENTIAL_DIR" ]]; then
    rm -rf "$RELEASE_CREDENTIAL_DIR"
  fi
}
trap cleanup_release_credentials EXIT

require_bws_secret() {
  local name="$1"
  if [[ -z "${!name:-}" ]]; then
    echo "$name is missing — run through: bws-project run -- scripts/desktop-release.sh" >&2
    exit 1
  fi
}

prepare_release_credentials() {
  require_bws_secret TESELA_ASC_API_PRIVATE_KEY
  require_bws_secret TESELA_ASC_API_KEY_ID
  require_bws_secret TESELA_ASC_API_ISSUER_ID
  require_bws_secret TESELA_DEVELOPER_ID_P12_BASE64
  require_bws_secret TESELA_DEVELOPER_ID_P12_PASSWORD
  require_bws_secret TESELA_TAURI_SIGNING_PRIVATE_KEY
  require_bws_secret TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD

  export TAURI_SIGNING_PRIVATE_KEY="$TESELA_TAURI_SIGNING_PRIVATE_KEY"
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD"

  RELEASE_CREDENTIAL_DIR="$(mktemp -d /private/tmp/tesela-desktop-release.XXXXXX)"
  chmod 700 "$RELEASE_CREDENTIAL_DIR"

  ASC_KEY_ID="$TESELA_ASC_API_KEY_ID"
  ASC_ISSUER="$TESELA_ASC_API_ISSUER_ID"
  ASC_KEY_PATH="$RELEASE_CREDENTIAL_DIR/AuthKey_${ASC_KEY_ID}.p8"
  printf '%s\n' "$TESELA_ASC_API_PRIVATE_KEY" > "$ASC_KEY_PATH"
  chmod 600 "$ASC_KEY_PATH"

  local p12_path="$RELEASE_CREDENTIAL_DIR/developer-id.p12"
  printf '%s' "$TESELA_DEVELOPER_ID_P12_BASE64" | /usr/bin/base64 -D > "$p12_path"
  chmod 600 "$p12_path"

  RELEASE_KEYCHAIN="$RELEASE_CREDENTIAL_DIR/tesela-release.keychain-db"
  RELEASE_KEYCHAIN_PASSWORD="$(openssl rand -base64 32)"
  security create-keychain -p "$RELEASE_KEYCHAIN_PASSWORD" "$RELEASE_KEYCHAIN"
  security set-keychain-settings -lut 21600 "$RELEASE_KEYCHAIN"
  security unlock-keychain -p "$RELEASE_KEYCHAIN_PASSWORD" "$RELEASE_KEYCHAIN"
  security import "$p12_path" \
    -k "$RELEASE_KEYCHAIN" \
    -P "$TESELA_DEVELOPER_ID_P12_PASSWORD" \
    -T /usr/bin/codesign \
    -T /usr/bin/security >/dev/null
  security set-key-partition-list \
    -S apple-tool:,apple:,codesign: \
    -s \
    -k "$RELEASE_KEYCHAIN_PASSWORD" \
    "$RELEASE_KEYCHAIN" >/dev/null

  SIGN_IDENTITY="$(security find-identity -v -p codesigning "$RELEASE_KEYCHAIN" \
    | awk '/Developer ID Application/ { print $2; exit }')"
  if [[ -z "$SIGN_IDENTITY" ]]; then
    echo "Bitwarden PKCS#12 does not contain a Developer ID Application identity" >&2
    exit 1
  fi

  unset \
    TESELA_ASC_API_PRIVATE_KEY \
    TESELA_ASC_API_KEY_ID \
    TESELA_ASC_API_ISSUER_ID \
    TESELA_DEVELOPER_ID_P12_BASE64 \
    TESELA_DEVELOPER_ID_P12_PASSWORD \
    TESELA_TAURI_SIGNING_PRIVATE_KEY \
    TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD
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

create_final_updater_artifact() {
  echo "==> create signed updater artifact from stapled app"
  /bin/rm -f "$UPDATER_TAR_PATH" "$UPDATER_SIG_PATH"
  COPYFILE_DISABLE=1 /usr/bin/tar \
    -czf "$UPDATER_TAR_PATH" \
    -C "$(dirname "$APP_BUNDLE")" \
    "$(basename "$APP_BUNDLE")"
  cargo tauri signer sign "$UPDATER_TAR_PATH"
  if [[ ! -f "$UPDATER_SIG_PATH" ]]; then
    echo "Tauri did not write $UPDATER_SIG_PATH" >&2
    exit 1
  fi
}

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
if [[ "$SKIP_NOTARIZE" != true ]]; then
  prepare_release_credentials
fi

echo "=== Tesela Desktop Release ==="
echo "    product:      $PRODUCT_NAME"
echo "    bundle id:    $BUNDLE_ID"
echo "    app bundle:   $APP_BUNDLE"
echo "    zip:          $ZIP_PATH"
echo "    notarization: $([[ "$SKIP_NOTARIZE" == true ]] && echo skipped || echo enabled)"

APP_AVAILABLE=false
echo "==> 1/6  Tauri app bundle"
if [[ "$SKIP_NOTARIZE" == true ]]; then
  if [[ -d "$APP_BUNDLE" ]]; then
    echo "    using pre-built app bundle"
    APP_AVAILABLE=true
  else
    warn "app bundle not found; --skip-notarize mode does not require a real build, so build/sign/zip are skipped"
  fi
else
  echo "    building a fresh web frontend and Tauri app bundle"
  # cargo tauri build does NOT rebuild the web (frontendDist=../web/build, no
  # beforeBuildCommand) — rebuild it first or we bundle stale web. See build-desktop.sh.
  ( cd "$REPO_ROOT/web" && npm run build )
  cargo tauri build --bundles app
  if [[ -d "$APP_BUNDLE" ]]; then
    APP_AVAILABLE=true
  else
    echo "cargo tauri build finished but $APP_BUNDLE was not found" >&2
    exit 1
  fi
fi

if [[ "$APP_AVAILABLE" != true ]]; then
  echo "==> done — plan validated; no distributable was produced"
  exit 0
fi

APP_BUNDLE_VERSION="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$APP_BUNDLE/Contents/Info.plist")"
if [[ "$APP_BUNDLE_VERSION" != "$DESKTOP_VERSION" ]]; then
  if [[ "$SKIP_NOTARIZE" == true ]]; then
    warn "pre-built app version $APP_BUNDLE_VERSION does not match release version $DESKTOP_VERSION"
  else
    echo "app bundle version $APP_BUNDLE_VERSION does not match release version $DESKTOP_VERSION" >&2
    exit 1
  fi
fi

SIGNED=false
echo "==> 2/6  codesign hardened runtime"
if [[ -z "$SIGN_IDENTITY" ]]; then
  warn "release identity is unavailable; leaving the pre-built signature unchanged"
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
  SIGN_ARGS+=(--keychain "$RELEASE_KEYCHAIN" --sign "$SIGN_IDENTITY" "$APP_BUNDLE")
  codesign "${SIGN_ARGS[@]}"
  codesign --verify --deep --strict "$APP_BUNDLE"
  SIGNED=true
fi

# notarytool submits a ZIP, so create it before submission. If stapling succeeds,
# the ZIP is refreshed so the final distributable contains the stapled ticket.
echo "==> 3/6  create distributable ZIP"
zip_app

if [[ "$SKIP_NOTARIZE" == true ]]; then
  emit_updater_manifest
  echo "==> --skip-notarize: notarytool and stapler skipped"
  echo "==> done — ZIP is at $ZIP_PATH"
  exit 0
fi

if [[ "$SIGNED" != true ]]; then
  echo "full release requires a Developer ID signature" >&2
  exit 1
fi

echo "==> 4/6  submit ZIP to Apple notary service"
xcrun notarytool submit "$ZIP_PATH" \
  --wait \
  --key "$ASC_KEY_PATH" \
  --key-id "$ASC_KEY_ID" \
  --issuer "$ASC_ISSUER"

echo "==> 5/6  staple ticket and refresh ZIP"
xcrun stapler staple "$APP_BUNDLE"
xcrun stapler validate "$APP_BUNDLE"
codesign --verify --deep --strict "$APP_BUNDLE"
spctl -a -vv "$APP_BUNDLE"
zip_app
create_final_updater_artifact
emit_updater_manifest

echo "==> done — notarized desktop ZIP is at $ZIP_PATH"
