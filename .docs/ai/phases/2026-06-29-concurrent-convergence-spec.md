# Spec: complete concurrent multi-device convergence (shared-base-before-authoring)

Status: SPEC (2026-06-29). Root cause CONFIRMED via 4 diagnosis passes + live evidence.
Builds on the committed partial fixes (see Done). The convergence-critical remaining
fix; implement deliberately + TDD (do NOT rush — riskiest area in the codebase).

## Symptom (Taylor's daily-driver, real)
Desktop daily showed `Bothnice onenice one` = `"Both"` (iPhone) + `"nice one"` + `"nice one"`
(desktop) — SEPARATE intended blocks' text CONCATENATED into ONE block (bid c35861c0), plus
persistent divergence (iPhone missing blocks the desktop had). Happens under CONCURRENT
multi-device editing of the same daily; sequential editing is fine.

## Root cause (confirmed)
`doc_for_note_mut` (crates/tesela-sync/src/engine/loro_engine.rs:1528-1537) does
`docs.entry(note_id).or_insert_with(|| LoroDoc::new())` — a BARE FRESH per-device Loro doc,
with NO deterministic seed and NO relay bootstrap. So when two devices each first author the
same note (e.g. today's daily) before syncing, each gets a **disjoint Loro lineage** (different
TreeIDs, no shared base). On merge Loro UNIONS the two lineages' `text_seq` insert-runs →
concatenation (`Both`+`nice one`+...), and/or makes same-bid TWINS that the non-recency-aware
min-TreeID dedup resolves lossily.

- The committed `write_block_text` fix (idempotent + minimal-UTF16-diff splice, `8171b0b8`)
  makes WITHIN-lineage writes convergent — but does NOT fix the CROSS-lineage union.
- My `bootstrap-when-behind` fix (`131a1039`) unions the disjoint lineages, which SURFACES the
  union (correct behavior; the disjoint authoring is the actual bug).

## The pattern that already solves this (mirror it)
`views_upsert` (loro_engine.rs:1928-1957): before the FIRST write to a builtin view, it
`doc.import(&builtin_views_seed_update())` so the write lands in "THE canonical seed container —
never a fresh same-key container that races the group's and drops the loser's fields wholesale
on merge." That is shared-base-before-authoring. `doc_for_note_mut` (the generic note path) does
NOT do this — that asymmetry is the bug.

## Design — shared-base-before-authoring
A device must never AUTHOR a note's content on a bare fresh disjoint doc. Establish a shared base
first:

1. **Bootstrap-before-author (relay HAS the note).** Before the first local author of a note,
   if the relay/snapshot has it, import that authoritative doc first (the shared base) so edits
   are granular splices on it. iOS already has `relayTicker.bootstrapNoteIfNeeded(slug:)`
   (idempotent resident-check) — wire it into the authoring path; desktop has bootstrap_from_snapshots.
2. **Deterministic daily seed (relay has NO note yet — concurrent first creation).** When neither
   device has the note and the relay has none (the true first-create race, esp. today's daily),
   both devices must derive a BYTE-IDENTICAL base so their lineages share a root. Build a daily
   seed = root node + `slug` + ONE empty placeholder block, generated with a FIXED seed peer-id
   (e.g. peer 0) and a DETERMINISTIC placeholder bid (`UUIDv5(namespace, date)`), so the seed
   update bytes are identical on every device. `doc_for_note_mut` (or a daily-aware creation
   wrapper) imports this seed instead of `LoroDoc::new()` for a daily. Then each device authors
   with its OWN peer on the shared base → granular merge, no union.
3. **iOS pre-materialization path.** `MockMosaicService.spliceTodayBlock` (~655) + the
   pre-materialization gate (~708-724) route the first edit of an unmaterialized daily through
   whole-content `scheduleWriteback` (engine diff CREATES the block on a fresh disjoint doc). Fix:
   ensure today's daily is bootstrapped (relay) or deterministically seeded BEFORE the first
   edit, so even the first edit is a granular splice on the shared base — never a disjoint
   whole-content lineage. (bootstrapNoteIfNeeded before the first splice.)
4. **Recency-aware twin resolution (fallback).** If disjoint twins still form, replace the
   min-TreeID survivor (loro_engine.rs:3298-3321, "NOT recency-aware") with a recency-aware pick
   so neither union (garble) nor stale-twin clobber occurs. This is a SAFETY NET; the primary
   fix is (1)+(2)+(3) eliminating twins.

## Semantics
- Concurrent edits to the SAME block on a SHARED base → character-merge (correct; granular splice
  path, already works — `splice_block_text_concurrent_inserts_interleave`).
- Residual true-concurrent-DISJOINT (should be eliminated by 1-3) → deterministic recency pick.
  NEVER union, NEVER lossy min-TreeID.

## TDD plan (write first, watch fail)
In crates/tesela-sync (mirror existing two-engine + in-process relay scaffolding):
1. `daily_concurrent_disjoint_first_create_converges`: engine A and engine B BOTH create today's
   daily (no shared base) and each add a distinct block, then converge via the relay → assert ONE
   daily doc, BOTH blocks present as SEPARATE blocks, NO concatenated text, NO dropped block.
2. `shared_base_concurrent_different_blocks_clean`: with a shared base, A and B edit DIFFERENT
   blocks concurrently → both survive, no garble (regression guard).
3. `local_unbroadcast_edit_survives_shared_base_bootstrap`: B has a local un-broadcast edit; a
   bootstrap/seed import must not drop it.
4. Keep `splice_block_text_concurrent_inserts_interleave` + the `8171b0b8` write_block_text tests
   green.

## Implementation order + files (each its own commit + verify)
1. Deterministic daily seed builder + `doc_for_note_mut` daily-aware seeding —
   crates/tesela-sync/src/engine/loro_engine.rs. TDD #1.
2. Bootstrap-before-author wiring (desktop relay tick + the author entry points). 
3. iOS: bootstrapNoteIfNeeded before first splice of an unmaterialized daily —
   app/Tesela-iOS/Sources/Data/MockMosaicService.swift (spliceTodayBlock + gate).
4. Recency-aware twin resolution — loro_engine.rs:3298-3321 (+ FFI/iOS mirror if needed).

## Risks (honest)
- `doc_for_note_mut` is hot-path (every note access) — daily-seeding must NOT regress non-daily
  notes or single-device. Keep the bare-fresh path for genuinely-new non-shared docs; seed only
  where a shared base is expected.
- The deterministic daily seed (fixed peer-id + UUIDv5 bid → identical bytes) is NEW design;
  verify two independent builds produce BYTE-IDENTICAL seed ops (else lineages still diverge).
- iOS gate reorder (bootstrap before first splice) could add a first-edit latency or, if
  mis-sequenced, drop the first keystrokes — sequence carefully + test.
- Recency-aware twin resolution must not re-introduce the stale-twin revert the min-TreeID rule
  was guarding; couple with eliminating twins (1-3) so it rarely fires.
- Existing already-garbled blocks (Taylor's today) do NOT self-heal — manual cleanup; consider a
  gated one-shot repair later (dry-run + raw dump first).

## Done (committed this arc)
- Past-day convergence heal-deposit (`cf212bee`).
- Bootstrap-when-behind compaction watermark + relay X-Tesela-Compaction-Seq header (`131a1039`).
- Convergent idempotent write_block_text (`8171b0b8`).
