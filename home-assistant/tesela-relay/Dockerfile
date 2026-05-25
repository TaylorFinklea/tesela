# syntax=docker/dockerfile:1.6
#
# Home Assistant addon image for tesela-relay.
#
# Built by the .github/workflows/relay-container.yml pipeline with the
# **repo root as build context** so the builder stage can see the full
# workspace (workspace Cargo.toml + every crate). The resulting image
# is pushed to ghcr.io/taylorfinklea/tesela-relay; the addon config.yaml
# pulls it from there via `image:`.
#
# Two stages:
#   1. `builder` — Rust 1.83 on debian bookworm. Cross-compiles via
#      buildx + QEMU when targeting linux/arm64 from a linux/amd64 host.
#   2. runtime — debian:bookworm-slim + bashio (vendored from the HA
#      base, but we ship a plain Debian here to keep the image lean
#      and let the BUILD_FROM choice in build.yaml stay flexible).
#      Actually we use the HA base directly so `bashio` is preinstalled
#      and `with-contenv` works as expected.

# ─── 1. builder ──────────────────────────────────────────────────────
FROM rust:1.83-bookworm AS builder
WORKDIR /src

# Copy workspace manifests first so dependency resolution caches
# independently of source edits. With ~20 crates this saves minutes
# per rebuild.
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates

# `--locked` ensures the image matches Cargo.lock exactly — no
# silent dep upgrades sneaking in on a `docker build`.
RUN cargo build --release --bin tesela-relay --locked

# ─── 2. runtime ──────────────────────────────────────────────────────
# Default to amd64 so a local `docker build` works without buildx
# magic. HA's buildx pipeline overrides this per-arch via the
# `build_from:` map in build.yaml.
ARG BUILD_FROM=ghcr.io/home-assistant/amd64-base-debian:bookworm
FROM ${BUILD_FROM}

# Standard HA addon labels — populated by the buildx pipeline.
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

# ca-certificates is the only runtime dep — sqlx bundles SQLite and
# reqwest uses rustls so OpenSSL isn't required.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/tesela-relay /usr/bin/tesela-relay
COPY home-assistant/tesela-relay/run.sh /run.sh
RUN chmod +x /run.sh

CMD [ "/run.sh" ]
