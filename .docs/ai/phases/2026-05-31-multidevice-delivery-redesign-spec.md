# Multi-device DELIVERY-LAYER redesign — spec (2026-05-31, second pass)

> The first convergence fix (`phases/2026-05-31-multidevice-converge-spec.md`)
> fixed the ENGINE (shared-base bootstrap + dedup) but the LIVE DELIVERY layer
> regressed: iOS froze, web edits didn't show on iOS, edits reverted. Engine
> tests proved convergence-given-a-base but never exercised the live path.
> **This pass fixes delivery, and is gated on SIM self-QA before any device test.**

## Evidence (confirmed, not theorized)
- **Partial WS deltas can't bootstrap a base-less device** — `tests/partial_delta_needs_base.rs` (PASSES): `export_doc_update(note, Some(pre_vv))` imported into an empty doc → `rendered=""` (pending, never materializes); only after importing the base does it apply. The first fix put bootstrap on the WRITE path, so a RECEIVE-only device never got the base.
- **iOS display = HTTP refresh, NOT the engine.** `MockMosaicService` shows HTTP-fetched markdown; inbound WS events (`WsEvent::NoteUpdated` text + binary delta) only TRIGGER `applyRemoteChange()` → full-note HTTP `refresh()` + `refreshLoadedPages()`. So every inbound edit = a full-note re-fetch + re-render.
- **The freeze/storm:** per-inbound-delta iOS did import + `tombstone` + full `Snapshot` export-to-disk + `materialize` + full-note HTTP refresh; per-keystroke iOS shipped a full `Snapshot`; and `RUST_LOG=info` made the server's snapshot responses ~40× slower (12ms vs ~500ms — the "506ms" pills). Stacked under a burst of web edits → queue backs up → freeze → refreshes never visibly land → "never updated".
- Server data ended intact (no dup bids) — the revert was live LWW ping-pong + the frozen UI, not disk corruption.

## Design (delivery layer)
1. **Server request logging (visibility FIRST).** Add a `tower_http::trace::TraceLayer` (or equivalent) so every request (bootstrap GET, refresh, WS upgrade) is logged at info with method/path/status/latency. Keep loro at `warn`. This is how we VERIFY behavior instead of guessing. (Keepable infra.)
2. **Catch-up on connect/open, not first-write.** The device must hold the server's note doc as a current base so (a) it produces converging pushes and (b) live deltas apply. Mechanism — pick the simplest that the sim proves works:
   - **Bootstrap-on-open:** when a note becomes visible (daily on launch; any opened note), if the engine doc isn't resident, `GET /loro/notes/{slug}/snapshot` → import. (Reuse the D-a endpoint.)
   - **Gap recovery:** if a device fell behind (live delta is ahead of its VV), re-sync via the snapshot (coarse) — a true VV-handshake (`POST /loro/notes/{id}/catchup` with the device VV → `export_doc_update(note, since=vv)`) is the precise version; implement only if the coarse path proves insufficient in sim QA.
3. **iOS sends DELTAS, not full snapshots.** `produceDeltaFrame` must export `export_doc_update(note, Some(last_pushed_vv))` — track the per-note version the device last pushed/holds, export only newer ops. Full snapshot only when there's no prior VV (cold). (This is the deferred #150, now mandatory.)
4. **Kill the refresh storm.** Inbound deltas must NOT each trigger a full-note HTTP refresh + `refreshLoadedPages`. Options (sim-QA picks): coalesce/debounce `applyRemoteChange` (e.g. one refresh per ~250ms regardless of delta count); refresh ONLY the affected note (drop `refreshLoadedPages` on the delta path); or render the affected note from the engine instead of re-fetching. The bar: a burst of N web edits causes O(1-few) refreshes, not O(N).
5. **Don't do heavy per-delta disk work on the hot path.** Re-evaluate `save_snapshot` + `materialize` on every inbound delta for large notes; debounce or move off the critical path if it dominates.

## SIM self-QA gate (MANDATORY before any device test)
Reproduce + verify on the iOS Simulator (sim shares the Mac network — fine for convergence/perf/display; only raw reachability needs the device, and that already works). Use a temporary iOS sync trace (print/os_log on bootstrap / inbound-apply / refresh / push) + the server request log.
- **Repro FIRST (baseline):** confirm the current build's failure on the sim (web edit storm → frozen/no-update) so we know we're measuring the right thing.
- **Acceptance (post-fix), all observed BY ME on the sim:**
  1. Edit on web `/g` → sim shows it in <1s; server log shows O(1) refresh per edit, not a storm.
  2. Rapid burst of web edits → sim stays responsive (no freeze), converges to final text.
  3. iOS-side edit → appears on web <1s, correct text, no revert on web refresh.
  4. The big "Abide" daily specifically (the note that froze) handles a burst without lockup.
- Only after all four pass on the sim do we build+install on Roshar (clean sandbox) for the device round-trip.

## Tasks (subagent-driven, two-stage review; SIM-verify before device)
1. **Server request logging** — `TraceLayer` in `tesela-server` (`main.rs` router + `tracing` deps). Verify: requests appear in the log with latency. Rust.
2. **iOS bootstrap-on-open** — move/add the snapshot bootstrap to the note-load/display path (and on WS (re)connect for the visible note), not just `recordAndPush`. Keep the resident-check (idempotent). Both shells if shell-level. Swift.
3. **iOS deltas-not-snapshots** — `produceDeltaFrame`/`produce_note_delta` use a real `sinceVv` (track per-note last-held VV); cold = full. FFI may need a small helper; regen if so. Rust+FFI+Swift.
4. **iOS refresh coalescing** — debounce/scope `applyRemoteChange` so a delta burst → O(1-few) refreshes; drop `refreshLoadedPages` on the hot delta path. Swift.
5. **SIM self-QA** — reproduce baseline failure, apply fixes, verify the 4 acceptance points on the sim (web via Playwright + sim screenshots + traces). Iterate until green.
6. **Device round-trip** (user) — clean install on Roshar + Sel, full bidirectional + concurrent test.

## Risks / notes
- `apply_relay_updates` returns `applied>0` even when an import is PENDING (nothing materialized) — so `applyInboundDelta`'s `applied>0` guard fires a refresh even on a no-op apply. The bootstrap-on-open fix makes applies real; also consider gating the refresh on actual content change.
- Keep `hubMode` (relay gate) — verify in sim QA that the relay coordinator is actually off (backend must be `.http`; if a sim runs in mock/default mode, hubMode stays false and the relay coordinator runs).
- Don't delete the engine bootstrap/dedup from pass 1 — this builds on it.
- iOS `endpoint()` slug not percent-encoded (latent, codebase-wide) — fine for date/slug ids.
