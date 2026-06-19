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

# APNs silent-push (sync durability P3c). Optional: the relay enables push
# only when all four of key_p8/key_id/team_id/bundle_id are set; any unset
# → poll-only, exactly as before. Export only the non-empty ones so the
# relay's clap sees genuinely-unset vars as `None` (an exported empty
# string would NOT be None). apns_key_p8 may be a /data path to the .p8 or
# the inline PEM (the relay auto-detects).
APNS_KEY_P8="$(jq -r '.apns_key_p8 // empty' "$OPTS")"
APNS_KEY_ID="$(jq -r '.apns_key_id // empty' "$OPTS")"
APNS_TEAM_ID="$(jq -r '.apns_team_id // empty' "$OPTS")"
APNS_BUNDLE_ID="$(jq -r '.apns_bundle_id // empty' "$OPTS")"
APNS_HOST="$(jq -r '.apns_host // empty' "$OPTS")"
[[ -n "$APNS_KEY_P8" ]] && export APNS_KEY_P8
[[ -n "$APNS_KEY_ID" ]] && export APNS_KEY_ID
[[ -n "$APNS_TEAM_ID" ]] && export APNS_TEAM_ID
[[ -n "$APNS_BUNDLE_ID" ]] && export APNS_BUNDLE_ID
[[ -n "$APNS_HOST" ]] && export APNS_HOST

echo "[tesela-relay] starting on $TESELA_RELAY_BIND" >&2
echo "[tesela-relay] db=$TESELA_RELAY_DB  max-body=$MAX_BODY  log=$LOG_LEVEL" >&2
if [[ -n "$APNS_KEY_ID" && -n "$APNS_TEAM_ID" && -n "$APNS_BUNDLE_ID" && -n "$APNS_KEY_P8" ]]; then
    echo "[tesela-relay] APNs push configured (key id $APNS_KEY_ID, host ${APNS_HOST:-https://api.push.apple.com})" >&2
else
    echo "[tesela-relay] APNs push not configured (poll-only)" >&2
fi

exec /usr/bin/tesela-relay
