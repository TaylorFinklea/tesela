# Tesela Sync Relay — Cloudflare Worker

Zero-knowledge sync relay for Tesela, implemented as a Cloudflare
Worker with Durable Objects for per-group state. **Same wire protocol
as `crates/tesela-relay`** — a desktop or iOS client written against
either implementation works against both.

## Why this exists alongside the Rust self-host

The Rust self-host (Docker / HA addon) is the "I run my own
infrastructure" path. This Cloudflare Worker is the "zero
infrastructure" path:

- **Free for personal use.** CF Workers' free tier covers ~100k
  requests/day — a single-user mosaic with five-second polls makes
  ~17k requests/day, so you sit comfortably inside it.
- **Global edge.** Each group's Durable Object pins to the nearest
  edge to wherever it was first registered, then serves all the
  group's devices from there. Lower latency than a single-region
  self-host.
- **Zero maintenance.** No OS updates, no SSL renewal, no port-
  forwarding, no Docker.

Trade-off: you trust Cloudflare with the *metadata* (group IDs,
device IDs, timestamps, ciphertext sizes). The protocol's zero-
knowledge guarantee means CF still can't decrypt your content — same
properties as the self-host. If that trust isn't acceptable, run the
self-host.

## Status

**Wire-conformant and proven against the Rust relay's own test suite.**
The shared conformance harness in
`crates/tesela-relay/tests/conformance.rs` runs against any base URL
(set `TESELA_RELAY_CONFORMANCE_URL`); pointed at this Worker via
`wrangler dev`, all 17 tests pass — identical coverage to the Rust
self-host:

```sh
cd cloudflare-relay && npx wrangler dev --port 8787 &   # one terminal
TESELA_RELAY_CONFORMANCE_URL=http://127.0.0.1:8787 \
  cargo test -p tesela-relay --test conformance          # → 17 passed
```

That's registration/TOFU, MAC auth + replay window + nonce dedupe,
monotonic ops, durable retention, snapshot-gated compaction, per-IP
rate limiting, body-size cap, cross-group isolation, and admin
recovery — all byte-identical on the wire to the Rust relay. Not yet
deployed to a production Cloudflare account (that's the remaining
`wrangler deploy` + free-tier-fit step).

## Architecture

```
                            ┌─────────────────────────────┐
   client GET/PUT           │ Cloudflare Worker (router)  │
   /groups/{id}/...   ──────▶  /groups/:id → idFromName(id)
                            │                  │           │
                            │                  ▼           │
                            │  ┌──────────────────────────┐│
                            │  │ Durable Object: group X  ││
                            │  │  - SQLite (registration, ││
                            │  │    ops, device_seen,     ││
                            │  │    snapshots, group_meta)││
                            │  │  - in-memory nonce LRU   ││
                            │  │  - in-memory IP ratelimit││
                            │  │  - MAC verify + replay   ││
                            │  └──────────────────────────┘│
                            └─────────────────────────────┘
```

