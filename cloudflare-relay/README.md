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

This is a **port of the Rust relay's wire surface** that has not yet
been deployed against by a real client end-to-end. The protocol
matches the design doc + the Rust implementation; the conformance
test vectors from `crates/tesela-relay/tests/conformance.rs` should
pass against this Worker once someone wires them up. Treat as
beta until that happens.

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
                            │  │    ops, device_seen)     ││
                            │  │  - in-memory nonce LRU   ││
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

Both implementations were written against the same protocol doc, and
the Rust side has a 13-test conformance harness at
[`crates/tesela-relay/tests/conformance.rs`](../crates/tesela-relay/tests/conformance.rs).
A future TODO is to port those tests to run against an arbitrary
relay URL (Rust dev binary OR `wrangler dev`) so we can prove the two
implementations are byte-identical on the wire.

Until that runs green here, treat the Worker as "should be correct,
but not yet proven against the same test vectors."

## Storage limits

Durable Object SQLite storage is currently 1 GB per DO. At realistic
op sizes (~5 KB per envelope) that's room for ~200k unsent ops per
group before the relay refuses inserts. Since the relay GCs ops once
every known group member has acked, real-world usage stays nowhere
near this — a single group typically has < 100 pending ops at any
moment.

## What this Worker does NOT do

- **Push notifications.** When a new op arrives, the Worker doesn't
  notify peers. Devices poll on their own cadence (the iOS RelayTicker
  polls every 5s). APNs / WebPush is its own (deferred) project.
- **Rate limiting beyond the body-size cap.** CF's free tier enforces
  request counts at the platform level; the Worker itself just lets
  every request through. The Rust self-host has an explicit per-IP
  sliding window which we don't replicate here.
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
