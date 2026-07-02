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

echo "==> 1/3  building web frontend (web/build)…"
( cd "$REPO/web" && npm run build )

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

echo "==> 2/3  bundling desktop app…"
( cd "$REPO" && cargo tauri build --bundles app )

echo "==> 3/3  installing + relaunching…"
bash "$REPO/scripts/install-desktop.sh"
