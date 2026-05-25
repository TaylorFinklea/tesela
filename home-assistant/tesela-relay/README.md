# Tesela Sync Relay (HA Add-on)

Zero-knowledge sync relay for [Tesela](https://github.com/TaylorFinklea/tesela)
notes, packaged as a Home Assistant add-on. Acts as a store-and-forward
deposit box between your devices — every payload is AEAD-sealed
client-side, so the relay can never read your notes.

See [`DOCS.md`](DOCS.md) for the full install + configuration walkthrough.

## Quick start

1. HA → **Settings → Add-ons → Add-on Store → ⋮ → Repositories**.
2. Paste `https://github.com/TaylorFinklea/tesela`.
3. Install **Tesela Sync Relay** → set `admin_token` → Start.
4. Point your desktop at the relay via **Web Settings → Sync → WAN Relay**.

## What gets stored

| In the container | Outside |
|---|---|
| `/data/relay.sqlite` (registrations + sealed ops + device-seen) | — |
| (in-memory nonce cache; rebuilds on restart)                    | — |

HA persists `/data` across upgrades and snapshots it with your normal backup.
