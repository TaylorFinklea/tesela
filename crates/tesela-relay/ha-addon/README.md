# Tesela Sync Relay — Home Assistant Add-on

A zero-knowledge sync relay for [Tesela](https://github.com/tfinklea/tesela)
notes, packaged as a Home Assistant add-on. Acts as a store-and-forward
deposit box between your devices — every payload is AEAD-sealed
client-side, so the relay can never read your notes.

## Why run this on HA?

Home Assistant is already running 24/7 in your home, has stable
storage, supports add-on installation in two clicks, and can front the
relay through its existing ingress / Cloudflared / Nginx Proxy Manager
add-ons for WAN access. No separate VPS, no Cloudflare Worker
deployment — your relay rides on infrastructure you already own.

## Installation

There are two supported paths.

### Option 1 — Add this repo to your add-on store

1. Home Assistant → **Settings → Add-ons → Add-on Store**.
2. Top-right ⋮ → **Repositories**.
3. Paste:

   ```
   https://github.com/tfinklea/tesela
   ```

   (HA scans the repo for any folder containing an addon `config.yaml`
   — it'll discover `crates/tesela-relay/ha-addon/`.)
4. Find **Tesela Sync Relay** in the store and click **Install**.

### Option 2 — Local add-on (developing the relay)

1. Copy `crates/tesela-relay/ha-addon/` into your HA host at
   `/addons/tesela-relay/`. The recommended way is to enable the
   **Samba** add-on and drag the folder into the `addons` share.
2. Home Assistant → **Settings → Add-ons → Add-on Store** → ⋮ →
   **Check for updates**. The add-on appears under *Local add-ons*.
3. Click **Install**. The first build pulls the Rust image + compiles
   the binary (~5–10 minutes on a Raspberry Pi 4).

## Configuration

Open the addon's **Configuration** tab.

| Option         | Default     | Notes                                              |
|----------------|-------------|----------------------------------------------------|
| `admin_token`  | _empty_     | **Required.** Used for `DELETE /admin/registration/:id` hijack recovery. Generate with `openssl rand -hex 32`. |
| `max_body`     | `5242880`   | Per-PUT body cap in bytes (5 MB default).          |
| `log_level`    | `info`      | One of `trace · debug · info · warn · error`.      |

Save → **Start**. Check the **Log** tab — you should see
`Starting Tesela relay on 0.0.0.0:8484`.

## Exposing the relay

The add-on listens on port **8484** inside the container. The
`Network` tab in HA maps it to the host. You then need to make that
port reachable to your devices.

| Reachability scope | Recommended approach |
|---|---|
| LAN only           | Configure desktop / iOS to point at `http://homeassistant.local:8484` |
| Tailnet            | Install Home Assistant's **Tailscale** add-on; use the `100.x.x.x` IP |
| Public internet    | Install **Cloudflared** add-on → tunnel `http://localhost:8484` |
| Public, with TLS   | Install **Nginx Proxy Manager** → reverse-proxy to `localhost:8484` |

The relay itself **must not** terminate TLS — keep it HTTP-only and
let the front carry the cert. This matches the Docker deploy story in
[../DEPLOY.md](../DEPLOY.md).

## Wiring up your desktop

In each Tesela mosaic's web Settings → Sync → **WAN Relay** section,
enter the relay URL (e.g. `https://relay.your-domain.com`) and save.
Restart the server when prompted. From then on, every paired device
will sync through this relay automatically — see
[../DEPLOY.md](../DEPLOY.md) for the pairing flow.

## Data persistence

Everything the relay needs lives under `/data` inside the container,
which HA persists across upgrades:

- `/data/relay.sqlite` — registrations + ops table + device-seen index
- (in-memory nonce cache; not persisted, rebuilds on restart)

Backups are handled automatically by Home Assistant's snapshot system.

## Updating

When a new version of this add-on is pushed:

1. **Settings → Add-ons → Tesela Sync Relay → Update**.
2. The container rebuilds; `/data` is preserved.
3. Existing groups + cursors keep working — the wire protocol has a
   versioned `v1` namespace and is forwards-compatible across minor
   releases.

## Threat model recap

The relay sees:
- Group IDs (opaque 16-byte identifiers)
- Device IDs (opaque 16-byte identifiers)
- Sealed ciphertext (AEAD payload, key-derived headers)
- Timestamps + sequence numbers

The relay **cannot** see:
- Note content
- Note titles
- Block/page structure
- Who's in the group (only device IDs that have registered/posted)

If the host VM is compromised, the attacker can read sealed
ciphertext + tamper with seq ordering, but **cannot decrypt** without
the per-group key (which never touches the relay). They can also
register a fresh group ID to squat — your devices detect this via the
`signed_intent` hijack check on first GET, and the operator recovers
with `DELETE /admin/registration/:id` using the admin token.

See [../DEPLOY.md](../DEPLOY.md) for the full threat model writeup.
