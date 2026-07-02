# Tesela Sync Relay (HA Add-on)

Zero-knowledge sync relay for [Tesela](https://github.com/TaylorFinklea/tesela)
notes, packaged as a Home Assistant add-on. Acts as a store-and-forward
deposit box between your devices — every payload is AEAD-sealed
client-side, so the relay can never read your notes.

> **Relay surface is frozen** (as of 2026-07-01). The relay conforms to a
> stable specification; new features are not accepted. Self-hosters can
> rely on the API remaining backward-compatible. See
> [`crates/tesela-relay/README.md`](https://github.com/TaylorFinklea/tesela/blob/main/crates/tesela-relay/README.md)
> for the frozen surface and change policy.

See [`DOCS.md`](DOCS.md) for the full install + configuration walkthrough.

## Quick start

1. **HA → Settings → Add-ons → Add-on Store → ⋮ → Repositories** —
   paste `https://github.com/TaylorFinklea/tesela`.
2. **Reload the store** (⋮ → Reload) so HA pulls the latest manifest
   and notices the prebuilt image.
3. **Tesela Sync Relay → Install.** Pulls
   `ghcr.io/taylorfinklea/tesela-relay:latest` (multi-arch).
4. **Configuration tab → set `admin_token`** to the output of
   `openssl rand -hex 32` (this is the only thing you must configure).
5. **Start.** The Log tab should show
   `[tesela-relay] starting on 0.0.0.0:8484`.
6. On your Mac, **web Settings → Sync → WAN Relay → Configuration** —
   paste your relay URL (e.g. `http://homeassistant.local:8484` or
   your Tailscale IP), Save, **Restart server**.

### Forking / running your own build

If you've forked this repo and pushed your own build of the addon
image to your account's ghcr.io, you have a one-time step:
visit `https://github.com/users/<you>/packages/container/tesela-relay/settings`
and **Change visibility → Public** so HA can pull without registry
auth. Installs from the upstream `TaylorFinklea/tesela` repo skip
this step — that image is already public.

## What gets stored

| In the container | Outside |
|---|---|
| `/data/relay.sqlite` (registrations + sealed ops + device-seen) | — |
| (in-memory nonce cache; rebuilds on restart)                    | — |

HA persists `/data` across upgrades and snapshots it with your normal backup.
