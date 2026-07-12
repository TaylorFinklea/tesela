#!/usr/bin/env bash
# Install the freshly-built desktop bundle into /Applications, replacing the
# current (running) app. Run AFTER `cargo tauri build --bundles app`.
#
# Authorized for Claude to run perpetually so the /Applications build is always
# the latest (see the allow-rule in .claude/settings.local.json). Safe: verifies
# the source build, quits gracefully, backs up the current install before swap.
#
# tesela-ejn.3 root cause: an agent-launched `open` used to start the app
# shell but not reliably the embedded server. The old app's graceful shutdown
# drains for up to 30s (auto-backup etc, see `RunEvent::Exit` in
# src-tauri/src/main.rs) while still holding the mosaic's single-writer
# `flock(LOCK_EX|LOCK_NB)`. This script previously slept a fixed 3s before
# swapping+relaunching, which raced that drain: if the old process hadn't
# released the flock yet, the NEW instance's `serve()` failed the
# non-blocking flock immediately, panicking `.build().expect(...)` in
# `main()` before any window ever showed — a silent, headless-invisible
# crash indistinguishable from "the shell started but the server didn't".
# Fix: poll for the old process to fully EXIT (not just receive the quit
# request) before swapping the bundle, and poll for the new server's
# /health instead of a single fixed-sleep check. The script now fails loudly
# (non-zero exit) if the server never comes up, so the rebuild+relaunch loop
# is self-verifying end to end.
set -euo pipefail

REPO="/Users/tfinklea/git/tesela"
SRC="$REPO/target/release/bundle/macos/Tesela.app"
DST="/Applications/Tesela.app"
BACKUP="/tmp/Tesela-prev.app"
BIN_PATTERN='Tesela.app/Contents/MacOS/tesela-desktop'

if [ ! -d "$SRC" ]; then
  echo "ERROR: no built app at $SRC" >&2
  echo "  run: cargo tauri build --bundles app" >&2
  exit 1
fi

echo "→ quitting running Tesela (graceful)…"
osascript -e 'quit app "Tesela"' 2>/dev/null || true

echo "→ waiting for the old process to fully exit (up to 35s, graceful drain)…"
for _ in $(seq 1 35); do
  pgrep -f "$BIN_PATTERN" >/dev/null 2>&1 || break
  sleep 1
done
if pgrep -f "$BIN_PATTERN" >/dev/null 2>&1; then
  echo "⚠ old Tesela process still running after 35s — force-quitting" >&2
  pkill -f "$BIN_PATTERN" 2>/dev/null || true
  sleep 1
fi

echo "→ backing up current install → $BACKUP"
rm -rf "$BACKUP"
[ -d "$DST" ] && cp -R "$DST" "$BACKUP"

echo "→ installing new build → $DST"
rm -rf "$DST"
cp -R "$SRC" "$DST"
if ! codesign --verify --deep --strict "$DST" >/dev/null 2>&1; then
  echo "→ sealing local bundle with an ad-hoc signature…"
  codesign --force --deep --sign - "$DST"
fi
codesign --verify --deep --strict "$DST"
echo "  installed: $(stat -f '%Sm' "$DST/Contents/MacOS/tesela-desktop")"

echo "→ relaunching…"
open "$DST"

# The embedded tesela-server binds 127.0.0.1:0 (a RANDOM port, set in
# src-tauri/src/main.rs), NOT 7474 — so probe whatever port the launched
# process is actually listening on and hit /health there. Poll rather than
# a single fixed-sleep check so a slow-but-successful boot isn't misread.
SERVED=""
for _ in $(seq 1 30); do
  PID="$(pgrep -f "$BIN_PATTERN" | head -1)"
  if [ -n "$PID" ]; then
    for p in $(lsof -nP -p "$PID" 2>/dev/null | grep LISTEN | grep -oE '127\.0\.0\.1:[0-9]+' | cut -d: -f2 | sort -u); do
      if curl -sS -m 4 "http://127.0.0.1:$p/health" 2>/dev/null | grep -q '"ok"'; then SERVED="$p"; break 2; fi
    done
  fi
  sleep 1
done

if [ -n "$SERVED" ]; then
  echo "✓ server up on :$SERVED — new build is live."
else
  echo "✗ server not detected after 30s — the loop is broken (check Console.app for a" >&2
  echo "  tesela-desktop crash/panic); a human Finder/Dock relaunch may be needed." >&2
  exit 1
fi
