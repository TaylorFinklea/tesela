# Spec: relay-based presence (Phase 3b) — Taylor's daily-driver path

Status: DESIGN (2026-06-27), `relay-presence-design` workflow (4 agents). Supersedes
hub-mode (HTTP→Mac WS) as the PRIMARY presence transport — Taylor daily-drives the
**relay**, not hub-mode. Hub-mode (Phase 2/3) stays as a same-LAN fallback.

## Why the pivot
On-device test (2026-06-27): hub-mode presence showed nothing. Root cause was NOT
a presence bug — the desktop was the pre-Phase-2 build (12:41), and my relaunch from
`target/` left the loopback server dead (7474 down) → iPhone had no server → sync
degraded + no presence. Taylor's real setup = **relay** (CF). So presence belongs on
the relay. Hub-mode is fragile (needs Mac up, same tailnet, HTTP backend).

## Mechanism — CF Durable-Object WebSocket (Option A, recommended)
`GET /groups/{id}/presence/ws` upgrades to a WebSocket on the GroupDO. Device sends
`{device_id}` first, then `{type:"PRES", ciphertext}` on caret moves. GroupDO holds an
**in-memory** `Map<device_id, WebSocket>` (never persisted), broadcasts each frame to
the OTHER connected sockets. Zero-knowledge preserved — relay forwards opaque AEAD
ciphertext, never decrypts. Real-time (<100ms). Effort ~M (4h) for the Worker.
- Fallback Option B (poll): `POST/GET /groups/{id}/presence` + KV TTL. Simpler (S) but
  laggy (1–2s + KV eventual ~5s) — rejected for "live" cursors.

## Reuse (Phases 1/2/3 are NOT wasted — all reuse cleanly)
- `LoroPresence` codec (PRES frame: `{peer,color,name,slug,bid,offset}`) — web `presence.ts`
  + iOS `LoroPresence.swift`, identical.
- `EphemeralStore` (FFI) + `RemoteCursorStore` (web/iOS) — the store layer.
- AEAD `seal/unseal` (XChaCha20-Poly1305, group key) — wrap the PRES JSON; AAD choice
  baked into the codec (test seal→unseal round-trip before ship; AAD mismatch silently
  corrupts).
- Pathway: `encode (PRES)` → `AEAD seal (group_key)` → relay (opaque) → `unseal` →
  `EphemeralStore.apply` → render.

## Build plan (ordered)
1. **CF Worker** (~4h, AUTONOMOUS — wrangler dev + wscat): `/presence/ws` upgrade,
   MAC-verified, in-memory Map, broadcast, auto-reconnect contract. `cloudflare-relay/
   src/group-do.ts` (wsClients Map ~:59; `GET /presence/ws` case in fetch ~:103;
   handlePresenceWs ~:414). Low-risk (additive route, ops path untouched). DEPLOY to
   Taylor's live relay needs his OK (wrangler deploy).
2. **Desktop bridge** (~M, AUTONOMOUS — two browser tabs): tesela-server bridges local
   `/ws` PRES ↔ CF relay. `routes/ws.rs` route_inbound_binary detects PRES → seal →
   `RelayClient.put_envelope` (outbound); `sync_relay.rs` tick collects inbound presence
   → fan out on `ws_delta_tx` origin=None. ~50 lines. ⚠ ws.rs needs device_id+group_id
   from AppState; verify the origin=None echo path.
3. **iOS relay client** (~M, PARTIAL sim): `RelayClient.put_presence` (mirror
   put_envelope) in `transport/relay.rs:88`; iOS `RelayTicker` presence handle
   (~:95/:379/:1296) — extract relayUrl+groupKey from PairingCode, seal LoroPresence,
   POST/WS. Render in BlockRow overlay (Phase 3 view layer reuses).
4. **Integrate + verify** (~2h): wrangler local + 2 sims, then iPhone+desktop over the
   live relay. Needs Taylor's devices for the final verify.

## Risks
- **Layer-2 convergence** "blocks hard" for op-anchored cursors (twin tombstone kills the
  TreeID anchor). MITIGATED in v1: we use plain `(bid, offset)` + resolve to the live
  survivor (already the chosen anchor). Suppress a cursor whose block has open twins.
- CF DO ~128 WS conns/instance (fine for 1–2 devices; shard later).
- DO idle timeout / eviction → silent disconnect → MUST auto-reconnect (backoff + 30s
  heartbeat).
- `PairingCode.relayUrl` is optional (None for LAN-only) → hub-mode fallback.
- AEAD AAD mismatch silently corrupts → bake AAD into the codec + round-trip test.
- No relay history → late-joiner sees blank until next update (intentional, ephemeral).

## GATE before building
Confirm **relay sync itself is solid** (esp. deletes propagating) on Taylor's restored
setup. The "deletes don't propagate / slow sync" he saw was on the DEAD hub server; if
it persists on RELAY, that's a sync regression and the priority — fix before presence.
GATE CLEARED 2026-06-28: past-day convergence fixed (`cf212bee`) + verified on-device.

---

