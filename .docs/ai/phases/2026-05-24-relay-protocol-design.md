# Sync Relay — Protocol Design

**Date:** 2026-05-24
**Status:** Draft for review
**Replaces:** the 6-line stub at `crates/tesela-sync/src/transport/relay.rs` (Phase 3 placeholder)
**Related decisions:** [sync-architecture-p2p](../../memory/MEMORY.md), [sync-work-order](../../memory/MEMORY.md)

## Why this exists

The previous P2P-only sync decision (2026-05-12) ruled out a content-storing
relay for trust + cost reasons. Two weeks of dogfooding surfaced that
P2P-only doesn't cover the actual usage shape: the Mac server isn't always
on, devices roam off LAN, and iOS especially needs to operate independently
of the Mac being awake. The user reformulated the ask: a relay that's
either **transient** (no persistence) or **zero-knowledge** (server can't
read content). That keeps the spirit of the original decision (no
trust delegated to the relay) while solving asynchronous, cross-network
delivery.

This document specifies the relay protocol. Two reference implementations
will ship against it: a Rust/Axum self-host (Docker) and a Cloudflare
Worker (TypeScript + Durable Objects). Both speak the same wire format.

## Goals

- **Zero-knowledge.** The relay sees only routing metadata + opaque
  payload bytes. It cannot decrypt note content, derive block content
  from sizes, or correlate user identity beyond the
  per-group identifier.
- **Asynchronous.** A device can deposit ops while the recipient is
  offline; the recipient fetches when it next comes online. This is
  what makes "compose on iPhone at 3am, Mac picks up at 9am" work.
- **Portable across hosts.** Same protocol on Docker (Rust/Axum/SQLite)
  and Cloudflare Worker (TS/Durable Objects). Self-host = full control;
  Worker = zero infra.
- **Backward-compatible with existing sync.** Reuses `SyncEnvelope`,
  AEAD primitives, and group identity from `tesela-sync`. Relay
  is just another `Transport` impl.
- **Authenticated.** Only members of a group can deposit / fetch for
  that group. Uses the existing `GroupKey` for MAC-based auth — no
  account system, no separate credential store.
- **Operator-trivial.** Self-host = one binary + SQLite file. No
  database setup, no migrations during bring-up.

## Non-goals

- Real-time delivery latency targets. The relay is a deposit box, not a
  message bus. Push delivery is a separate (deferred) APNs-style concern.
- Multi-tenant SaaS. Each relay deployment is single-user (or
  small-trust-group). Multi-tenancy emerges naturally from group-ID
  namespacing if someone wants it, but isn't a designed feature.
- Replacing LAN transport. LAN-mDNS stays the preferred path when peers
  see each other; the relay is fallback / cross-network.
- Content versioning, history, conflict resolution. All of that happens
  at the `SyncEngine` layer, not the relay. The relay forwards ops
  blindly.

## Architecture at a glance

```
┌──────────┐                                    ┌──────────┐
│ Device A │ ─── PUT /groups/{g}/ops ──────────▶│  Relay   │
│  (Mac)   │                                    │ (Worker  │
└──────────┘                                    │  or self-│
                                                │  host)   │
┌──────────┐                                    └──────────┘
│ Device B │ ◀── GET /groups/{g}/ops?since=N ───       │
│ (iPhone) │ ──── ACK applied seqs ────────────────────┘
└──────────┘
```

The relay is a per-group FIFO of opaque envelopes. Devices append
(`PUT`); other devices in the group fetch new envelopes since their
last-applied seq (`GET`); the relay garbage-collects envelopes once
every known group member has acked them.

## The wire format

### Outer envelope (relay-visible)

```jsonc
// PUT /groups/{group_id}/ops
{
  "from_device": "<hex device id, 32 bytes>",
  "seq": 0,                  // monotonic per group; relay assigns
  "ts": 1748182600.123,      // unix epoch seconds, relay-assigned
  "payload_b64": "<base64 standard, no padding>"
}
```

`from_device` and the request body are signed with the group's MAC key
(see "Auth" below). `seq` and `ts` are server-assigned in the response.

```jsonc
// GET /groups/{group_id}/ops?since={seq}
[
  { "from_device": "...", "seq": 17, "ts": ..., "payload_b64": "..." },
  { "from_device": "...", "seq": 18, "ts": ..., "payload_b64": "..." }
]
```

### Inner payload (encrypted, relay-opaque)

