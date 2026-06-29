#!/usr/bin/env bash
# Install the freshly-built desktop bundle into /Applications, replacing the
# current (running) app. Run AFTER `cargo tauri build --bundles app`.
#
# Authorized for Claude to run perpetually so the /Applications build is always
# the latest (see the allow-rule in .claude/settings.local.json). Safe: verifies
# the source build, quits gracefully, backs up the current install before swap.
#
# NOTE: bringing the new build LIVE (server + relay on :7474) currently still
# needs a human Finder/Dock relaunch — an agent-launched `open` starts the app
# shell but not the embedded server. This script attempts the relaunch and tells
# you if you need to reopen from the Dock.
set -euo pipefail

REPO="/Users/tfinklea/git/tesela"
SRC="$REPO/target/release/bundle/macos/Tesela.app"
DST="/Applications/Tesela.app"
BACKUP="/tmp/Tesela-prev.app"

if [ ! -d "$SRC" ]; then
  echo "ERROR: no built app at $SRC" >&2
  echo "  run: cargo tauri build --bundles app" >&2
  exit 1
fi

echo "→ quitting running Tesela (graceful)…"
osascript -e 'quit app "Tesela"' 2>/dev/null || true
sleep 3

echo "→ backing up current install → $BACKUP"
rm -rf "$BACKUP"
[ -d "$DST" ] && cp -R "$DST" "$BACKUP"

echo "→ installing new build → $DST"
rm -rf "$DST"
cp -R "$SRC" "$DST"
echo "  installed: $(stat -f '%Sm' "$DST/Contents/MacOS/tesela-desktop")"

echo "→ relaunching…"
open "$DST"
sleep 14
# The embedded tesela-server binds 127.0.0.1:0 (a RANDOM port, set in
# src-tauri/src/main.rs), NOT 7474 — so probe whatever port the launched
# process is actually listening on and hit /health there.
PID="$(pgrep -f 'Tesela.app/Contents/MacOS/tesela-desktop' | head -1)"
SERVED=""
if [ -n "$PID" ]; then
  for p in $(lsof -nP -p "$PID" 2>/dev/null | grep LISTEN | grep -oE '127\.0\.0\.1:[0-9]+' | cut -d: -f2 | sort -u); do
    if curl -sS -m 4 "http://127.0.0.1:$p/health" 2>/dev/null | grep -q '"ok"'; then SERVED="$p"; break; fi
  done
fi
if [ -n "$SERVED" ]; then
  echo "✓ server up on :$SERVED — new build is live."
else
  echo "⚠ server not detected — if the window didn't appear, open Tesela from the Dock."
fi
