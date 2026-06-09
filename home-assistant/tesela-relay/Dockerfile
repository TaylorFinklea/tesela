# syntax=docker/dockerfile:1.6
#
# Home Assistant addon image for tesela-relay.
#
# Built by .github/workflows/relay-container.yml with the **repo root
# as build context** so the builder stage can see the full workspace.
# The resulting image is pushed to ghcr.io/taylorfinklea/tesela-relay;
# the addon config.yaml pulls it from there via `image:`.
#
# Two stages, both Debian bookworm so we get apt + ca-certificates
# everywhere and a clean multi-arch story (debian images cover both
# linux/amd64 and linux/arm64 transparently).
#
# Note: HA addon images don't *have* to be built FROM the HA base
# image. The base buys you `bashio` (a bash helper that reads
# /data/options.json) and a few S6 conventions, but neither is
# required — we read options.json with jq in run.sh instead, which
# also avoids the BUILD_FROM / TARGETARCH naming mismatch
# (HA uses `aarch64-base-debian`, TARGETARCH is `arm64`).

# ─── 1. builder ──────────────────────────────────────────────────────
# Use the build host's native arch for the Rust compiler — QEMU is
# horribly slow at running rustc. `--platform=$BUILDPLATFORM` keeps
# the builder native; the binary it produces targets $TARGETPLATFORM
# via standard cross-compile triples below.
FROM --platform=$BUILDPLATFORM rust:1.95-bookworm AS builder

ARG TARGETARCH
ARG TARGETPLATFORM

# Install cross-compilation toolchain for aarch64 when building from
# amd64 (the common GitHub Actions runner combo). For native builds
# this is a no-op (gcc is already present).
RUN if [ "$TARGETARCH" = "arm64" ]; then \
        dpkg --add-architecture arm64 \
        && apt-get update \
        && apt-get install -y --no-install-recommends \
            gcc-aarch64-linux-gnu \
            libc6-dev-arm64-cross \
        && rm -rf /var/lib/apt/lists/* \
        && rustup target add aarch64-unknown-linux-gnu; \
    fi

WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
# `src-tauri` is a workspace member (the Tauri desktop crate) outside `crates/`.
# `cargo build --locked` resolves the WHOLE workspace, so its manifest must be
# present even though we only build `--bin tesela-relay` (the desktop crate is
# never compiled here). Omitting it breaks the build with "failed to read
# src-tauri/Cargo.toml". Keep this in sync if more top-level members are added.
COPY src-tauri ./src-tauri

# Pick the cargo target triple from $TARGETARCH and build. `--locked`
# enforces Cargo.lock so no silent dep upgrades sneak in on rebuild.
RUN set -eux; \
    case "$TARGETARCH" in \
        amd64) target=x86_64-unknown-linux-gnu ;; \
        arm64) target=aarch64-unknown-linux-gnu \
            && export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc \
            && export CC_aarch64_unknown_linux_gnu=aarch64-linux-gnu-gcc ;; \
        *) echo "Unsupported TARGETARCH=$TARGETARCH" >&2; exit 1 ;; \
    esac; \
    rustup target add "$target"; \
    cargo build --release --target "$target" --bin tesela-relay --locked; \
    cp "target/$target/release/tesela-relay" /tesela-relay

# ─── 2. runtime ──────────────────────────────────────────────────────
FROM debian:bookworm-slim

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
    io.hass.version="${BUILD_VERSION}" \
    org.opencontainers.image.source="https://github.com/TaylorFinklea/tesela"

# ca-certificates for outbound TLS, jq to parse HA's /data/options.json.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates jq \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /tesela-relay /usr/bin/tesela-relay
COPY home-assistant/tesela-relay/run.sh /run.sh
RUN chmod +x /run.sh

CMD [ "/run.sh" ]
