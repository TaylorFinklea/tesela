# Loro Migration — Pre-commitment Spike Spec

*Drafted 2026-05-27 morning, ahead of the 8–10 week migration committed in [decisions.md](../decisions.md#2026-05-27).*

## Purpose

Find structural blockers in an afternoon, not three weeks in. The migration's calendar cost is significant; we don't want to discover halfway through that loro-swift can't be linked into iOS or that snapshot size blows iCloud quota. The five spike items below are the questions whose answers most-aggressively gate the timeline.

If **all five are green**, the timeline commitment stands and we scaffold the dual-write feature flag next.
If **any one is red**, we re-evaluate: workaround possible? fall back to patch-then-migrate-later with a hard Q1 2027 deadline? abandon Loro?

## Spike items

### 1. UniFFI compatibility with loro-swift

**Question:** Does Loro's Swift binding (`loro-swift`) coexist with our existing UniFFI flow (`tesela-sync-ffi`), or does it require a separate binding generator that fights with ours?

**How to check:**
- Open Loro's GitHub. Check what binding generator they use (UniFFI? Swift's manual `extern "C"`? Some other tool?).
- Read their iOS integration docs.
- Check `loro-swift` package — what's the linkage shape? `.xcframework`? CocoaPods? SwiftPM?
- Verify it can sit alongside our `target/aarch64-apple-ios{,-sim}/release/libtesela_sync_ffi.a` static lib without symbol conflicts.

**Green criteria:** Loro can be added as a SwiftPM dep or static lib, and both libraries' symbols don't collide. We can call `LoroDoc.new()` from Swift after adding the dep.

**Yellow criteria:** Requires moderate plumbing changes (e.g., regenerate bindings with a different tool, or restructure the FFI crate). Doable in 1–2 days.

**Red criteria:** Structurally incompatible — Loro requires us to abandon UniFFI for our own code, or vice versa. Would mean rewriting our existing FFI surface.

### 2. Snapshot size vs current SQLite oplog

**Question:** When we import Taylor's existing notes corpus into a Loro doc, how big is the resulting snapshot? Is it acceptable for iOS storage + iCloud backup + relay transit?

**How to check:**
- Find a representative-sized note in Taylor's mosaic (`~/Library/Application Support/tesela/logseq/notes/`). Daily notes work well.
- Programmatically: create a Loro doc, parse the note's blocks via the existing `parse_note`, build the equivalent Loro tree, serialize to snapshot bytes. Measure.
- Compare against the current SQLite oplog size for the same note (rows in `oplog` table where the note_id matches, summed by encoded length).

**Green criteria:** Loro snapshot ≤ 5x the SQLite oplog. For a typical note this means under ~10 KB.

**Yellow criteria:** 5x–20x. Means iOS storage isn't a concern but iCloud quota for huge mosaics could be. Worth documenting.

**Red criteria:** > 20x or > 100 KB for a typical note. iOS storage / relay bandwidth concerns become real.

### 3. Apply-changes latency

**Question:** How fast does Loro apply ~100 representative ops? Sub-millisecond per op is the bar; Loro 1.x has had perf cliffs in specific scenarios worth catching early.

**How to check:**
- Generate 100 ops representing a realistic edit sequence (mix of `LoroText.insert`, `LoroMap.set`, `LoroMovableList.insert/move/delete`).
- Time `doc.import(batch)` (or equivalent).
- Repeat 10x to get a median.

**Green criteria:** < 100 ms total for 100 ops on Taylor's M-series Mac. Should comfortably fit inside a 5-second relay tick.

**Yellow criteria:** 100 ms – 1 s. Workable but worth profiling before going wider.

**Red criteria:** > 1 s. Sync ticks would feel laggy; pathological.

### 4. Move-op semantics parity

**Question:** Does Loro's move-op handling match what we want when one device moves block A under block B while another device concurrently edits block A's text?

**How to check:**
- Build the scenario in the current hand-rolled engine: device 1 emits `BlockMove(A, parent: B)`; device 2 emits `BlockUpsert(A, text: "modified")`. Both ops arrive on a third device. What's the result?
- Build the same scenario in Loro using `LoroMovableList` for blocks. Compare.
- Specifically check the cycle-detection edge: device 1 moves A under B; device 2 moves B under A. What happens?

**Green criteria:** Both edits visible in final state (text from device 2's edit, position from device 1's move). Cycle-creating concurrent moves resolve deterministically — Loro picks one, drops the other.

**Yellow criteria:** Semantics differ from current engine but Loro's choice is also defensible. Document the change in expected behavior.

**Red criteria:** Loro's semantics are surprising in a way that breaks user intent.

### 5. Oplog → Loro doc one-way import path

**Question:** Can we cleanly convert Taylor's existing SQLite oplog into a Loro doc on first migration boot? Specifically: walk the oplog in HLC order, apply equivalent Loro ops, verify materialized output matches.

**How to check:**
- Pick the daily note (`2026-05-27.md`). It has many bid-stamped blocks now.
- Walk Taylor's `oplog` table rows for that note (`SELECT * FROM oplog WHERE note_id = ? ORDER BY hlc`).
- For each `OpPayload` variant, sketch the equivalent Loro doc operation:
  - `NoteUpsert` → set the doc's `body` map or replace the blocks list
  - `BlockUpsert` → `LoroMovableList.insert` or `LoroText.replace_with`
  - `BlockMove` → `LoroMovableList.move`
  - `BlockDelete` → `LoroMovableList.delete`
- After applying all ops, materialize the Loro doc back to markdown. Compare against the current `2026-05-27.md` file. Should match byte-for-byte (or close to it, modulo formatting).

**Green criteria:** Materialized output matches current file. Import logic is straightforward.

**Yellow criteria:** Output close but not byte-identical. Format normalization needed but doable.

**Red criteria:** Op model doesn't map cleanly. Migration requires rethinking what's stored.

## Output

A markdown report at `.docs/ai/phases/2026-05-27-loro-spike-report.md` with:
- Green/Yellow/Red verdict for each of 5 items
- For yellows, what to plan around
- For reds, what's blocked and what the alternative path looks like
- Final go/no-go recommendation
