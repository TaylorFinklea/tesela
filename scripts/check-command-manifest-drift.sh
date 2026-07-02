#!/usr/bin/env bash
#
# Command-manifest drift check (tesela-cib): the iOS palette bundles a
# checked-in COPY of the one canonical manifest,
#
#   web/src/lib/command-manifest.json                       (source of truth,
#                                                              tesela-cmdd.2)
#   app/Tesela-iOS/Sources/Data/CommandManifest.json         (bundled copy)
#
# because Swift has no `include_str!`-equivalent to embed the web file
# directly at compile time the way the Rust `GET /commands` route does
# (`crates/tesela-server/src/routes/commands.rs`). Exits non-zero when the
# copy has drifted from the source — i.e. `web/src/lib/command-manifest.json`
# was regenerated (`npm run generate:commands`) without re-syncing the iOS
# copy. Standalone: bash scripts/check-command-manifest-drift.sh
#
# To FIX drift:
#   cp web/src/lib/command-manifest.json app/Tesela-iOS/Sources/Data/CommandManifest.json
#
# TODO: not yet wired into CI (mirrors the same TODO on check-ffi-drift.sh).
set -euo pipefail
cd "$(dirname "$0")/.."
SRC="web/src/lib/command-manifest.json"
COPY="app/Tesela-iOS/Sources/Data/CommandManifest.json"

if ! diff -u "$SRC" "$COPY"; then
  echo "" >&2
  echo "COMMAND MANIFEST DRIFT — $COPY does not match $SRC" >&2
  echo "Regenerate: cp $SRC $COPY" >&2
  exit 1
fi
echo "    command manifest in sync ($SRC == $COPY)"