## VERIFIED CONTRACT (workflow `w4ljve190`, 2026-06-28) — locked for Stages 2/3

Built by an 8-agent workflow (4 boundary maps → synthesized contract → adversarial
design review `contractReady=true` → impl → security review `approve=true`). All claims
verified against source. **Stage 1 (CF Worker) DONE + committed `70338b18`** (NOT
deployed — Taylor-gated `wrangler deploy`).

### WS protocol (native clients only — URLSession/tokio-tungstenite can set headers; browser `new WebSocket` CANNOT → web stays hub-mode)
- URL: `wss|ws://{relay_base}/groups/{group_id_hex}/presence/ws` (scheme-swap the pairing-code relayUrl; nil relayUrl → no relay presence, hub-mode fallback).
- Upgrade GET carries the SAME MAC headers as `GET /ops`, signed over canonical
  `GET\n/groups/{hex}/presence/ws\n\n{nonce_b64}\n{ts_secs}\n` (empty query+body_hash).
  Headers: X-Tesela-Group/Device/Nonce/Ts/Mac. **Signed path MUST be `/presence/ws`** (CF
  rebuilds canonical from x-tesela-original-path). Device id = the MAC-verified
  X-Tesela-Device header (NO plaintext first-frame handshake — that'd be spoofable).
- Frame = `postcard(OuterPayload{nonce:[u8;24], ciphertext})` raw binary over WS (no b64).
  ciphertext = XChaCha20-Poly1305 seal of the EXISTING inner PRES wire `b"PRES" ++ utf8(JSON{peer,color,name?,slug,bid,offset})` — LoroPresence/RemoteCursorStore/presence.ts codec REUSED unchanged. Relay sees only opaque bytes.

### AEAD — the #1 (AAD-parity) risk, RESOLVED by single-source-in-Rust
- **NEW `presence_aad(group_id:&[u8;16]) -> [u8;32] = b"tesela-pres-v1\0\0"(16) || group_id(16)`**, placed in `crates/tesela-sync/src/crypto/aead.rs` next to `envelope_aad`/`snapshot_aad` (modeled byte-for-byte on `snapshot_aad` = `b"tesela-snap-v1\0\0"||group_id`). GROUP-ONLY (no from_device). MUST NOT reuse `envelope_aad` (device||group).
- seal/open = `crypto::aead::seal/open(group_key, …, presence_aad(group_id))` (group_key direct, no HKDF). Defined ONCE in Rust, called by desktop (RelayClient) AND iOS (new FFI) — **never reimplement in Swift/TS**. Add a `seal→open` round-trip test (mirror `aead.rs:99`) before ship.
- MAC/auth key stays `derive_relay_auth_key(group_key, group_id)` (cached `RelayClient.auth_key`), REUSED for the WS-upgrade MAC. Two keys (AEAD=group_key, MAC=auth_key) + two nonces (24-byte AEAD vs 16-byte request) — never crossed.

### Build plan (Stages 2/3) + ADOPTED decisions (contract recommendations)
- **DECISION: shared Rust presence WS client** (tokio-tungstenite) used by BOTH desktop + iOS — defines protocol+MAC+seal+reconnect ONCE (kills drift). Crate: `tesela-sync` (add tokio-tungstenite). iOS reaches it via FFI; Swift owns NO AEAD/socket protocol. (If Taylor prefers a Swift URLSessionWebSocketTask + seal-only FFI, that's the alternative — flagged in the report.)
- **DECISION: CF Worker is the SOLE presence backend** for v1 (Taylor daily-drives CF); the Axum `crates/tesela-relay` stays presence-less.
- **Stage 2 — desktop bridge** (`crates/tesela-server`): bridge local `/ws` PRES ↔ CF presence WS. Outbound: `is_presence_frame(frame) && origin.is_some()` → seal → send to CF (skip CF-injected origin=None frames → no loop). Inbound from CF → fan out on `ws_delta_tx` with origin=None (mirror sync_relay tick fan-out); drop frames whose embedded sender==engine.device(). 3 echo guards.
- **Stage 3 — iOS client**: extract relayUrl+groupKey from PairingCode; reuse the shared Rust presence client via FFI; re-point the existing hub-mode sendPresence/onPresence (SyncState) at the relay; render via the existing RemoteCursorStore (unchanged).
- **Stage 4 — integrate + verify**: needs Taylor's 2 devices over the live relay.

### Accepted Stage-1 residuals (documented, non-blocking)
- X-Tesela-Device NOT in the MAC canonical → a group member can spoof another's echo-exclusion key (availability-only; inner sealed PRES carries the real `peer`). Tighten later if needed.
- No per-frame rate limit post-upgrade (within-trusted-group DoS only).
- Heartbeat (30s) + CF `setWebSocketAutoResponse(ping/pong)` NOT yet added — nail in the shared Rust client (Stage 2) so NAT/edge-dropped idle sockets are detected.
- Layer-2 cursor anchor (suppress/resolve cursors on blocks with open bid-twins) — SEPARATE follow-up vs the layer-2 model (web+iOS); pre-existing, self-healing (cursors re-publish each move + prune 10s). Not a presence gate.
