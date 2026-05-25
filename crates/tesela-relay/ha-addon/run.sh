#!/usr/bin/with-contenv bashio
# ──────────────────────────────────────────────────────────────────────
# Tesela Sync Relay — Home Assistant addon entrypoint.
#
# Reads the user-configured options from /data/options.json via the
# `bashio` helper (preinstalled on HA base images) and exports them as
# the env vars the relay binary already understands. No hot-reload —
# changing options in the HA UI restarts the addon, which is what we
# want anyway since the relay's auth state is established at boot.
# ──────────────────────────────────────────────────────────────────────

set -euo pipefail

ADMIN_TOKEN="$(bashio::config 'admin_token')"
MAX_BODY="$(bashio::config 'max_body')"
LOG_LEVEL="$(bashio::config 'log_level')"

if [[ -z "${ADMIN_TOKEN}" ]]; then
    bashio::log.fatal \
        "admin_token is empty. Set it in the addon Configuration tab — \
generate one with: openssl rand -hex 32"
    exit 1
fi

export TESELA_RELAY_ADMIN_TOKEN="${ADMIN_TOKEN}"
export TESELA_RELAY_MAX_BODY="${MAX_BODY}"
# tracing_subscriber reads RUST_LOG; map the friendly enum onto it.
export RUST_LOG="tesela_relay=${LOG_LEVEL},tower_http=${LOG_LEVEL}"

bashio::log.info "Starting Tesela relay on ${TESELA_RELAY_BIND}"
bashio::log.info "DB: ${TESELA_RELAY_DB} · max-body: ${MAX_BODY} · log: ${LOG_LEVEL}"

exec /usr/bin/tesela-relay