`payload_b64` decodes to the existing `SyncEnvelope.ciphertext`:
AEAD-sealed postcard-encoded `Vec<EncodedOp>`. The AEAD nonce + auth
tag are inside the sealed bytes (see `crypto::aead::seal` for the
canonical layout). The relay never opens this.

## Auth

Each group has a 32-byte symmetric `GroupKey` (already issued by
`tesela-sync::crypto::keys::load_or_create`). The `GroupKey` itself
encrypts/decrypts content and never leaves devices. To let the relay
verify request authenticity without giving it the content key, every
device deterministically derives a per-group **auth key** via HKDF:

```
auth_key = HKDF-SHA256(
    ikm  = group_key,
    salt = group_id,
    info = b"tesela-relay-auth-v1",
    len  = 32,
)
```

Properties that fall out of this:

- One-way: holding `auth_key` doesn't let you recover `group_key`. The
  relay can authenticate requests but can't decrypt payloads.
- Deterministic: every device in the group computes the same
  `auth_key` from the same `group_key` independently. No out-of-band
  key exchange to the relay beyond first-write registration.
- Per-group: an auth key compromised for one group doesn't touch any
  other group. Domain-separated by the `info` string + `group_id` salt
  so future protocol versions can rotate (`tesela-relay-auth-v2`)
  without rebuilding existing groups.

### Registration with cryptographic intent proof

Before the relay can verify MACs for a group it needs the
`auth_key`. The first device that PUTs to a never-seen `group_id`
follows a one-step registration that includes a **signed intent** —
a value that ONLY a holder of `group_key` can produce, even though the
relay can't verify it on its own:

```
POST /groups/{group_id}/register
{
  "auth_key_b64": "<32 bytes base64>",
  "registered_at": 1748182600,           // unix seconds
  "intent_b64": "<HMAC-SHA256(group_key, intent_msg)>"
}

intent_msg = "tesela-relay-register-v1"
           + "|" + group_id_hex
           + "|" + auth_key_b64
           + "|" + registered_at
```

The relay stores `(group_id, auth_key, registered_at, intent_b64)`
verbatim. It does NOT (and cannot) verify the intent — the verification
key is `group_key`, which is content-bearing. From that point the
relay verifies every request's MAC against the stored `auth_key`.

Subsequent calls to `/register` from any device return:

- `200 OK` if the supplied tuple **exactly** matches the stored one
  (byte-for-byte). Idempotent — late joiners + post-data-loss
  self-heal both work without re-registration since `auth_key` is
  deterministic from `group_key` and `registered_at` doesn't change
  retroactively.
- `409 Conflict` with the stored registration payload if a different
  tuple was registered first. Clients **must** treat 409 as a hijack
  signal and surface it to the user — see "Joiner verification" below.

### Joiner verification (the load-bearing check)

Every device joining a group with a relay configured must, **on first
connection to that relay**, fetch the registration record and verify
the signed intent against its local `group_key`:

```
GET /groups/{group_id}/registration
→ { "auth_key_b64": "...", "registered_at": ..., "intent_b64": "..." }

// Joiner recomputes locally:
expected_intent = HMAC(local_group_key, intent_msg)

// If expected_intent != intent_b64 from relay:
//   abort, surface "relay has been hijacked" error to user
// Else:
//   pin (relay_url, group_id, auth_key, registered_at) locally,
//   trust subsequent traffic
```

This pushes the security guarantee out of the relay (which has no
keys) into the clients (which all hold `group_key`):

- A hijacker who doesn't have `group_key` can register a bogus
  `auth_key` + bogus `intent_b64`. They can deposit + fetch under the
  bogus auth_key fine — but **no legitimate group member will ever
  trust that registration**, because the intent check fails on every
  joiner.
- The hijacker also can't deposit messages legitimate members will
  *apply*: the inner AEAD seal uses the real group_key, which the
  hijacker doesn't have. Their deposits are unreadable garbage.
- Outcome: hijacker DoSes the group by squatting the relay-side
  registration; they cannot impersonate members or read content.
  The user sees a clear error + invokes admin recovery (below).

The first-device that registered the group never sees the joiner
verification failure path because it provided the registration. But
it **does** pin its own `(auth_key, registered_at)` locally after
`POST /register` returns 200, and every subsequent request from that
device verifies the relay still serves the same registration on
`GET /registration` (cheap, can be amortized to once per hour) —
so a relay that's compromised AFTER initial registration is also
detectable.

### Per-request MAC

Every relay request other than `/register` and `/registration` carries:

