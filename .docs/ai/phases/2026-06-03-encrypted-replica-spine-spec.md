# Multi-Transport Sync Spine (Encrypted Cloud + LAN P2P) — milestone spec

**Goal:** one transport-agnostic encrypted Loro-delta sync layer with TWO interchangeable transports the user can toggle between (Anytype-style):
1. **Encrypted cloud replica** — durable, E2E-encrypted, always-on, real-time replica of all notes in the cloud = the cross-network sync spine + off-site backup.
2. **LAN peer-to-peer** — devices on the same network sync DIRECTLY to each other (no cloud), local-first/private/offline-of-internet.

**User toggle (per Taylor, 2026-06-03):** "Cloud sync" (cloud [+ LAN when local]) vs "Local only / device sync" (LAN P2P only — nothing leaves the network). Like Anytype's local-only vs cloud modes.

**Why this composes cleanly:** the sealed TLR2 Loro delta (`AEAD(group_key, TLR2(Vec<LoroDocUpdate>))`) is TRANSPORT-AGNOSTIC. The same encrypted envelope rides the cloud DO or a direct LAN connection; the CRDT converges regardless. So this is ONE delta format + a client-side **transport router** (cloud / LAN-peer / both), not two sync systems.

**Honest constraint:** a web BROWSER cannot do true LAN P2P (no raw sockets / mDNS). So "local-only" is first-class for the native apps (iPhone↔iPad↔Mac direct); in local-only mode the web participates only via the Mac on the LAN. Web's full-power path is the cloud (or the local Mac server).

**Decided (Taylor, 2026-06-03):**
- **Cloud is THE spine.** A Cloudflare Worker + one Durable Object per group is the always-on real-time hub + encrypted store. iPhone, iPad, web, AND the Mac are all equal clients of it. The Mac keeps materializing `.md` files (git/MCP/transcription/backup) but is **no longer the sync hub** → one sync path, no hub↔relay failover logic.
- **Zero-knowledge.** The cloud sees only ciphertext; the group key never leaves devices (existing XChaCha20-Poly1305 AEAD group seal). Holds for the browser too.
- **TypeScript Worker** (not Rust→WASM). Reuse the CF DO/R2/WS primitives; port the small deterministic crypto (HMAC auth-MAC, AEAD open/seal) to TS — the web already has TS TLR2 (`web/src/lib/loro/tlr2.ts`) + `@noble/hashes` (blake3). Rust→WASM = XL for no benefit.
- **Browser key = non-extractable WebCrypto in IndexedDB**, captured via the existing pairing flow (QR/short-code → group_key → `importKey(..., extractable=false)`). Passphrase-derived (Argon2) is a follow-on. Enables "open in any browser, decrypt from the cloud, leave nothing behind."
- **Compaction = device-initiated.** After N deltas a canonical device pushes a fresh sealed snapshot; the relay marks earlier deltas (`applied_vv <= snapshot_vv`) GC-eligible + drops them.

**~80% reuse:** AEAD group seal (`crates/tesela-sync/src/crypto/aead.rs`), HKDF auth-key (`crypto/relay_auth.rs`), TLR2 wire (`crates/tesela-sync/src/wire/mod.rs`), pairing (`crypto/pairing.rs`), per-note VV cursors, the iOS (`RelayTicker.swift`) + desktop (`sync_relay.rs`) produce/apply loops — all UNCHANGED. New = the **storage model** (transient FIFO → durable `{snapshot + delta-log + compaction}`) + the **runtime** (TS Worker/DO) + the **browser-direct E2E** path.

## Architecture (target)
- **Per note, server stores (ciphertext only):** latest encrypted Loro snapshot (R2, content-addressed by blake3) + append-only encrypted delta log (DO-SQLite / D1) + a compaction marker (snapshot_vv).
- **Push:** device exports delta since its broadcast cursor → AEAD-seal(group_key) → `PUT /groups/{g}/ops` → relay APPENDS (no GC-on-ack).
- **Live:** DO fans out the new sealed delta over hibernatable WebSockets (<100ms); clients open + `import_doc_update`.
- **Bootstrap (fresh/recovered device or browser):** `GET /groups/{g}/snapshots` (latest encrypted snapshot per note) → open + import → `GET /ops?since=snapshot_vv` for the tail.
- **Free-tier fit:** single-user ≈500 notes / ~1.5 MB biggest snapshot → ~17k req/day vs 100k cap; R2 pennies.

## Client architecture (Taylor confirmed 2026-06-03: native desktop is the endgame, NOT web)
All PRIMARY surfaces are native — **native desktop wrapper** (its own milestone; aligns with the SwiftUI-macOS GUI vision) + iPhone + iPad. So the **multi-transport sync (cloud client + LAN P2P) lives in the shared Rust `tesela-sync` crate, exposed via FFI** → native desktop + iOS inherit ONE implementation. The TS Cloudflare Worker is only the cloud SERVER endpoint. The **web is a thin secondary** ("open anywhere") surface, not load-bearing → the browser-direct-E2E + web-offline phases drop in priority (they solved for a non-endgame surface; web can lean on the cloud or a local native instance).

