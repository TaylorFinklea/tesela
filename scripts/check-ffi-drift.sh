#!/usr/bin/env bash
#
# FFI binding drift check (audit A13): rebuild the FFI dylib, regenerate the
# UniFFI Swift bindings (library mode) into a temp dir, and diff them against
# the checked-in artifacts:
#
#   app/Tesela-iOS/Generated/tesela_sync_ffi.swift
#   app/Tesela-iOS/Generated/tesela_sync_ffiFFI.h
#   app/Tesela-iOS/Generated/tesela_sync_ffiFFI.modulemap
#   app/Tesela-iOS/CFFI/tesela_sync_ffiFFI.h        (synced copy of Generated/)
#   app/Tesela-iOS/CFFI/module.modulemap            (synced copy, renamed)
#
# Exits non-zero on any drift — i.e. crates/tesela-sync-ffi changed without the
# bindings being regenerated. A forgotten regen otherwise surfaces as a
# sim/device runtime failure or SourceKit ghost errors, and (worse) a stale
# binary silently writing legacy wire formats. Run by scripts/ios-testflight.sh
# before every archive; standalone: bash scripts/check-ffi-drift.sh
#
# To FIX drift, regenerate into Generated/ and re-sync CFFI/:
#   cargo build -p tesela-sync-ffi
#   cargo run -p tesela-sync-ffi --features cli --bin uniffi-bindgen -- \
#     generate --library target/debug/libtesela_sync_ffi.dylib \
#     --language swift --out-dir app/Tesela-iOS/Generated
#   perl -pi -e 's/[ \t]+$//' app/Tesela-iOS/Generated/tesela_sync_ffi.swift \
#     app/Tesela-iOS/Generated/tesela_sync_ffiFFI.h
#   cp app/Tesela-iOS/Generated/tesela_sync_ffiFFI.h app/Tesela-iOS/CFFI/tesela_sync_ffiFFI.h
#   cp app/Tesela-iOS/Generated/tesela_sync_ffiFFI.modulemap app/Tesela-iOS/CFFI/module.modulemap
#
# TODO: .github/workflows/ios-smoke.yml can adopt this script as a CI step (CI
# workflow wiring is owned by the parallel CI stream — deliberately not done here).
set -euo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"
GEN="$ROOT/app/Tesela-iOS/Generated"
CFFI="$ROOT/app/Tesela-iOS/CFFI"

echo "==> 1/3  build the FFI dylib (host, debug)"
cargo build -p tesela-sync-ffi

echo "==> 2/3  regenerate Swift bindings (library mode) into a temp dir"
TMP="$(mktemp -d)"
trap '/bin/rm -rf "$TMP"' EXIT
cargo run -p tesela-sync-ffi --features cli --bin uniffi-bindgen -- \
  generate --library "$ROOT/target/debug/libtesela_sync_ffi.dylib" \
  --language swift --out-dir "$TMP"
# UniFFI's Swift templates emit trailing spaces on blank lines and selected
# initializer arguments. Keep generated artifacts whitespace-clean without
# weakening the byte-for-byte drift check.
perl -pi -e 's/[ \t]+$//' \
  "$TMP/tesela_sync_ffi.swift" \
  "$TMP/tesela_sync_ffiFFI.h"

echo "==> 3/3  diff against the checked-in artifacts"
DRIFTED=()
check() {  # check <fresh> <checked-in>
  if ! diff -u "$2" "$1" >/dev/null 2>&1; then
    DRIFTED+=("$2")
    echo "--- drift in $2 (checked-in vs freshly generated, first 40 lines):"
    diff -u "$2" "$1" | head -40 || true
  fi
}
check "$TMP/tesela_sync_ffi.swift"        "$GEN/tesela_sync_ffi.swift"
check "$TMP/tesela_sync_ffiFFI.h"         "$GEN/tesela_sync_ffiFFI.h"
check "$TMP/tesela_sync_ffiFFI.modulemap" "$GEN/tesela_sync_ffiFFI.modulemap"
check "$TMP/tesela_sync_ffiFFI.h"         "$CFFI/tesela_sync_ffiFFI.h"
check "$TMP/tesela_sync_ffiFFI.modulemap" "$CFFI/module.modulemap"

if (( ${#DRIFTED[@]} > 0 )); then
  echo "" >&2
  echo "FFI BINDING DRIFT — checked-in bindings do not match crates/tesela-sync-ffi:" >&2
  printf '  %s\n' "${DRIFTED[@]}" >&2
  echo "Regenerate + sync (commands in the header of scripts/check-ffi-drift.sh), then commit." >&2
  exit 1
fi
echo "    bindings in sync (5 files)"
