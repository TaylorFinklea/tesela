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
