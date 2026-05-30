#!/bin/bash
# ──────────────────────────────────────────────────────────────────────
# Tesela Sync Relay — Home Assistant addon entrypoint.
#
# Reads the user-configured options from /data/options.json (HA writes
# this file before exec) and exports them as the env vars the relay
# binary already understands. No hot-reload — changing options in the
# HA UI restarts the addon, which is what we want anyway since the
# relay's auth state is established at boot.
# ──────────────────────────────────────────────────────────────────────

set -euo pipefail

OPTS=/data/options.json
if [[ ! -f "$OPTS" ]]; then
    echo "[tesela-relay] FATAL: $OPTS not found. Is this running outside Home Assistant?" >&2
    exit 1
fi

ADMIN_TOKEN="$(jq -r '.admin_token // empty' "$OPTS")"
MAX_BODY="$(jq -r '.max_body // 16777216' "$OPTS")"
LOG_LEVEL="$(jq -r '.log_level // "info"' "$OPTS")"

if [[ -z "$ADMIN_TOKEN" ]]; then
    echo "[tesela-relay] FATAL: admin_token is empty. Set it in the addon Configuration tab — generate one with: openssl rand -hex 32" >&2
    exit 1
fi

export TESELA_RELAY_ADMIN_TOKEN="$ADMIN_TOKEN"
export TESELA_RELAY_MAX_BODY="$MAX_BODY"
export TESELA_RELAY_BIND="0.0.0.0:8484"
export TESELA_RELAY_DB="/data/relay.sqlite"
# tracing_subscriber reads RUST_LOG; map the friendly enum onto it.
export RUST_LOG="tesela_relay=${LOG_LEVEL},tower_http=${LOG_LEVEL}"

echo "[tesela-relay] starting on $TESELA_RELAY_BIND" >&2
echo "[tesela-relay] db=$TESELA_RELAY_DB  max-body=$MAX_BODY  log=$LOG_LEVEL" >&2

exec /usr/bin/tesela-relay