One DO instance per `group_id`. The DO serializes all access (no
concurrency races to handle by hand) and gives us SQLite-backed
durable state without an external database. Nonce LRU is in-memory
in the DO (matches the Rust process's `state.rs` cache); the
5-minute TTL means a DO restart loses replay defence for at most
5 minutes, identical to the self-host's behaviour.

## Deploy

You need a Cloudflare account, Workers enabled, and `wrangler`
installed (`npm install -g wrangler`).

```sh
cd cloudflare-relay
npm install
wrangler login

# Set the admin token used by DELETE /admin/groups/:id/register
wrangler secret put TESELA_RELAY_ADMIN_TOKEN
# (paste a long random string, e.g. `openssl rand -hex 32`)

# Deploy. wrangler.toml's [[migrations]] block creates the GroupDO
# class on first deploy; subsequent deploys are no-ops for storage.
wrangler deploy
```

Your relay is now live at `https://tesela-relay.<your-subdomain>.workers.dev`.
Plug that URL into your desktop's `[sync.relay]` block or your iOS
app's pairing flow, exactly like the self-host.

## Local dev

```sh
npm run dev
# → Worker at http://localhost:8787
# → Durable Object SQLite at .wrangler/state/v3/do/<class>/
```

The dev runtime emulates the DO + SQLite locally so you can iterate
without burning Worker invocations. Note that nonce LRU is per-process,
so a `wrangler dev` restart resets it.

## Wire protocol

Identical to the self-host. See:

- [`crates/tesela-relay/src/handlers.rs`](../crates/tesela-relay/src/handlers.rs)
  for the reference Rust implementation (this Worker mirrors response
  codes, body shapes, and header names exactly).
- [`.docs/ai/phases/2026-05-24-relay-protocol-design.md`](../.docs/ai/phases/2026-05-24-relay-protocol-design.md)
  for the design rationale + auth derivation.

In short: `auth_key = HKDF-SHA256(group_key, salt=group_id, info="tesela-relay-auth-v1")`,
every request other than `/register` carries an HMAC-SHA256 over a
canonical-request string, and the inner envelope is AEAD-sealed with
the group_key client-side so the relay sees only opaque bytes.

## Conformance against the Rust self-host

Both implementations are proven byte-identical on the wire by the SAME
test suite. `crates/tesela-relay/tests/conformance.rs` honors a
`TESELA_RELAY_CONFORMANCE_URL` env var: unset, it spawns the in-process
Rust relay; set, it runs every (pure-HTTP) test against that URL. So
the one suite gates both deployments — point it at `wrangler dev` and
all 17 tests pass (see [Status](#status) for the command).

This is the "one suite, both implementations" guarantee: a client
written against either relay works against both, because the MAC
canonical-request format, body-hash, status codes, and JSON shapes are
verified identical by the Rust client driving both servers.

## Storage limits & headroom

Durable Object SQLite storage on the **free tier is 1 GB per DO** (paid
tiers offer higher limits). The relay is a **durable encrypted replica**
— it RETAINS the full encrypted op log (it's the off-site backup + the
fresh-device bootstrap source), so the log is bounded by **snapshot-
gated compaction**, not by acks: when a device deposits a per-note
snapshot batch via `PUT /snapshot` covering relay-seq N, the relay GCs
ops with `seq <= N` (the snapshot supersedes them). A fresh/wiped device
restores the whole mosaic from `GET /snapshots` + the `GET /ops?since=N`
tail. Steady-state storage is ~one compacted snapshot per note plus the
un-compacted tail.

### Projected headroom at 5,000 notes

For a 5,000-note mosaic (10× a typical personal library):

| Component | Size | Notes |
|-----------|------|-------|
| Snapshots (5k notes × 3 KB avg) | 15 MB | Loro snapshots vary: sparse ~500B, typical ~3KB, large ~50KB |
| Ops tail (post-compaction) | 200 KB | ~500 recent ops × 350B |
| Device tracking | 3 KB | ~20 devices per group |
| SQLite overhead (10%) | 1.5 MB | |
| **Total** | **17 MB** | |
| Free tier cap | 1 GB | |
| **Headroom remaining** | **1,007 MB (98.3% free)** | |

**Conclusion:** The free tier provides 60× headroom at 5k notes. Even
scaling to 50k notes (extreme case), projected usage is ~170 MB,
leaving 85% free. The CF DO-SQLite free tier is **not a constraint** for
personal Tesela use.

## What this Worker does NOT do

- **Push notifications.** When a new op arrives, the Worker doesn't
  notify peers. Devices poll on their own cadence (the iOS RelayTicker
  polls every 5s). APNs / WebPush is its own (deferred) project.
- **Global / cross-group rate limiting.** The Worker replicates the
  Rust self-host's per-IP sliding window (1000 req / 10s → 429), but
  *per Durable Object* (i.e. per group) rather than globally, since DO
  state is per-group. An attacker spreading load across many group IDs
  isn't globally throttled by this — CF's platform-level DDoS / rate
  protection (and the native Rate Limiting binding) is the backstop for
  that. Tightening to a global limit is a deploy-hardening follow-up.
- **Compression.** Payloads are stored as-is. Envelopes are typically
  already compressed (postcard + AEAD ciphertext); double-compressing
  wouldn't help.

## Operating it

- **Logs:** `wrangler tail` for live request/response stream.
- **Hijack recovery:** if a hijacker squats a group_id (a real edge
  case — they'd need to know the group_id ahead of you, which is
  random-generated client-side), nuke their registration via
  `curl -X DELETE -H "Authorization: Bearer $ADMIN_TOKEN" https://your-worker/admin/groups/$ID/register`.
  Legitimate clients then re-register on their next tick.
- **Wipe a group:** the same admin DELETE clears registration + ops
  + device-seen for that group. The next client tick re-registers
  cleanly with current group identity.

## Threat model recap

The relay sees:

- 16-byte group IDs (opaque random)
- 16-byte device IDs (opaque random)
- Per-request HMACs (proof of group membership, no content info)
- AEAD-sealed payload bytes (typically 100B–10KB)
- Timestamps + sequence numbers

The relay **cannot** see:

- Note content, titles, structure, or property metadata
- Who's in the group beyond device IDs that have registered/posted
- Whether two ops belong to the same logical note (each is opaque
  ciphertext)

If Cloudflare itself is the adversary, they read sealed bytes but
cannot decrypt without your `group_key` (which never leaves your
devices). They can also tamper with seq ordering or insert garbage
ops — your devices detect the latter via the signed-intent hijack
check on first connect, and tampered ops just fail to decrypt
client-side and get dropped.