- `X-Tesela-Group: <group_id_hex>`
- `X-Tesela-Device: <device_id_hex>`
- `X-Tesela-Nonce: <16 random bytes, base64>`
- `X-Tesela-Ts: <unix seconds>` (clock skew tolerance: ±300s)
- `X-Tesela-Mac: <HMAC-SHA256(auth_key, canonical_request)>`

`canonical_request = method + "\n" + path + "\n" + query + "\n" + nonce
+ "\n" + ts + "\n" + body_hash` where `body_hash = SHA256(body)` or
empty for GETs.

The relay verifies:

1. The group is registered.
2. The timestamp is within ±300s (replay protection).
3. The nonce hasn't been seen in the last 5 minutes (per-group
   in-memory LRU; dedupes burst replays).
4. The MAC matches `HMAC-SHA256(stored_auth_key, canonical_request)`.

Failures return `401` (group not registered or MAC mismatch) or `400`
(timestamp out of window, replayed nonce).

### Admin recovery (for the hijack-DoS case)

When a hijacker squats a group_id, legitimate devices can't register
under the same id. Recovery requires the relay operator to nuke the
bogus registration:

```
DELETE /admin/groups/{group_id}/register
Authorization: Bearer <admin_token>
```

The `admin_token` is set at relay deployment (env var on self-host;
Wrangler secret on Worker). Once the bogus registration is gone, the
legitimate device's `/register` POST succeeds and group sync resumes.
This is the operator-shaped escape hatch — same pattern as Caddy/Nginx
removing a bad ACME registration.

In the Worker case, `admin_token` is per-deployment; in the
self-host case, it's per-installation. There's no per-group admin
credential — once the operator can delete one group's registration,
they can delete any group on that relay. That's fine: the operator
already has full operator-level access to the relay (they run it).

### Per-request MAC

Every relay request other than `/register` carries:

- `X-Tesela-Group: <group_id_hex>`
- `X-Tesela-Device: <device_id_hex>`
- `X-Tesela-Nonce: <16 random bytes, base64>`
- `X-Tesela-Ts: <unix seconds>` (clock skew tolerance: ±300s)
- `X-Tesela-Mac: <HMAC-SHA256(auth_key, canonical_request)>`

`canonical_request = method + "\n" + path + "\n" + query + "\n" + nonce
+ "\n" + ts + "\n" + body_hash` where `body_hash = SHA256(body)` or
empty for GETs.

The relay verifies:

1. The group is registered.
2. The timestamp is within ±300s (replay protection).
3. The nonce hasn't been seen in the last 5 minutes (per-group
   in-memory LRU; dedupes burst replays).
4. The MAC matches `HMAC-SHA256(stored_auth_key, canonical_request)`.

Failures return `401` (group not registered or MAC mismatch) or `400`
(timestamp out of window, replayed nonce).

### Trust model recap

What the relay sees: `group_id`, `device_id`s of registered members,
`auth_key` + `intent_b64` per group, envelope sizes, timestamps, IP
addresses.

What the relay does NOT see: note content, block content, anything
about what the user is doing inside their notes. Payloads are AEAD-
sealed with the `group_key`, which never reaches the relay. The
relay cannot even *verify* the registration intent — only group
members holding `group_key` can.

A relay operator (you or your hosting provider) can confirm "this
group exists, here are its device fingerprints, here's the traffic
volume." They cannot read any note. They CAN, with the admin token,
delete a group registration to recover from hijack squatting.

### Adversary capabilities (formal)

- **Network eavesdropper on TLS-stripped link:** can see request
  metadata + opaque envelopes. Can't decrypt, can't forge MACs
  (would need `auth_key`).
- **Holder of `group_id` only** (e.g. via pairing-code screenshot
  with the key portion redacted): can hit `/register` with a bogus
  `auth_key`. Legitimate group members reject the bogus
  registration on `GET /registration` (intent verification fails),
  surface the hijack to the user, and admin-recover. Hijacker cannot
  read content, cannot impersonate members.
- **Holder of `group_key`** (full pairing code leak, or device
  compromise): wins. They are the group as far as crypto goes —
  same as Signal "if your phone is unlocked, the attacker IS you".
  No protocol can fix this; out of scope.
- **Compromised relay operator:** can read metadata, can drop /
  reorder envelopes (denial of service), can NOT read content. Real
  group members holding `group_key` can detect tampering with the
  registration via re-fetching `/registration` and verifying the
  pinned values.

