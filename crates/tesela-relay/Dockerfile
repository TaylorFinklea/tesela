# syntax=docker/dockerfile:1.7
#
# Multi-stage build for the Tesela sync relay.
#
# Stage 1 builds the binary against the workspace; stage 2 ships only
# the binary + minimal runtime so the resulting image is small and
# the attack surface is whatever's in debian:bookworm-slim. The
# binary is statically linked apart from libc.

# ─── Stage 1: build ───────────────────────────────────────────────
FROM rust:1.83-bookworm AS builder
WORKDIR /work

# Pre-fetch deps via a stub binary so the dep cache layer doesn't
# invalidate on every source edit. This is the standard cargo-chef-
# free recipe — slower than cargo-chef but no extra tooling.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# Release build, target only the relay binary so we don't pull in the
# rest of the workspace (tesela-server, tesela-tui, etc).
RUN cargo build --release --bin tesela-relay

# ─── Stage 2: runtime ─────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Need ca-certificates for outbound HTTPS (the relay itself doesn't
# call out, but if you ever front it with a webhook on PUT this is
# already in place). Tini ensures signals propagate cleanly so
# `docker stop` flushes WAL on the way down.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates tini \
    && rm -rf /var/lib/apt/lists/*

# Non-root user. SQLite needs to write into /data so we chown that
# in the entrypoint rather than baking it (lets the operator
# override the volume mount without permission gymnastics).
RUN useradd --system --create-home --home-dir /home/relay --shell /sbin/nologin relay

WORKDIR /app
COPY --from=builder /work/target/release/tesela-relay /usr/local/bin/tesela-relay

# Defaults — operators override via env / compose. The binary reads
# `--bind`, `--db`, `--admin-token`, `--max-body` from env vars
# `TESELA_RELAY_BIND`, `TESELA_RELAY_DB`, etc. (see `src/main.rs`).
ENV TESELA_RELAY_BIND=0.0.0.0:8484 \
    TESELA_RELAY_DB=/data/relay.sqlite \
    TESELA_RELAY_MAX_BODY=1048576
# `TESELA_RELAY_ADMIN_TOKEN` is intentionally NOT set here — operators
# set it via .env / docker secret / compose env. Unset = admin
# endpoints return 404 (still safe; the only effect is no
# hijack-recovery DELETE).

VOLUME ["/data"]
EXPOSE 8484

USER relay
ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/bin/tesela-relay"]

# Healthcheck so orchestrators (docker compose / Swarm / k8s) know
# when the relay is ready. The `/` endpoint returns a tiny JSON
# health blob; non-200 = unhealthy.
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
  CMD wget --quiet --tries=1 --spider http://127.0.0.1:8484/ || exit 1
