#!/usr/bin/env bash
# Two-mosaic, one-relay end-to-end smoke test for the Tesela sync
# relay. Run from the repo root:  crates/tesela-relay/scripts/smoke.sh
#
# Verifies: an edit on mosaic A propagates through the relay to
# mosaic B without any LAN discovery. If this fails, the pipeline
# (RelayClient + AEAD + Axum relay + apply path) is broken on your
# system.
#
# Layout:
#   relay      → 127.0.0.1:18484, sqlite at $TMP/relay.sqlite
#   server-A   → 127.0.0.1:18001, mosaic at $TMP/mosaic-a
#   server-B   → 127.0.0.1:18002, mosaic at $TMP/mosaic-b
#
# Both servers are configured to poll the relay every 2 s; the script
# waits up to 30 s for propagation.

set -euo pipefail

# ── Config ──────────────────────────────────────────────────────────
RELAY_PORT=18484
SERVER_A_PORT=18001
SERVER_B_PORT=18002
ADMIN_TOKEN="smoke-test-admin-$$"
TMP="${TMPDIR:-/tmp}/tesela-smoke-$$"
PROPAGATION_TIMEOUT_SECS=30

# ── Logging helpers ─────────────────────────────────────────────────
log()  { printf '\033[2m[smoke]\033[0m %s\n' "$*" >&2; }
ok()   { printf '\033[32m[smoke] ✓ %s\033[0m\n' "$*" >&2; }
fail() { printf '\033[31m[smoke] ✗ %s\033[0m\n' "$*" >&2; }

# ── Cleanup on exit ─────────────────────────────────────────────────
RELAY_PID=""
SERVER_A_PID=""
SERVER_B_PID=""
trap cleanup EXIT INT TERM
cleanup() {
    local rc=$?
    log "cleaning up…"
    for pid in $RELAY_PID $SERVER_A_PID $SERVER_B_PID; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
            wait "$pid" 2>/dev/null || true
        fi
    done
    if [[ $rc -eq 0 ]]; then
        rm -rf "$TMP"
    else
        log "left logs + state in $TMP for inspection"
    fi
    exit $rc
}

# ── Prereqs ─────────────────────────────────────────────────────────
for tool in cargo curl jq; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        fail "missing dependency: $tool"
        exit 1
    fi
done

# Ports must be free.
for port in $RELAY_PORT $SERVER_A_PORT $SERVER_B_PORT; do
    if lsof -ti ":$port" >/dev/null 2>&1; then
        fail "port $port is in use — close whatever's bound to it and rerun"
        exit 1
    fi
done

mkdir -p "$TMP"
log "scratch dir: $TMP"

# ── Build (release for speed of the test loop) ──────────────────────
log "building release binaries…"
cargo build -q --release --bin tesela-relay --bin tesela-server

RELAY_BIN="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)/release/tesela-relay"
SERVER_BIN="$(cargo metadata --format-version 1 --no-deps | jq -r .target_directory)/release/tesela-server"

# ── Start the relay ─────────────────────────────────────────────────
log "starting relay on 127.0.0.1:$RELAY_PORT"
TESELA_RELAY_BIND="127.0.0.1:$RELAY_PORT" \
TESELA_RELAY_DB="$TMP/relay.sqlite" \
TESELA_RELAY_ADMIN_TOKEN="$ADMIN_TOKEN" \
RUST_LOG="info,tesela_relay=info" \
    "$RELAY_BIN" >"$TMP/relay.log" 2>&1 &
RELAY_PID=$!

# Wait for relay readiness.
for _ in $(seq 1 30); do
    if curl -fs "http://127.0.0.1:$RELAY_PORT/" >/dev/null 2>&1; then
        break
    fi
    sleep 0.5
done
if ! curl -fs "http://127.0.0.1:$RELAY_PORT/" >/dev/null; then
    fail "relay never became healthy — see $TMP/relay.log"
    exit 1
fi
log "relay healthy"

# ── Set up two mosaics ──────────────────────────────────────────────
make_mosaic() {
    local dir="$1"
    local port="$2"
    mkdir -p "$dir/.tesela" "$dir/notes"
    cat > "$dir/.tesela/config.toml" <<EOF
[server]
bind = "127.0.0.1:$port"

[sync.relay]
url = "http://127.0.0.1:$RELAY_PORT"
poll_interval_ms = 2000
EOF
}
make_mosaic "$TMP/mosaic-a" "$SERVER_A_PORT"
make_mosaic "$TMP/mosaic-b" "$SERVER_B_PORT"
log "mosaics seeded (A=$TMP/mosaic-a, B=$TMP/mosaic-b)"