The TOFU window for new groups is bounded by "between
`POST /register` and the first joiner's `GET /registration`". The
group's *creator* (the first device) is by definition the registrant,
so the race is "between group creation on device A and pairing-code
delivery to device B" — typically seconds, on the user's own
devices.

## Storage shape (relay-side)

Per group, the relay keeps:

| Field | Type | Notes |
|---|---|---|
| `seq` | u64 PRIMARY KEY | Monotonic, gapless, per-group. |
| `from_device` | bytes(32) | For peer-cursor display only. |
| `ts` | f64 | Relay-assigned epoch seconds. |
| `payload` | blob | Opaque ciphertext. |
| `acks` | bytes (bitset / json set) | Set of device ids that have acked this seq. |

```sql
-- SQLite (self-host)
CREATE TABLE relay_ops (
  group_id BLOB NOT NULL,
  seq INTEGER NOT NULL,
  from_device BLOB NOT NULL,
  ts REAL NOT NULL,
  payload BLOB NOT NULL,
  acks TEXT NOT NULL DEFAULT '[]',  -- JSON array of hex device ids
  PRIMARY KEY (group_id, seq)
);
CREATE INDEX idx_relay_ops_group_seq ON relay_ops(group_id, seq);
```

Cloudflare Worker: one **Durable Object per group**. Storage API is
key-value; key = `op:{seq_padded}`, value = the row. Devices ack via
a small JSON file per group: `acks:{seq_padded}` → list of device ids.
Garbage-collect once `len(acks) == known_member_count`. (Known members
list maintained via deposit traffic — every depositing device is
implicitly a member.)

## Endpoints

```
POST   /groups/{group_id}/register         # first-write registration with intent
GET    /groups/{group_id}/registration     # fetch registration record for joiner verification
PUT    /groups/{group_id}/ops              # deposit one envelope
GET    /groups/{group_id}/ops?since=N      # fetch envelopes seq > N
POST   /groups/{group_id}/ack              # body: {device, applied_seq}
GET    /groups/{group_id}/cursors          # debug: known members + min ack
DELETE /admin/groups/{group_id}/register   # admin-token gated; recover from hijack
```

That's it. No login, no `/me`, no profile, no project hierarchy.

## Retention

The relay drops envelopes from its store once every known group member
has acked them. **Known member** = "device that has either deposited
or acked in the past 30 days." Devices that drop off the network for
longer get pruned from the known-member set; the relay then frees their
backlog (the offline device can re-sync via LAN when it returns).

This means a phone that's offline >30 days loses pending deposits.
That's fine for the deposit-box model — sync is best-effort. The
authoritative source is the desktop's on-disk state, not the relay.

## Client wiring (`tesela-sync::transport::relay`)

```rust
pub struct RelayTransport {
    base_url: Url,
    group_id: GroupId,
    device_id: DeviceId,
    group_key: GroupKey,  // for inner AEAD seal/open
    http: reqwest::Client,
}

impl Transport for RelayTransport {
    async fn send(&self, env: SyncEnvelope) -> SyncResult<()> {
        // 1. AEAD-seal env.ciphertext using group_key (Phase 2 wrap).
        // 2. Wrap as outer relay envelope.
        // 3. PUT.
    }
    async fn poll(&self) -> SyncResult<Vec<SyncEnvelope>> {
        // 1. GET ?since=<our_last_seq>.
        // 2. AEAD-open each payload.
        // 3. Return SyncEnvelope list.
    }
    async fn ack(&self, max_applied_seq: u64) -> SyncResult<()> {
        // POST /ack.
    }
}
```

The desktop `tesela-server` adds a `[sync.relay]` config block:

```toml
[sync.relay]
url = "https://relay.example.com"
# OR self-host:
# url = "http://my-docker-host.tailnet:8484"
poll_interval_ms = 5000
```

When configured, the server brings up `RelayTransport` alongside the
existing `LanDiscovery` + `LoopbackTransport`. `SyncEngine` is transport-
agnostic; both run in parallel; envelopes reaching the SyncEngine
through either path apply identically.

## Pairing UX (delta from current)

The existing pair-device flow generates a `PairingCode` containing the
group id + group key (`crypto::pairing::encode`). The flow gains an
**optional relay URL** field: if the user has configured a relay on the
desktop, the pairing code embeds the URL so the joining device can
auto-configure. No change to the pairing UI's "scan QR / type 6-char
code" affordance — just one more field carried in the encoded payload.

## Conformance tests

Both implementations pass a shared test suite (`crates/tesela-relay-
conformance`). The suite is HTTP-level (no Rust deps in the assertions),
runnable against any deployed relay. Cases:

