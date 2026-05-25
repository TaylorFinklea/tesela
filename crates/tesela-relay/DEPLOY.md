# Deploying `tesela-relay`

The relay is a **zero-knowledge fanout** — it stores per-group FIFO
of opaque, AEAD-sealed envelopes that only group-key holders can read.
It's small (single Rust binary + SQLite file), trivial to host, and
designed to be deployable on a $5 Docker host or a Cloudflare Worker
(Worker port lands later — same wire format).

This doc walks you through the self-host path: Docker + an optional
Cloudflare Tunnel so the relay is reachable from your phone over the
cellular network without exposing a public port on your home network.

> **What does the relay do for you?** Lets two of your devices
> sync without both being on the same LAN at the same time. Your Mac
> deposits envelopes; your phone fetches them next time it has
> connectivity. The relay sees only encrypted blobs — your notes are
> never readable from the server.

---

## TL;DR

```sh
# From the repo root
cd crates/tesela-relay
cp .env.example .env
# edit .env and set TESELA_RELAY_ADMIN_TOKEN
docker compose -f docker-compose.yml --env-file .env up -d --build

# verify
curl http://localhost:8484/
# => {"service":"tesela-relay","status":"ok"}
```

Then configure your desktop:

```sh
# in your mosaic's .tesela/config.toml
[sync.relay]
url = "http://my-docker-host.tailnet:8484"
poll_interval_ms = 5000
```

Restart `tesela-server`. Pairing codes generated from this device
now carry the relay URL, so joining devices auto-configure.

---

## What you need

- A host that can run Docker. Anywhere works:
  - Your home server / NAS (Synology, unRAID, etc.)
  - A Raspberry Pi 4+
  - A $5 VPS (DigitalOcean, Linode, Hetzner)
  - Even your Mac, if you're testing
- Outbound connectivity from your devices to the relay
- (Optional) A domain name + Cloudflare account if you want
  cellular-network reachability without exposing a public port

## Step 1 — Build + start the container

```sh
cd crates/tesela-relay
cp .env.example .env
$EDITOR .env  # set TESELA_RELAY_ADMIN_TOKEN to a long random string
docker compose --env-file .env up -d --build
```

The first build takes a few minutes (compiling Rust + dependencies);
subsequent rebuilds reuse the cargo dep cache layer and are seconds.

**The build context is the repo root** (the Dockerfile references
`crates/`). If you've copied just the `crates/tesela-relay/` directory
somewhere, you'll need the rest of the workspace too — easiest path is
clone the repo on your Docker host.

Verify the relay is up:

```sh
curl http://<docker-host>:8484/
# => {"service":"tesela-relay","status":"ok"}
```

## Step 2 — Make it reachable

Three deployment shapes, pick whichever fits how you already operate:

### (a) LAN-only via Tailscale (zero new infra)

If your Mac, your iPhone, and the Docker host are all on a Tailscale
tailnet, you're done. Use the Tailscale IP / MagicDNS name as the relay
URL:

```toml
[sync.relay]
url = "http://docker-host.tailnet:8484"
```

Works at home, at a coffee shop, anywhere your devices have Tailscale
connectivity. No TLS needed — the tailnet provides transport security.
No public DNS / certificates / port forwards.

### (b) Cloudflare Tunnel (no exposed ports, free)

Best for cellular reachability without exposing a port on your home
network. You need a domain on Cloudflare (free tier is fine).

```sh
# On the Docker host
docker run -d --name cloudflared --restart=unless-stopped \
  cloudflare/cloudflared:latest \
  tunnel --no-autoupdate run --token <YOUR_TUNNEL_TOKEN>
```