## Transport abstraction (the spine)
Client-side `SyncTransport` router carrying the SAME sealed TLR2 deltas over interchangeable backends, driven by the user's **sync-mode toggle** + reachability:
- `CloudReplica` — the CF encrypted durable replica (cloud track below).
- `LanPeer` — direct device-to-device on the LAN (mDNS discover + direct connection; native apps only).
- (transitional) the existing Mac `/ws` hub = a degenerate LAN transport; folds into `LanPeer`/cloud over time.
Modes: **Cloud** = CloudReplica (+ LanPeer when on a shared LAN, for instant local). **Local-only** = LanPeer only; nothing leaves the network (cloud push disabled). Per-mosaic toggle, persisted.

## Phases (each ships standalone value)
**Cloud track:**
1. **Durable encrypted store — self-hosted Rust relay FIRST (decision-independent).** Extend `crates/tesela-relay` storage FIFO → durable `{snapshots table + delta log (stop ack-GC) + compaction}`; add `PUT /groups/{g}/snapshot/{doc_id}` + `GET /groups/{g}/snapshots`; client snapshot-push (threshold) + bootstrap-from-snapshots; device-initiated compaction. **Delivers: encrypted off-site backup + async multi-device, testable self-host, no CF.** Reuses the iOS/desktop loops as-is.
2. **Cloudflare deploy.** Port the durable store to a TS Worker + DO (state/cursors/auth) + R2 (snapshot blobs) + D1/DO-SQLite (delta log). Point Mac + iOS at it. Always-on + free + reachable anywhere. (Poll ok at this phase.)
3. **Real-time DO WebSocket hub.** Hibernatable WebSockets → instant fan-out; iOS `LiveSyncSocket` + web `ws-client` point at the DO. Failover complexity deleted (spine is always-on).
4. **Browser-direct E2E.** Web holds the group key (non-extractable WebCrypto via pairing) + decrypts from the relay directly + IndexedDB offline (snapshot + edit queue). Portable encrypted vault.

**LAN P2P track (parallel; the new/ungrounded piece — investigate before building):**
- L1. **Transport abstraction + sync-mode toggle** (client router + Settings UI; persisted mode).
- L2. **LAN discovery + direct peer transport (native).** mDNS/Bonjour discovery (some infra exists from the retired peer_sync) + a direct device↔device connection (likely each native app runs an `NWListener`/WS endpoint; the Mac tesela-server already can) exchanging the SAME sealed TLR2 deltas + VV catch-up. Mesh-vs-LAN-elected-hub TBD by the investigation. (Old peer_sync data plane was retired as Loro-incompatible — this is a rebuild over Loro deltas.)
- L3. **Local-only mode end-to-end** (cloud push suppressed; web participates via the Mac LAN server only).

**Later:**
5. **Multi-user / Savanne.** Per-user identity (Ed25519) + ACL + presence + attribution, layered on the spine. (Separate spec.)

**Sequencing:** Phase 1 (cloud durable store) is decision-independent + transport-independent → start NOW (delivers the encrypted backup). The LAN P2P track needs its own grounding pass (mesh vs LAN-hub, iOS `Network.framework` feasibility, the web constraint) before L2 — runs in parallel/after.

## Phase 1 task breakdown
- **1a Schema:** `relay_snapshots(group_id, note_id, snapshot_seq, encrypted_bytes, snapshot_vv, created_at)`; keep `relay_ops` as the durable delta log but tag each with `note_id` + `applied_vv` (or derive) so compaction can scope by note + VV. Migration in `crates/tesela-relay/src/store.rs`.
- **1b Endpoints (`handlers.rs`):** `PUT /groups/{g}/snapshot/{note_id}` (store encrypted snapshot, bump snapshot_seq, set compaction marker); `GET /groups/{g}/snapshots` (list latest per note); `GET /ops?since=` unchanged.
- **1c Compaction:** on snapshot deposit, GC delta-log rows for that note with `seq <= snapshot's covered seq`. Stop the all-members-acked GC (`gc_fully_acked_ops`) — durability over eviction; keep ack for cursor bookkeeping only.
- **1d Clients:** desktop `sync_relay.rs` + iOS `RelayTicker.swift` — push a sealed full snapshot per note after N deltas (or on idle); bootstrap a note from `GET /snapshots` then `?since=`. New `RelayClient` methods `put_snapshot` / `fetch_snapshots`.
- **1e Tests (`crates/tesela-relay` + `tesela-sync`):** delta log durable (survives all-acked); fresh-device bootstrap-from-snapshots converges; compaction drops only superseded deltas + bootstrap still converges; two engines converge over the durable relay (extend the existing `relay`/converge tests).

## Verify (Phase 1)
- `cargo test -p tesela-relay -p tesela-sync` green incl. new durability/bootstrap/compaction tests.
- Manual: run the Rust relay locally, point two engines at it, kill+recreate one engine's local store, bootstrap it purely from the relay's encrypted snapshots+deltas → full mosaic restored, converges.

## Open questions / unknowns to resolve in-phase
- Exact `note_id`/VV tagging on delta-log rows for per-note compaction scoping (read current `relay_ops` PK + `produce_relay_updates` batching).
- Snapshot cadence heuristic (N deltas vs size vs idle) — start simple (N=100 or >1 MB log), config later.
- CF specifics (DO SQLite limits, hibernatable WS, R2 multipart for >1.5 MB) — verify against current CF docs at Phase 2.
- Browser AEAD: XChaCha20-Poly1305 isn't native WebCrypto — needs a small WASM/noble lib (Phase 4).
