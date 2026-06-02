# Block-granular writes — spec (2026-06-02)

> Kills the last multi-device data-loss bug: a STALE client's whole-document
> write re-asserts blocks it didn't edit, clobbering concurrent peer edits.
> User chose Approach A, Stages 0–4 (full web+server kill + web re-settle + iOS #150).
> Build subagent-driven, two-stage review per stage. Verify on the 3 real devices.

## Confirmed root cause (deterministic repro)
`crates/tesela-sync/tests/concurrent_whole_body_clobber.rs` (outcome=clobber_reproduced, KEEP as the regression spine): web saves via `PUT /notes/{id}` with the FULL markdown body; server `update_note`→`record_sync_update` runs `diff_note_trees_with_options(old=server-authoritative-file, new=stale-client-body, {emit_deletes:false})`. When a peer concurrently edited block X (server file holds peer's new X) but the saving client is STALE (its body carries old X), the diff emits a `BlockUpsert` re-asserting the stale X text → peer edit lost. The repro shows the diff emitting a `BlockUpsert{text:"beta"}` for a block web never touched. `block_granular_write_preserves_both_edits` (same file, GREEN) proves: applying ONLY the op for the block actually edited preserves both.

**Two INDEPENDENT bugs (both must be fixed):**
1. **The clobber** (server manufactures stale ops from a whole-body diff) → fixed by block-granular writes (Stages 0/1/3).
2. **The stuck split** (web drops its own-echo refetch, never re-GETs the converged file) → fixed by a web re-settle (Stage 2). Block-granular writes ALONE do NOT fix this.

**iOS does NOT have the whole-body clobber** (verified): iOS writes go through the engine (`recordNoteDiff` diffs iOS's OWN materialized file → block ops; `pushPage`/`onLocalWrite` never PUT a full body to the server). iOS's only item is perf (#150: full-snapshot WS frame → delta). One NARROW iOS same-block race is noted in Risks (out of scope, flag a test).

## Key architecture facts (verified — don't re-derive)
- Web's `ParsedBlock.bid` IS the server `block_id` (the `<!-- bid:UUID -->` marker → `FlatBlock.id`, note_tree.rs). A block-granular op = `{block_id=bid, text=raw_text(bid-stripped), parent_bid, indent_level}` → `OpPayload::BlockUpsert`.
- Server has NO block-upsert endpoint (only `DELETE /notes/{id}/blocks/{bid}`, `/blocks/recur-bump`, `/blocks/set-property`). `delete_block` (notes.rs ~386) + `set_block_property` (~1292) are the load-mutate-persist-reindex-WS handler templates to mirror.
- Engine `BlockUpsert` apply (loro_engine.rs ~1856): `find_node_by_block_id` → creates if absent (APPENDS at document end, **ignores order_key**), updates text/indent in place if present. Render = document/creation order + indent. So a mid-note insert via a single BlockUpsert lands at END.
- The PUT post-write fan-out (notes.rs ~256-334): capture `pre_vv = doc_version(id)` BEFORE the write, after the write `export_doc_update(id, Some(pre_vv))` → `ws_delta_tx` (binary) + `WsEvent::NoteUpdated` (text) + reindex + update_links + record_version + ensure_tag_pages. The new endpoint MUST reuse this tail so peers converge identically.
- Web is a pure HTTP-refetch VIEW (drops binary frames at ws-client.svelte.ts:178); convergence depends on a `NoteUpdated` text event → `GET /notes/{id}` reading the always-converged materialized file.

## Stages (each independently shippable + verified; repro test is the spine)

### Stage 0 — server endpoint (no client uses it yet; zero risk)
- Add `POST /notes/{id}/blocks` → `notes::upsert_blocks`. Request: `{ ops: [BlockOp] }` where `BlockOp` is a tagged enum mirroring the engine ops:
  `{ kind:"upsert", bid, text, parent_bid:Option, indent_level }`, `{ kind:"move", bid, parent_bid:Option, indent_level }`, `{ kind:"delete", bid }`.
- Handler: 404 if note absent (mirror update_note:229) UNLESS you choose to seed a `NoteUpsert` when `doc_version`==None (decide; default = require note exists, web guarantees it via the existing create path). Capture `pre_vv`; map each `BlockOp`→`OpPayload`; `s.sync_engine.record_local(op)` per op (mirror record_sync_update's loop); re-read note; reindex + update_links + record_version + **ensure_tag_pages** (parity — so new #tags still spawn tag pages); then the SAME WS fan-out as update_note (`export_doc_update(id, Some(pre_vv))` → `ws_delta_tx{origin:None}` + `WsEvent::NoteUpdated`). Register route in routes/mod.rs near the `/blocks/{bid}` DELETE.
- **Reuse OpPayload + apply_payload + the fan-out verbatim — NO new op kind, wire format, or engine change.**
- Verify: integration test — seed alpha+beta; peer BlockUpsert beta→"beta PEER"; POST upsert_blocks with ONLY alpha→"alpha CHANGED"; assert render = "alpha CHANGED" + "beta PEER" (no clobber) AND a `WsDelta{origin:None}` fired whose `export_doc_update(Some(pre_vv))` carries only the alpha op AND `<slug>.md` materialized. `cargo test -p tesela-server`.

### Stage 1 — web in-place text + indent → block ops (STOPS the dominant loss)
- `web/src/lib/api-client.ts`: add `upsertBlocks(noteId, ops)` POSTing to the new endpoint; call `recordLocalSave(noteId)` (own-echo window) before it, exactly like `updateNote`.
- `web/src/lib/components/BlockOutliner.svelte` + `JournalView.svelte`: route `handleBlockChange(blockId,newText)` → one `{kind:"upsert", bid, text, parent_bid, indent_level}` op; `handleIndent`/`bulkIndent` → `{kind:"move", bid, parent_bid, indent_level}` per affected block. These go through `upsertBlocks`, NOT `buildFullContent`→`updateNote`. Keep the existing DELETE-endpoint path for deletes (already block-granular).
- **Dual-path invariant (CRITICAL, see Risks):** a given save uses EXACTLY ONE path. Text/indent → POST /blocks; frontmatter/title/structural-not-yet-migrated → PUT. The 500ms debounce must not fire a whole-body PUT for the same note+window the user block-edited. Split/guard the debounce per-path; both paths call `recordLocalSave`.
- Verify: the repro logic now holds end-to-end for text edits; tests 1–4 stay green; manual 2-client Chrome (edit block X on client 1, block Y on client 2 concurrently → both survive). svelte-check clean.

### Stage 2 — web re-settle (closes the stuck-split; orthogonal, ship parallel to Stage 1)
- `web/src/lib/ws-refresh-coordinator.ts`: today a `note_updated` echo arriving inside the 1500ms own-echo window has its targeted `["note",id]` refetch DROPPED with no trailing flush → the editing client never re-GETs the converged file. Mirror the iOS deferred-refresh pattern (MockMosaicService `pendingRemoteRefresh`): record the suppressed id in a `deferredNoteIds` set; when `isOwnEcho(id)` becomes false (window expiry), re-enqueue its targeted `["note",id]` invalidation. The hooks exist (`recentSaves`/`isOwnEcho` hold per-id expiry).
- **Must NOT reintroduce the mid-typing reseed clobber the coordinator exists to prevent:** the trailing flush fires ONLY at window expiry and ONLY the targeted `["note",id]` (broad list refresh already fires and is asserted not to feed the editor buffer). Respect the editor's dirty/`isEditingBlock` guard.
- Verify: unit — an own-echo `note_updated` inside the window produces a targeted `["note",id]` invalidation AFTER the window closes (today: none). Manual 2-client — concurrent different-block edits; WITHOUT further edit/reload, the editing client's buffer converges to show the peer's block.

### Stage 3 — web structural edits → block ops (FINISH the root-class kill)
- Migrate `handleEnter`/`handleNewBlockAbove`/`handlePasteBlock` → `{kind:"upsert"}` for the new block (bid already minted client-side via crypto.randomUUID — no server stamp). `handleBackspaceMerge` → `{kind:"upsert"}` on the SURVIVING (previous) block's existing bid with merged text + `{kind:"delete"}` for the absorbed block.
- **New-block-in-middle ordering (the one real gap):** the engine appends new blocks at document END (ignores order_key). For a block appended at the END of the note (the common case — the original repro), this is CORRECT. For a MID-note insert: v1 accepts end-append-then-position is imperfect on peers until re-canonicalized. **DO NOT "fix" position by following the insert with a stale whole-body PUT — that reintroduces the clobber.** Acceptable v1 options (implementer picks, loss-free is the hard requirement, position is the documented caveat): (a) accept end-append for mid-insert in this push + open a follow-up to teach the engine insertion position (order_key/anchor); (b) a position-only re-canonicalize that is itself block-granular (BlockMove with an order anchor) IF the engine can honor it — but it currently can't, so likely (a). State the choice in the report + a follow-up task.
- After Stage 3 the whole-body PUT is used ONLY for frontmatter/title/page-properties.
- Verify: the 3-client concurrent acceptance case (below); tests 1–4 green.

### Stage 4 — iOS #150 (perf, non-blocking; FFI regen + device rebuild)
- `produceDeltaFrame` (RelayTicker.swift ~370, currently `sinceVv:nil`) → pass the last-pushed per-note VV (`note_version` FFI) so steady-state ships a DELTA not a full snapshot. `produce_note_delta` already takes `since_vv`. Track last-pushed VV per note (update after a successful push). Cold/first push = nil (full). Regen Swift bindings if the binding changes; rebuild device/sim `.a`.
- Backstop: a dropped delta leaves the peer's VV behind; `bootstrapNoteIfNeeded` (shipped) catches up on open — confirm it covers a dropped-delta gap.
- Verify: WS frame size drops from snapshot to delta on steady-state; `ws_delta_round_trip*` convergence test green; on-device 2-device edit still converges; NO data-loss regression.

## Acceptance (headline = the 3-client concurrent case)
1. `concurrent_whole_body_clobber.rs::block_granular_write_preserves_both_edits` stays GREEN. The OLD `whole_body_diff_clobbers_concurrent_peer_edit` (expected-fail) → mark `#[ignore]` with a doc comment (it documents the old buggy PUT path) so CI is green.
2. Stage-0 endpoint test (above).
3. **3-client concurrent (the proof):** A/B/C open the same note with blocks X/Y/Z; concurrently A edits X, B edits Y, C edits Z via block-granular POST. After convergence ALL three clients' render AND `<slug>.md` show X'/Y'/Z' — zero loss. Then A inserts W at end while B edits Y → W appears + Y' survives. Then A backspace-merges Y into X while C edits Z → merged survivor correct, Y's node deleted, Z' survives.
4. Web re-settle unit + manual (Stage 2).
5. Peer convergence <1s via the WS delta path (POST /blocks fan-out emits the same `WsDelta` as PUT).
6. iOS: steady-state ships a delta; convergence + on-device test green; no data-loss regression.
7. NO regression: tests 1–4, the WS coalescer tests, T7 engine-render, the own-echo coalescer.

## Risks
- **Dual-write-path coexistence (highest):** the PUT path stays stale-prone; safe ONLY if the client picks exactly one path per save and never double-sends in one debounce window. Server can't enforce it — it's a client contract. Mitigate: per-path debounce + both paths call `recordLocalSave`.
- **Brand-new-note first-write via POST /blocks** won't materialize (slug resolves None → re-read 404). Editor must guarantee the note exists first (existing create path), or `upsert_blocks` seeds a NoteUpsert when doc absent. Decide in Stage 0.
- **New-block-in-middle ordering:** engine appends at end. Loss-free required; position imperfect for mid-insert v1 (Stage 3 caveat). Never re-canonicalize via a stale whole-body PUT.
- **Move/reorder:** `BlockMove` only recomputes indent, never reparents/reorders rows. Indent/outdent safe; true row-reorder needs the deferred fractional-index follow-up — out of scope.
- **PUT-only side effects:** copy `ensure_tag_pages` into `upsert_blocks`; recurrence/dependency bumps (`apply_post_save_bumps`/`apply_dependency_cycles`) run on PUT — status flips already have `set_block_property`; note any gap.
- **Re-settle must not reintroduce mid-typing reseed** (the coordinator's whole reason to exist) — defer to window expiry, targeted id only.
- **iOS narrow same-block race (out of scope, flag a test):** `scheduleWriteback` snapshots `todayBlocks` then async `recordNoteDiff` reads disk; if a peer edit to block X lands on disk between snapshot and diff while iOS edited only Y, iOS can re-assert stale X (LWW within that one block). Far narrower than web's whole-body bug; separate; add a targeted iOS test, don't fix here.
- **Atomicity:** `record_local` is per-op; a mid-batch failure leaves a partial apply already materialized + broadcast. Acceptable v1; revisit batch-txn if artifacts surface.

## Build/run notes
- Live server runs from worktree `.worktrees/sync-live-debug` (log `/tmp/tesela-server-sync-live-debug.log`). When testing the new endpoint live, rebuild+restart that server (I own it). Device builds bake Mac Tailscale IP 100.112.34.59 + http mode (Graphite has no Settings UI, task #156). Date 2026-06-02.
- `xcodebuild` authoritative over SourceKit phantoms. Both iOS shells (GrAppShell + Views/AppShell) stay in sync for any wiring change.