Create the tunnel + token via [Cloudflare Zero Trust →
Networks → Tunnels](https://one.dash.cloudflare.com/). When prompted
for a service, point it at `http://tesela-relay:8484` (the compose
service name) if you put `cloudflared` on the same compose network,
or `http://<host-lan-ip>:8484` otherwise.

Set up a public hostname like `relay.yourdomain.com → http://tesela-relay:8484`.

Then your relay URL is `https://relay.yourdomain.com`. Cloudflare
handles TLS, certificates, and DDoS for you; no inbound port is open
on your home router.

### (c) Home Assistant add-on (one-click on HA OS)

If you already run Home Assistant, the relay is packaged as an
add-on under [`ha-addon/`](ha-addon/). Two-click install:

1. HA → **Settings → Add-ons → Add-on Store → ⋮ → Repositories**.
2. Paste this repo URL; the **Tesela Sync Relay** add-on appears in
   the store.
3. Install → set `admin_token` in the Configuration tab → Start.

Front it with HA's existing **Cloudflared**, **Nginx Proxy Manager**,
or **Tailscale** add-on for WAN reachability. See
[`ha-addon/README.md`](ha-addon/README.md) for the full walk-through.

### (d) Reverse proxy with your own TLS (Caddy / Traefik)

If you already run a reverse proxy, point a vhost at the relay:

```caddyfile
# Caddyfile
relay.yourdomain.com {
    reverse_proxy localhost:8484
}
```

Caddy will auto-issue Let's Encrypt certificates. Equivalent Traefik
labels work too. Same end result: HTTPS URL pointing at the relay.

## Step 3 — Configure your desktop

The easiest path is the **web UI**: open `tesela-server`'s settings
page → **Sync → WAN Relay → Configuration**, paste the URL, click
**Save**, then **Restart server** when prompted. The settings page
writes `[sync.relay]` into your mosaic's `config.toml` for you and
the live status line lights up green once the relay handshakes.

If you'd rather hand-edit, drop this into the mosaic's
`.tesela/config.toml`:

```toml
[sync.relay]
url = "https://relay.yourdomain.com"
# Optional. Default is 5000 (5 s). Lower = faster sync, more traffic.
poll_interval_ms = 5000
```

Restart `tesela-server`. On startup it'll:

1. Build a `RelayClient` for `(group_id, device_id, group_key)` →
   the relay URL.
2. POST `/register` (idempotent — re-registering with the same
   `auth_key` returns 200).
3. GET `/registration` and verify the stored intent against the local
   `group_key` (the load-bearing **hijack detection** check).
4. Spawn a background poll/produce loop on the configured interval.

You should see in the server logs:

```
INFO  tesela_server: relay: registered + verified at https://relay.yourdomain.com
```

And `GET http://localhost:7474/sync/relay/status` returns the current
relay state (URL, cursors, last poll/put timestamps, last error if
any). The web settings page surfaces this same JSON.

## Step 4 — Pair a second device

Once one device is paired with the relay, joining devices get the
relay URL automatically:

- Generate a pairing code on the relay-configured device:
  `POST /sync/peer/pairing-code` (or the web Settings UI → "Pair a
  device" → "Show code").
- The encoded code now carries `relay_url` as well as the group
  identity.
- The joining device decodes the code, adopts the group identity,
  registers under the same auth_key (deterministic via HKDF), verifies
  the stored intent — and only then trusts the relay.

If the verification fails (impossible without a hijack, since intent
is HMAC-signed under the group key) the joining device surfaces an
error rather than silently sync'ing through a hostile relay.

## Hijack recovery

If you ever see a "relay registration intent does not verify" error,
someone with the `group_id` but not the `group_key` has squatted the
registration on your relay. They cannot read your content (no key) or
impersonate you (no auth_key), but they can stop legitimate
registration. Recovery:

```sh
curl -X DELETE \
  -H "Authorization: Bearer $TESELA_RELAY_ADMIN_TOKEN" \
  https://relay.yourdomain.com/admin/groups/$GROUP_ID_HEX/register
```

Then restart a legitimate device's `tesela-server`. It'll re-register
under the real `auth_key` and the rest of the group catches up.

`GROUP_ID_HEX` is the 32-char hex of your group id. You can find it
in any of your devices' `.tesela/group_id.hex`.

## Operating it

- **Backups.** `tesela-relay-data` is a Docker volume holding the
  SQLite file + WAL. Snapshot it however you snapshot anything else
  (`docker run --rm -v tesela-relay-data:/data -v $(pwd):/backup
  busybox tar czf /backup/relay-data.tgz /data`). Losing it just
  means clients re-register on their next bring-up; cursors reset to
  0 and the next sync round catches everyone up.
- **Logs.** `docker compose logs -f tesela-relay`. Bump
  `RUST_LOG=tesela_relay=debug` for verbose tracing.
- **Disk growth.** Per-op rows are GC'd as soon as every known group
  member acks. A phone offline >30 days gets pruned from the
  known-member set + its backlog is released. Disk should stay small
  (KiB to MiB) for a typical 2–5-device user.
- **Upgrades.** Re-pull the repo on the Docker host, then
  `docker compose up -d --build`. The schema is `CREATE TABLE IF NOT
  EXISTS`; clients don't need to do anything.

## Threat model recap

The relay sees:
- Group ID (16 random bytes)
- Device IDs of every device that's PUT or fetched
- Auth keys (32-byte HKDF-derived, can't recover group_key)
- Signed intent payload (only group-key holders can produce it)
- Envelope sizes + arrival timestamps
- IP addresses of requests

The relay does **NOT** see:
- Any note content
- Any block content
- The group key itself
- Anything inside the encrypted payload

A relay operator can confirm "this group exists, here are its device
fingerprints, here's the traffic volume." They cannot read any note.
This is the **zero-knowledge** guarantee the protocol was designed
around.

For the full protocol spec, see
`.docs/ai/phases/2026-05-24-relay-protocol-design.md` in the repo.
