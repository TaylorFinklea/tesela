# Tesela Sync Relay — Add-on Documentation

## Why run this on Home Assistant?

Home Assistant is already running 24/7 in your home, has stable storage,
supports add-on installation in two clicks, and can front the relay
through its **Cloudflared** / **Nginx Proxy Manager** / **Tailscale**
add-ons for WAN access. No separate VPS, no Cloudflare Worker —
your relay rides on infrastructure you already own.

## Installation

1. Home Assistant → **Settings → Add-ons → Add-on Store**.
2. Top-right ⋮ → **Repositories** → paste
   `https://github.com/TaylorFinklea/tesela` → Add.
3. Top-right ⋮ → **Reload** (forces HA to re-scan the repository
   after Add, and also after any addon-image bump on the maintainer
   side).
4. **Tesela Sync Relay** appears under "Tesela Add-ons" → **Install**.
   HA pulls the prebuilt multi-arch image from
   `ghcr.io/taylorfinklea/tesela-relay:latest` — installs in seconds,
   no on-device compile.

> **Forking?** If you've pushed your own build of the image to *your*
> ghcr.io account, you'll get a `403 denied` on Install until you
> mark the package public:
> `https://github.com/users/<you>/packages/container/tesela-relay/settings`
> → bottom → **Change visibility → Public**. One-time, persists across
> image pushes. Installs from the upstream `TaylorFinklea/tesela` repo
> skip this — that image is already public.

## Configuration

| Option         | Default     | Notes                                              |
|----------------|-------------|----------------------------------------------------|
| `admin_token`  | _empty_     | **Required.** Used for `DELETE /admin/registration/:id` hijack recovery. Generate with `openssl rand -hex 32`. |
| `max_body`     | `16777216`  | Per-PUT body cap in bytes (16 MiB default). Must exceed the largest note's Loro snapshot on the wire — a single doc can't be split across envelopes. |
| `log_level`    | `info`      | One of `trace · debug · info · warn · error`.      |

Save → **Start**. Check the **Log** tab — you should see
`Starting Tesela relay on 0.0.0.0:8484`.

## Exposing the relay

The add-on listens on **8484** inside the container; HA maps it to host
port 8484. Pick whichever reachability path matches your setup:

| Scope             | Approach                                                                |
|-------------------|-------------------------------------------------------------------------|
| LAN only          | Point each Tesela device at `http://homeassistant.local:8484`           |
| Tailnet           | Install HA's **Tailscale** add-on; use the `100.x.x.x` IP               |
| Public, with TLS  | Install **Nginx Proxy Manager** → reverse-proxy to `localhost:8484`     |
| Public, no ports  | Install **Cloudflared** → tunnel `http://localhost:8484` to a CF domain |

The relay itself **must not** terminate TLS — keep it HTTP-only and let
the front carry the cert. This matches the docker-compose deploy story
in `crates/tesela-relay/DEPLOY.md`.

## Wiring up your desktop

On the Mac running `tesela-server`, open the web Settings → Sync →
**WAN Relay → Configuration**. Paste the relay URL (e.g.
`https://relay.your-domain.com` or `http://homeassistant.tailnet.ts.net:8484`),
click **Save**, then **Restart server** when prompted. From then on
every paired device syncs through this relay automatically.

## Updating

1. **Settings → Add-ons → Tesela Sync Relay → Update**.
2. The container pulls the new image; `/data` is preserved.
3. Existing groups + cursors keep working — the wire protocol has a
   versioned `v1` namespace and is forwards-compatible across minor
   releases.

## Threat model recap

The relay sees:

- Group IDs (opaque 16-byte identifiers)
- Device IDs (opaque 16-byte identifiers)
- Sealed ciphertext (AEAD payload)
- Timestamps + sequence numbers

The relay **cannot** see note content, titles, structure, or membership
metadata beyond device IDs that have registered/posted. If the HA host
is compromised, the attacker reads sealed ciphertext but cannot decrypt
without the per-group key (which never touches the relay). They can
register a fresh group ID to squat; your devices detect this via the
signed-intent hijack check, and you recover with
`DELETE /admin/registration/:id` using the admin token.

Full threat-model writeup: see
[`crates/tesela-relay/DEPLOY.md`](https://github.com/TaylorFinklea/tesela/blob/main/crates/tesela-relay/DEPLOY.md).
