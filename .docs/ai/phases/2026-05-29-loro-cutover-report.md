# Loro Cutover — Finish Report (2026-05-29)

Status: **DONE (engineering).** Loro is the sole sync engine; flag-day + dedup + DR drill complete, all green. One operational step remains (live data reset), user-coordinated — see below.

## What shipped

### 1. ai-business snapshot dedup — `8ef366e`
- LoroEngine stored the FULL markdown on each per-note doc's root `content` meta — duplicating the body (already in the `blocks` tree) and doubling every snapshot.
- Now stores only the verbatim frontmatter on root `frontmatter`; full markdown reconstructed on demand via `doc_full_markdown` (frontmatter + rendered body). Helpers: `doc_frontmatter`, `doc_full_markdown` (loro_engine.rs). Readers updated: `render_note_full`, `refresh_note_derived`, `rebuild_index_from_docs`.
- Backward-compatible: pre-dedup docs that still carry `content` fall back to it verbatim. Lean schema lands on **fresh docs** only (Loro snapshots are cumulative — a tombstone doesn't reclaim history), so the size win requires a reseed.
- Regression test: `note_upsert_stores_lean_frontmatter_not_full_content`.

### 2. Flag-day: delete the legacy stack — `471d619` (breaking)
Net ~3,600 lines deleted. Loro is the only engine; no fallback.
- DELETED: `engine/sqlite_engine.rs`, `engine/dual_engine.rs`, `tests/convergence.rs`, `examples/two_node.rs`.
- `SyncEngine` trait slimmed: removed `apply_changes` / `produce_changes_since` / `produce_local_authored_since` / `uses_loro_relay_payload` / `ProducedBatch`. LoroEngine is the sole impl.
- Wire: deleted `encode_op_batch` / `decode_op_batch` (the v1 op-wire); kept the Loro v2 (`TLR2`) payload + the discrimination test.
- Server: `main.rs` engine construction flattened to unconditional `LoroEngine::with_dirs` (dropped `TESELA_LORO_DUAL_WRITE` / `TESELA_LORO_AUTHORITATIVE`; kept `TESELA_LORO_RESEED`). `sync_relay.rs` tick = Loro v2 only (dropped legacy branches + dead `outbound_cursor_ntp`). Deleted the dual-write divergence endpoints (`/loro/notes/:slug`, `/loro/divergence`, `/loro/reconcile-stale-blocks`); kept `/loro/index`.
- FFI: removed legacy `open` / `open_with_mosaic` (open_loro is the sole constructor); SyncCoordinator ticks = Loro v2 only.

### 3. peer_sync (LAN P2P) — data-plane retired
The op-replay pull model is fundamentally incompatible with Loro (no op log to replay from a cursor) and **fully redundant with the relay spine** (which broadcasts every update to every peer → no convergence loss). `produce` / `receive_envelope` return **501**; the daemon path is a no-op. Pairing + discovery + status stay live. **Follow-up:** reimplement LAN P2P over the Loro relay-update protocol as a latency optimization (the pairing/transport scaffolding is kept for it).

### 4. iOS FFI — `c626d25`
Rebuilt `libtesela_sync_ffi.a` for `aarch64-apple-ios{,-sim}` against the flag-day FFI; regenerated UniFFI bindings (0.31 library mode) so `Generated/` + `CFFI/` match (the old bindings referenced the removed `open_with_mosaic` symbol/checksum). `SyncSettingsView.swift` smoke helper → `openLoro`. **`xcodebuild -scheme Tesela -sdk iphonesimulator` → BUILD SUCCEEDED.**

## DR drill (2026-05-29) — recovery + dedup validated on an isolated copy

Procedure (non-destructive; no relay config → no live-relay/iPhone contact):
1. Copy live `<mosaic>/notes/` (514 `.md`, the source of truth) → fresh temp mosaic.
2. Boot `tesela-server --mosaic <temp>` with `TESELA_LORO_RESEED=1 TESELA_DISABLE_MDNS=1` on port 7475.
3. Verify, then tear down + remove temp dir.

Results:
- **Recovery:** reseeded 514 notes into lean Loro docs; `/health` 200, `/notes` serves, `/loro/index` = 514 entries.
- **Dedup payoff:** ai-business snapshot **5,131,733 → 2,580,415 bytes** (50% ↓) — now under the relay's 5 MB body limit, so it will sync. Total loro dir 11 M → 7.1 M.
- **Index correctness post-dedup:** ai-business index entry has title + 2 tags, derived from the reconstructed (frontmatter+body) content — confirms `doc_full_markdown` on real data.

**DR procedure (canonical):** the mosaic's `notes/*.md` ARE the source of truth. Recovery = restore `notes/` → boot with `TESELA_LORO_RESEED=1` (one device only) → Loro rebuilds. Snapshots under `.tesela/loro/` are a derived cache.

## Remaining: live data reset (USER-COORDINATED — needs the iPhone)

The dedup's size win only lands on **fresh** docs. The live mosaic still holds the pre-dedup (bloated) snapshots; ai-business won't fit the relay until rebuilt. The reset is destructive + changes ALL doc identities, so it must be coordinated with every peer:

1. Stop the Mac server. Back up `<mosaic>` (DR backup).
2. `rm -rf "<mosaic>/.tesela/loro/"` then boot with `TESELA_LORO_RESEED=1` (rebuilds fresh lean docs from disk).
3. **iPhone re-bootstrap:** wipe the iPhone app's local Loro docs (fresh-identity docs would otherwise merge with its old docs → duplication) and let it re-pull from the relay. (Clearing the relay's stored ops first is cleanest.)

Until then the flag-day server runs fine on existing docs via the backward-compat fallback (ai-business simply stays unsynced, as before). Do this with the user + device present.
