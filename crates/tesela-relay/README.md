# tesela-relay — Frozen Surface

This is the **Rust reference relay implementation** for Tesela's sync protocol. As of 2026-07-01, the relay surface is **frozen**: new features are not accepted; only conformance-parity updates are permitted.

## Frozen Surface (ADR-2)

The relay's public API is defined by the **conformance test suite** (`cargo test -p tesela-relay --test conformance`). Any change must pass this suite. The stable endpoints are:

- **`POST /register`** — Device registration (group + auth_key + intent signature)
- **`GET /registration`** — Verify stored intent against the local group_key (hijack detection)
- **`POST /deposit`** — Relay PUT: client uploads sealed envelopes for the group
- **`GET /sync`** — Relay GET: client polls for sealed envelopes since a cursor
- **`POST /ack`** — Acknowledgment: client confirms received ops (garbage collection trigger)
- **`GET /snapshot`** — Snapshot delivery: full state dumps for new devices
- **`GET /discover`** — Discovery: list groups a peer ID has ever joined (read-only; used for peer-finding)
- **`POST /admin/groups/:id/register`** — Admin registration reset (hijack recovery)
- **`DELETE /admin/groups/:id/register`** — Admin deregistration (hijack recovery; MAC-gated rotation DELETE pending, see below)

## Permanent Exclusions

The relay will **never** implement:

- **Presence** — Ephemeral presence (who's online now) is handled exclusively by the **Cloudflare Durable Object WebSocket** (`cf-worker-relay`), not the Rust relay. The Rust relay carries no WebSocket feature; presence is out of scope here.
- **APNs delivery** — Apple Push Notifications are best-effort hints sent by the CF Worker on deposit. The Rust relay does not know about APNs; see the CF Worker for delivery logic.

## Change Policy

After the freeze:

1. **Conformance parity is the ONLY gate.** Any change must pass `cargo test -p tesela-relay --test conformance` (this is the CI gate in `.github/workflows/ci.yml`).
2. **Never add features.** New endpoints, new fields, new behaviors — all blocked. The relay's job is synchronous store-and-forward; auth/presence/delivery are upstream concerns (CF Worker, client libraries, APNs infrastructure).
3. **Bug fixes and security updates** are allowed if they preserve conformance.
4. **Self-hosting safety** is non-negotiable. Self-hosters' expectations are set once; breaking changes are unacceptable after this freeze.

## One Sanctioned Pending Addition

**MAC-gated self-teardown DELETE** (from `phases/2026-07-01-groupkey-rotation-spec.md` §6):

When a group rotates its key (removing a compromised device), the removed device may trigger its own cleanup via:

```
DELETE /admin/groups/:id/devices/:peer_id
  Authorization: Bearer <MAC(device_id, new_group_key)>
```

This is a **conformance case** already designed in the rotation spec; it must land as a BOTH-relay change (Rust + CF Worker) once the rotation feature ships. It is NOT a feature-creep exemption — it is part of the conformance contract, deferred only until rotation itself is ready.

## CI Gate

The **`worker-conformance`** job in `.github/workflows/ci.yml` (line 159) runs the conformance test suite against the **Cloudflare Worker relay** — the production relay. The Rust relay's `cargo test -p tesela-relay --test conformance` is its local mirror. Both must pass before a release ships. If either fails, the freeze is violated and the change is rejected.

## Self-Hosting

For self-hosters, see [`DEPLOY.md`](DEPLOY.md) for full walk-throughs. Key constraints:

- **Max request body: 16 MiB** (set in `Cargo.toml` via `axum`'s default limits). This must be documented wherever the relay is deployed (Docker, Home Assistant add-on, etc.). A request larger than 16 MiB is rejected with HTTP 413 Payload Too Large.

Home Assistant add-on hosts also note this in their config — see [`home-assistant/tesela-relay/DOCS.md`](../../home-assistant/tesela-relay/DOCS.md).

## Development

- **Run the relay locally:** See `DEPLOY.md` for Docker quick-start.
- **Run tests:** `cargo test -p tesela-relay`
- **Conformance suite:** `cargo test -p tesela-relay --test conformance` — defines the frozen surface; this is the contract.
- **Smoke tests:** `cargo test -p tesela-relay --test smoke` — operational health checks.

See [`SMOKE.md`](SMOKE.md) for integration-testing patterns.

## Further Reading

- **Threat model & protocol:** `.docs/ai/phases/2026-05-24-relay-protocol-design.md`
- **Relay architecture decision:** `.docs/ai/decisions.md#adr-2 — Relay end-state`
- **Rotation spec (pending DELETE addition):** `.docs/ai/phases/2026-07-01-groupkey-rotation-spec.md`