# ── Start both servers ──────────────────────────────────────────────
start_server() {
    local name="$1"
    local mosaic="$2"
    local logfile="$3"
    local port="$4"
    # TESELA_SERVER_BIND wins over per-mosaic config (`resolve_bind_addr`
    # only reads the global config today — separate bug; tracked in
    # current-state). TESELA_DISABLE_MDNS so the two servers DON'T
    # discover each other over the LAN and sync directly, which would
    # mask whether the relay actually carried the edit.
    TESELA_SERVER_BIND="127.0.0.1:$port" \
    TESELA_DISABLE_MDNS=1 \
    RUST_LOG="info,tesela_server=info,tesela_sync=info" \
        "$SERVER_BIN" --mosaic "$mosaic" >"$logfile" 2>&1 &
    local pid=$!
    log "server-$name pid $pid (mosaic $mosaic, port $port)"
    echo $pid
}
SERVER_A_PID=$(start_server "A" "$TMP/mosaic-a" "$TMP/server-a.log" "$SERVER_A_PORT")
SERVER_B_PID=$(start_server "B" "$TMP/mosaic-b" "$TMP/server-b.log" "$SERVER_B_PORT")

wait_for_server() {
    local port="$1"
    local label="$2"
    for _ in $(seq 1 60); do
        if curl -fs "http://127.0.0.1:$port/health" >/dev/null 2>&1; then
            log "server-$label healthy"
            return 0
        fi
        sleep 0.5
    done
    fail "server-$label never became healthy — see $TMP/server-$(echo "$label" | tr A-Z a-z).log"
    exit 1
}
wait_for_server $SERVER_A_PORT "A"
wait_for_server $SERVER_B_PORT "B"

# ── Pair B with A ───────────────────────────────────────────────────
log "asking server-A for a pairing code…"
PAIR_CODE=$(curl -fs "http://127.0.0.1:$SERVER_A_PORT/sync/peer/pairing-code" | jq -r '.code')
if [[ -z "$PAIR_CODE" || "$PAIR_CODE" == "null" ]]; then
    fail "couldn't get pairing code from server-A — see $TMP/server-a.log"
    exit 1
fi

log "feeding code to server-B…"
PAIR_RESULT=$(
    curl -fs -X POST "http://127.0.0.1:$SERVER_B_PORT/sync/peer/pair-code" \
        -H 'Content-Type: application/json' \
        -d "{\"code\":\"$PAIR_CODE\"}"
)
log "pair-code result: $PAIR_RESULT"

# Give the daemons a moment to (re-)register against the relay under
# the now-shared group key.
sleep 3

# Sanity: both should be configured + registered.
for label in A B; do
    port_var="SERVER_${label}_PORT"
    port="${!port_var}"
    status=$(curl -fs "http://127.0.0.1:$port/sync/relay/status" || echo "{}")
    configured=$(echo "$status" | jq -r '.configured')
    registered=$(echo "$status" | jq -r '.registered_at')
    if [[ "$configured" != "true" ]]; then
        fail "server-$label is not configured for relay (status: $status)"
        exit 1
    fi
    if [[ "$registered" == "null" ]]; then
        fail "server-$label has not registered against the relay yet (status: $status)"
        exit 1
    fi
done
log "both servers registered with relay"

# ── Write an edit on A ──────────────────────────────────────────────
EDIT_BODY=$(cat <<'EOF'
---
title: "smoke-test"
tags: ["smoke"]
---

hello from mosaic A — if you see this on B, relay sync works
EOF
)
log "writing /notes/smoke-test on mosaic A"
curl -fs -X POST "http://127.0.0.1:$SERVER_A_PORT/notes" \
    -H 'Content-Type: application/json' \
    -d "$(jq -n --arg title "smoke-test" --arg content "$EDIT_BODY" \
        '{title: $title, content: $content}')" >/dev/null

# ── Wait for propagation ────────────────────────────────────────────
log "waiting ≤${PROPAGATION_TIMEOUT_SECS}s for edit to appear on mosaic B…"
elapsed=0
got_body=""
while (( elapsed < PROPAGATION_TIMEOUT_SECS )); do
    if got_body=$(curl -fs "http://127.0.0.1:$SERVER_B_PORT/notes/smoke-test" 2>/dev/null | jq -r .body 2>/dev/null); then
        if [[ -n "$got_body" && "$got_body" != "null" ]]; then
            if echo "$got_body" | grep -q "hello from mosaic A"; then
                ok "edit visible in mosaic B (took ${elapsed}s)"
                ok "OK: relay end-to-end works"
                exit 0
            fi
        fi
    fi
    sleep 1
    elapsed=$((elapsed + 1))
done

fail "edit never appeared in mosaic B after ${PROPAGATION_TIMEOUT_SECS}s"
log "last B response: $got_body"
log ""
log "relay status A: $(curl -fs http://127.0.0.1:$SERVER_A_PORT/sync/relay/status || echo '<unreachable>')"
log "relay status B: $(curl -fs http://127.0.0.1:$SERVER_B_PORT/sync/relay/status || echo '<unreachable>')"
log ""
log "tail of server-A log:"
tail -n 30 "$TMP/server-a.log" >&2 || true
log "tail of server-B log:"
tail -n 30 "$TMP/server-b.log" >&2 || true
log "tail of relay log:"
tail -n 30 "$TMP/relay.log" >&2 || true
exit 1
