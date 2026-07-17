#!/usr/bin/env bash
# Full desktop redeploy: rebuild the web frontend, bundle the Tauri app, install.
#
# WHY THIS EXISTS: `cargo tauri build` does NOT rebuild the web — tauri.conf.json
# has frontendDist=../web/build and no beforeBuildCommand, so a bare desktop build
# bundles whatever stale web/build already exists. Web-source changes then silently
# never reach the app (this bit hard 2026-06-29: a correct flag fix shipped stale
# for several rebuild cycles). Always go through this script when web/src changed.
#
# Rust-only change? You can skip the web step, but running this is always safe.
set -euo pipefail

REPO="/Users/tfinklea/git/tesela"
APP_BUNDLE="$REPO/target/release/bundle/macos/Tesela.app"
source "$REPO/scripts/lib/desktop-bundle.sh"

echo "==> 1/3  building web frontend (web/build)…"
( cd "$REPO/web" && npm run build )

# Release updater credentials live in Bitwarden Secrets Manager. Running this
# through `bws-project run -- scripts/build-desktop.sh` injects the TESELA_*
# names; discard ambient Tauri credentials, then map only the BWS names without
# persisting them locally.
unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
if [[ -n "${TESELA_TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY="$TESELA_TAURI_SIGNING_PRIVATE_KEY"
fi
if [[ -n "${TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
fi
unset TESELA_TAURI_SIGNING_PRIVATE_KEY TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD

echo "==> 2/3  bundling desktop app…"
TAURI_CONFIG_ARGS=()
if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" || -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  echo "    updater signing key unavailable; building local app without updater artifacts"
  unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  UPDATER_TAR="$REPO/target/release/bundle/macos/Tesela.app.tar.gz"
  /bin/rm -f "$UPDATER_TAR" "${UPDATER_TAR}.sig"
  TAURI_CONFIG_ARGS+=(--config '{"bundle":{"createUpdaterArtifacts":false}}')
fi
( cd "$REPO" && cargo tauri build --bundles app "${TAURI_CONFIG_ARGS[@]}" )
assert_desktop_web_bundle "$APP_BUNDLE"

echo "==> 3/3  installing + relaunching…"
bash "$REPO/scripts/install-desktop.sh"