1. **Register + round-trip:** POST /register with `(auth_key,
   timestamp, intent)`; PUT one envelope with a valid MAC; GET fetches
   it back. Seq is 1.
2. **Re-register idempotent:** POST /register with the *same* tuple
   returns 200. POST with a different `auth_key` returns 409 +
   the stored registration in the body.
3. **GET /registration returns stored record verbatim** — bytes
   identical to what was POSTed (so joiner-side intent verification
   has fixed inputs).
4. **MAC required for non-registration endpoints:** PUT without
   `X-Tesela-Mac` returns 401. PUT with a MAC computed under a
   different auth_key returns 401.
5. **Monotonic seq:** Three PUTs from two devices interleaved; seqs
   are 1, 2, 3 in arrival order.
6. **Since filter:** PUT three envelopes; GET ?since=1 returns only
   seqs 2 + 3.
7. **Ack + GC:** Two devices ack seq 3; envelope is gone on subsequent
   GET (no peers need it).
8. **Body size cap:** PUT > 1 MiB returns 413.
9. **Per-IP rate limit:** 1000 PUTs in 10 seconds from one IP returns
   429 on the last few.
10. **Cross-group isolation:** PUTs against group A don't leak into
    GETs against group B even with valid headers.
11. **Replay window:** A request with `X-Tesela-Ts` more than 300s
    old returns 400.
12. **Nonce dedupe:** Same nonce within 5 minutes returns 400.
13. **Admin recovery:** `DELETE /admin/groups/{id}/register` with the
    admin token wipes the registration; subsequent /register with a
    new tuple succeeds. Without the token, returns 401.

The Rust impl runs these against itself in CI. The Worker impl gets a
separate workflow that deploys to a preview environment and runs the
same suite.

## Operator surface

### Self-host (Docker)

```yaml
# docker-compose.yml fragment
services:
  tesela-relay:
    image: ghcr.io/tfinklea/tesela-relay:latest
    ports: ["8484:8484"]
    volumes: ["./relay-data:/data"]
    environment:
      TESELA_RELAY_DB: /data/relay.sqlite
      TESELA_RELAY_BIND: 0.0.0.0:8484
      TESELA_RELAY_MAX_BODY: 1048576
```

No setup beyond pulling the image. SQLite file lives in the mounted volume.
TLS termination is the operator's problem (Caddy / Cloudflare Tunnel / etc).

### Cloudflare Worker

```bash
wrangler init tesela-relay --template tesela-relay-cf
# edit wrangler.jsonc — set CUSTOM_DOMAIN, leave the rest
wrangler deploy
```

Single Worker; per-group state in a Durable Object class. Pricing scales
with envelopes-stored × hours, not user count. The reference deployment
under one user's typical load (5 devices, ~50 ops/day) costs well under
the free tier.

## Iteration plan

| Stage | What | Effort |
|---|---|---|
| **1. This doc** | Design review + approval | — |
| **2. Conformance test suite** | HTTP-level tests; no impls yet | 1–2 days |
| **3. Rust/Axum relay** | `crates/tesela-relay` binary + SQLite store | ~3–5 days |
| **4. `RelayTransport` client** | In `tesela-sync::transport::relay`; AEAD-wrap path | ~2–3 days |
| **5. Desktop wiring** | `tesela-server` config + pairing-code URL field | ~2 days |
| **6. End-to-end smoke** | Two Macs on different networks sync via Docker relay | — |
| **7. Cloudflare Worker port** | TS impl passing conformance suite | ~3–5 days |
| **8. iOS UniFFI track** | (Separate spec) iPhone becomes a real sync peer | ~3–6 weeks |

Stages 2–6 are the "relay track" the user picked. Stages 7 ships the
portability promise. Stage 8 is the parallel iOS track that closes the
whole sync story.

## Open follow-ups (intentionally not blocking)

- **APNs push proxy** so iOS gets notified to wake up + fetch when a
  new envelope lands. The relay can call APNs as a side-effect of PUT;
  the push payload is content-free ("new mail in group X").
- **Worker-side rate-limit / abuse handling** beyond per-IP. Probably
  needs `cf_zone_id` integration.
- **Multi-relay redundancy.** Devices configured with N relays
  deposit to all, fetch from all. Cheap reliability boost; needs no
  protocol change.
- **Per-relay metrics + observability.** Cloudflare's analytics covers
  the Worker side; the Rust impl needs `tracing` + a `/metrics`
  Prometheus endpoint.
