# syntax=docker/dockerfile:1.6
#
# Home Assistant addon Dockerfile for tesela-relay.
#
# Build context: the addon directory itself (HA convention). We can't
# COPY from the workspace root, so the builder stage `git clone`s the
# repo at a pin-able ref. This keeps the addon self-contained — users
# install via "Repositories → paste GitHub URL" without needing to
# check out the full workspace into /addons/.
#
# Two stages:
#   1. `builder` — Rust 1.83 on bookworm. Cross-compiles via QEMU when
#      HA's buildx targets a non-host arch (aarch64 on amd64, etc.).
#   2. runtime — the HA per-arch debian base, carries the binary plus
#      ca-certificates. `bashio` is preinstalled on HA bases; the
#      run.sh entrypoint uses it to read addon options.

# ─── 1. builder ──────────────────────────────────────────────────────
FROM rust:1.83-bookworm AS builder

# Pin the source ref. Bump the default when cutting an addon release,
# or override via `--build-arg TESELA_REF=...` in the local addon
# build flow. Shallow clone keeps the layer small.
ARG TESELA_REPO=https://github.com/TaylorFinklea/tesela.git
ARG TESELA_REF=main

RUN apt-get update \
    && apt-get install -y --no-install-recommends git \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /src
RUN git clone --depth 1 --branch "${TESELA_REF}" "${TESELA_REPO}" . \
    && cargo build --release --bin tesela-relay --locked

# ─── 2. runtime ──────────────────────────────────────────────────────
ARG BUILD_FROM=ghcr.io/home-assistant/amd64-base-debian:bookworm
FROM ${BUILD_FROM}

# Standard HA addon labels — populated from build.yaml.
ARG BUILD_ARCH
ARG BUILD_DATE
ARG BUILD_DESCRIPTION
ARG BUILD_NAME
ARG BUILD_REF
ARG BUILD_REPOSITORY
ARG BUILD_VERSION
LABEL \
    io.hass.name="${BUILD_NAME}" \
    io.hass.description="${BUILD_DESCRIPTION}" \
    io.hass.arch="${BUILD_ARCH}" \
    io.hass.type="addon" \
    io.hass.version="${BUILD_VERSION}"

# ca-certificates is the only runtime dep — sqlx bundles SQLite, and
# reqwest uses rustls so OpenSSL isn't required.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/tesela-relay /usr/bin/tesela-relay
COPY run.sh /run.sh
RUN chmod +x /run.sh

# Bind inside the container on 0.0.0.0; HA maps the host port via the
# `ports:` entry in config.yaml. DB lives under /data which HA
# persists across addon upgrades.
ENV TESELA_RELAY_BIND="0.0.0.0:8484"
ENV TESELA_RELAY_DB="/data/relay.sqlite"

CMD [ "/run.sh" ]
