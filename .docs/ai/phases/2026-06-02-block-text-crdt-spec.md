# Block text as a real CRDT (LoroText) — spec (2026-06-02)

> The DEEPEST fix: concurrent edits to the SAME block currently do whole-text
> last-writer-wins (one side's typing is silently lost — wire-proven on device).
> Make each block's text a Loro `LoroText` sequence CRDT so same-block edits
> INTERLEAVE/merge. User chose the full CRDT fix. Approach (b): engine-only,
> zero client/wire changes. Build subagent-driven, two-stage review, verify the
> merge ON THE WIRE on the sim before any device test.

## Confirmed root cause (wire-proven device trace)
A block's text is a Loro LWW MAP REGISTER: `apply_payload_inner` BlockUpsert arm does
`meta.insert("text", text)` on the tree node's meta `LoroMap` (loro_engine.rs ~2362; also `seed_tree_from_flatblocks` ~1540). A map value is last-writer-wins. So when web + iOS write the SAME block's whole text concurrently, the higher (lamport,peer) wins and the loser's text vanishes — NOT character merge. Device wire trace: web POST `fb728a67 "splendid work http://localhost"`; iOS WS frame set the SAME block to `"splendid work by a great man..."`; iOS won; web's text lost. Every prior fix protected WHICH blocks apply; none addressed how a single block's concurrent text edits combine. The module docstring (loro_engine.rs:15) already PLANS `text: LoroText` ("character-level concurrent edits") — never implemented.

## The fix — Approach (b): LoroText engine + server-side whole-text→splice (ENGINE-ONLY)
**Central verified insight:** the WS/relay multi-device path already merges Loro update deltas automatically once block text is a `LoroText` container — the relay stays a dumb byte pipe, nothing on the wire changes. And `LoroText::update(new_text, opts)` (loro 1.12, lib.rs:2442) runs Myers' diff INTERNALLY, converting a whole-string replacement into the minimal splice ops against the container's CURRENT value. So:
- Clients keep sending WHOLE block text, UNCHANGED (web `upsertOpForBlock`, iOS `record_note_diff` re-authoring from markdown). The hardest constraint (iOS has no per-keystroke delta at the FFI) is sidestepped entirely.
- `OpPayload::BlockUpsert.text: String` stays a whole String. `op.rs`, `diff.rs`, `block-ops.ts`, the FFI, `note_tree.rs` are UNCHANGED.
- The ONLY change is HOW the engine WRITES text: get-or-create a child `LoroText` and `update()` it, instead of `meta.insert("text", ...)`.
Concurrent same-block edits then interleave because each replica splices its own LoroText and Loro merges the splice-sets on import.

**Verified loro 1.12 API (vendored source read):** a tree node's `get_meta()` returns a `LoroMap` (lib.rs:2989); `meta.get_or_create_container(key, LoroText::new())` is idempotent — returns the EXISTING Text handler if present, else inserts (handler.rs:4175). `LoroText::update(s, opts)` Myers-diffs (lib.rs:2442); `update_by_line` is the fallback for very long blocks (UpdateTimeoutError). `LoroText::to_string()` reads it back. Text ops are first-class mergeable (json_schema.rs:991 TextOp Insert/Delete merge by Loro ordering). Snapshot/updates carry Text ops correctly.

**Hazard (must honor):** concurrently inserting DIFFERENT containers at the same key on different peers can overwrite not merge (lib.rs:2132). Mitigation: use a NEW key `text_seq` (distinct from the legacy `text` register) and ALWAYS go through `get_or_create_container` so all writers converge on one container; seed + upsert paths must both use it.

## CRITICAL CAVEAT (necessary-not-sufficient — sequencing)
True char-merge only holds when both edits apply to a SHARED LoroText lineage (shared TreeID for the block). DISJOINT TreeID twins (the `project_multidevice_convergence` case) hold two INDEPENDENT LoroTexts — `update()` against one is a fresh diff, not a CRDT merge. So this fix is correct ONLY on top of the shared-base bootstrap (T7 / Part C era) that keeps devices on one lineage. The convergence test MUST use a SHARED base (not disjoint twins). Where twins still occur, the dedup/heal must merge twin text via `update()` into the survivor (not pick one). Document this; it's why the engine test seeds a shared base.

## The real effort sink (do NOT under-scope)
`peer_genuine_block_changes` (loro_engine.rs ~1846-1968, the Part C discriminator) scans `JsonMapOp::Insert{key:"text"}` ops to rebuild per-block text history + server history + resolve container→block_id. When text becomes a `LoroText`, those become TEXT-container ops, not map-register inserts — **this ~120-line block goes DEAD and must be rewritten** to read text from the LoroText container (or re-derive from `current_block_texts`), and resolve a text-container → its node → block_id. The import heal (457-539) must be re-evaluated: Part C twin case-a (stale re-assert) is largely OBVIATED by real interleave; case-b (genuine edit on a discarded twin) survivors absorb twin text via `update()`. The implementer MUST read these exact lines; do NOT prescribe replacement code (codebase-derived).

## Invariants
1. Two replicas on a SHARED base, each applying a different whole-text BlockUpsert to the SAME block, cross-import → both LoroTexts byte-identical AND an INTERLEAVED merge of both edits (NOT the LWW whole-string pick; the result must differ from picking the higher-(lamport,peer) string).
2. All readers (render, tree_matches_blocks, current_block_texts, flatblock) read via one `read_block_text` helper: prefer `text_seq` LoroText, fall back to legacy `text` register (old snapshots).
3. Backward compat: a doc with only the legacy `text` register still renders correctly (fallback read); a freshly-written block uses `text_seq`. No reseed required to run; `TESELA_LORO_RESEED` rebuilds lean.
4. No wire/client change: OpPayload, diff.rs, web block-ops, FFI, note_tree unchanged. The relay/WS path merges Text deltas automatically.
5. No regression: all convergence + dedup tests green (concurrent_whole_body_clobber, disjoint_history_revert, put_base_diff, snapshot_merge_keeps_local, ws_apply_*, positional_insert*, block_granular_write, the e2e real-socket test, loro_cutover_spike).

## Staged plan (each shippable + testable; subagent-driven, two-stage review)
- **Stage 0 (de-risk):** throwaway compile of the apply arm — `meta.get_or_create_container("text_seq", LoroText::new())` + `text_c.update(text, Default::default())` in BlockUpsert. Confirm types/imports before committing. Ships nothing.
- **Stage 1 (engine container + lazy migration + fallback read):** write `text_seq` LoroText at BlockUpsert apply (2362) + `seed_tree_from_flatblocks` (1540) via get_or_create_container + update. Add `read_block_text(tree, node)` (prefer text_seq container → to_string, else legacy `text` register) and route ALL readers through it (flatblock_from_node 1289, current_block_texts 2005, tree_matches_blocks 1503, test block_text 4523). Existing single-writer behavior unchanged; old snapshots read via fallback. Tests: existing tesela-sync suite passes (rewrite the read_meta_str-for-text tests at 3407/4518 + A_BID/B_BID suite at 4526); add a unit test that a block written via text_seq round-trips through render_note.
- **Stage 2 (whole-text→splice on apply + discriminator rewrite):** apply already calls update() from Stage 1, so concurrent splices interleave at the engine. The dedicated work: rewrite `peer_genuine_block_changes` (1846-1968) to read text from the LoroText container / re-derive from current_block_texts instead of JsonMapOp::Insert{key:"text"}; re-evaluate the import heal (457-539). Shippable: relay path merges Loro deltas automatically. Tests: full tesela-sync + tesela-sync-ffi green.
- **Stage 3 (verify same-block merge — ENGINE + WIRE):** engine convergence test (acceptance below). Then re-arm the server diag + drive web + iPad-sim editing the SAME block concurrently; assert the DIAG-WS shows the merged interleave on the server (not a revert). Claude-driven on the sim.
- **(Deferred, approach c):** web emits true CodeMirror `update.changes` splices for cursor-accurate same-region merges; iOS stays whole-text. Not v1.

## Acceptance (both required)
1. **Engine (deterministic test):** two LoroEngine replicas from a SHARED base for the same note (shared TreeID for the target block, NOT disjoint twins). Replica A applies BlockUpsert(block X, "The quick brown fox"); replica B concurrently applies BlockUpsert(block X, "The quick red fox jumps"). Export each delta, cross-import both ways, commit. Assert: both replicas' LoroText for X byte-identical AND an INTERLEAVED merge (contains both the red/brown-region change AND "jumps", neither side wholly dropped) AND the result is NOT the LWW whole-string pick. Today this FAILS (one side vanishes); post-fix PASSES.
2. **Wire (sim, Claude-driven):** web + iPad-sim edit the SAME block concurrently → the server's converged text is a merge of both, neither lost; DIAG-WS shows no whole-block revert. Then the device test (user).

## Risks
- Discriminator rewrite (1846-1968) is the under-scope trap — require reading those lines.
- Necessary-not-sufficient: merge only on a SHARED base; sequence on top of shared-base bootstrap; test with shared base.
- `update()` is O(n) Myers for huge blocks (>50k chars) → `update_by_line` fallback on UpdateTimeoutError.
- Migration: dual-read (text_seq | legacy text); never write the legacy `text` register again (avoid same-key concurrent-create hazard by using `text_seq`).
- Don't regress Part C/T7/base-diff/dedup; keep the relay a dumb byte pipe (no relay change).
- Tests using read_meta_str for text (3407/4518/4526) need rewriting to read_block_text.

## Build/verify notes
- Server diag (TESELA_DIAG_WRITES) is currently ARMED for wire verification — keep through Stage 3, REMOVE after.
- Live server :7474, log /tmp/tesela-srv.log. iPad sim 60369457 (idb-tappable). Both devices on the current build. Mac IP 100.112.34.59.
