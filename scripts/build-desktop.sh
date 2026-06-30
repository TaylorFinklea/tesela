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

echo "==> 2/3  bundling desktop app…"
( cd "$REPO" && cargo tauri build --bundles app )

echo "==> 3/3  installing + relaunching…"
bash "$REPO/scripts/install-desktop.sh"
