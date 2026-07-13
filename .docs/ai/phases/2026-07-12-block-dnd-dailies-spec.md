# Block drag-and-drop in Dailies (web + iOS) — spec

Date: 2026-07-12 · Author: Opus (Lead) · Status: approved by Taylor
Execution: git worktree off `main`; phased Rust → web → iOS.

## Problem

In the Dailies feed you cannot drag a block (with its children) to another day.
No drag affordance exists in `BlockOutliner` at all; the only block moves are
keyboard (Alt-↑/↓, Alt-→, Alt-Shift-←) and intra-note. There is **no cross-note
move primitive** anywhere: the wire `BlockOp::Move` carries no note id and no
order key (`crates/tesela-sync/src/oplog/op.rs:142`), and iOS's `.moveTo`
context-menu verb is a no-op stub.

## Scope (approved)

- Drag = cross-day move **and** within-day reorder **and** re-parent under a
  drop target. One gesture, three landing semantics.
- Surfaces: web (SvelteKit) **and** iOS (SwiftUI, Graphite shell).
- Out of scope: durable-through-Loro ordering (open P1 `tesela-8zd.17` —
  this feature persists via the same note-body/diff paths existing edits use
  and inherits that behavior); wiring the iOS `.moveTo` context-menu item;
  legacy iOS `DailyView` (Graphite `GrDailyView` only).

## Interaction model (identical on both platforms)

- Drag handle: the block bullet.
- Per-row drop zones by cursor Y within the row:
  - top 25% → insert **before** target (same indent as target, sibling)
  - bottom 25% → insert **after** target's subtree (sibling)
  - middle 50% → insert **inside** (first child; indent = target.indent + 1)
- Day-section drop target (header / empty area / preview-only unmounted day):
  append at end of that day at indent 0.
- The dragged block's whole subtree (contiguous slice of strictly-deeper
  indents) always travels; subtree indents rebase by the same delta as the
  root.
- Guards: drop into own subtree = no-op; drop onto self = no-op.
- Visual feedback: insertion line for before/after, highlight ring for inside;
  day-level target highlights on dragover.

## Architecture

Subtree math ("a block + children" = contiguous slice; hierarchy is flat array
+ indent on every platform) exists three times by necessity of runtime, all
tested against equivalent fixtures:

1. **Rust** (canonical): splice helper in `tesela-core`, used by the new
   server route. Cross-note moves are server-side and atomic per request.
2. **TypeScript**: `moveSubtreeTo` in `web/src/lib/block-tree-move.ts` for
   *same-day* moves (extends the existing `moveSubtreeUp/Down/Under` family).
3. **Swift**: small `BlockSubtree` helper for iOS, since iOS writes bypass the
   HTTP server entirely (UniFFI engine + relay; whole-day re-render via
   `renderBody`/`recordNoteDiff`).

## Server: `POST /blocks/move` (web cross-day path)

Request body (spec-derived, exact):

```json
{
  "bid": "<uuid>",
  "from_note_id": "<note id>",
  "to_note_id": "<note id>",
  "parent_bid": "<uuid> | null",
  "after_bid": "<uuid> | null"
}
```

Semantics:

- `parent_bid = null` → root level in destination. `after_bid = null` →
  first position under the parent (or top of note); otherwise insert after
  `after_bid`'s **subtree end**.
- `from_note_id == to_note_id` is allowed (server-side same-note move) but the
  web client uses the TS path for that; don't optimize for it.
- Loads both notes, computes splice with the `tesela-core` helper, records the
  resulting ops for both notes through the sync engine **in one handler** —
  atomic from the client's perspective.
- Errors: 404 unknown note or bid; 409 if `parent_bid`/`after_bid` is inside
  the moved subtree or not found in destination; 422 if the moved block has no
  bid. Error body mirrors existing route error shape.
- Response: the updated parsed blocks of **both** notes so the client can
  refresh both days without refetch:
  `{ "from_blocks": [...], "to_blocks": [...] }` (ParsedBlock wire shape).

Implementation notes (codebase-derived — read, then mirror; do not trust this
spec for signatures):

- Mirror the `upsert_blocks` handler (`crates/tesela-server/src/routes/notes.rs:530`)
  for note loading, `stable_uuid_from_slug`, and `sync_engine.record_local`
  op recording (delete ops on source + upsert ops carrying `note_id` on
  destination — `OpPayload::BlockUpsert` already has `note_id`).
- Destination note must exist on disk (`upsert_blocks` 404s otherwise); the
  web client guarantees this (see below). Do not add server-side note
  creation.

## Web client

UI (`BlockOutliner.svelte` + `JournalView.svelte`):

- Native HTML5 drag events — no library. Mirror `KanbanBoard.svelte`
  (`handleCardDragStart` etc.) for the event pattern.
- Bullet gets `draggable="true"`. `dataTransfer` payload: JSON under a custom
  type `application/x-tesela-block` = `{ noteId, bid, blockId }`; also set
  `text/plain` to the block id for inertness elsewhere.
- Rows compute drop zone on dragover (Y vs quartiles), render indicator.
- `JournalView` registers day-level drop targets, including preview
  (unmounted) days.

Persistence:

- **Same day**: new pure `moveSubtreeTo(blocks, blockId, target)` in
  `block-tree-move.ts` where
  `target = { kind: "before" | "after" | "inside", blockId } | { kind: "end" }`.
  Returns `TreeMoveResult`. Persist exactly like `handleMoveBlock` does today
  (whole-body save path with base) — same durability as Alt-↑/↓.
- **Cross-day**: call new `api.moveBlock(...)` → `POST /blocks/move`; on
  success apply `from_blocks`/`to_blocks` to the two day outliners. On failure
  leave local state untouched and surface the existing save-error affordance.
- Preconditions before a cross-day call: flush pending saves for the source
  day (block must have a `bid`; if missing, flush then re-read, mirroring
  `saveBlocksViaOps` null-op fallback); if destination is a synthetic day,
  run JournalView's existing lazy-create first (`flushSave` /
  `api.getDailyNote` path — read it, mirror it).

## iOS client (Graphite shell only)

- `BlockRow` gets `.draggable` / `.dropDestination` (native SwiftUI,
  Transferable payload carrying block id + note id/day key). Same three-zone
  semantics via drop location Y within the row frame. Day sections in
  `GrDailyView` accept drops for end-append.
- New `BlockSubtree` helper (pure, unit-tested): subtree slice bounds for a
  flat `[Block]` by indent, splice-out + splice-in with indent rebase, own-
  subtree guard. Mirror the semantics (and port the fixtures) of the TS
  `block-tree-move` tests.
- Same-day move: mutate the day's block array via the helper, then the
  existing writeback (`scheduleWriteback` for today; the equivalent existing
  push path for other days — read `MockMosaicService` and use what each day
  section already uses).
- Cross-day move: splice out of source array, into destination array, write
  back **both** days through the existing engine path (two note diffs;
  local-first, failure semantics identical to any other edit).
- No new FFI surface.

## Error handling summary

- Web cross-day: server call is all-or-nothing; UI applies result only on
  success. No optimistic destructive state.
- Blocks without `bid`: flush-then-retry once; if still no bid, abort with
  the save-error affordance (never silently drop).
- iOS: local-first engine writes; no new failure modes.

## Testing / Verify

- Rust: unit tests for the `tesela-core` splice helper (before/after/inside/
  end, indent rebase, own-subtree rejection); `tesela-server` route test
  (cross-note move, into empty note, 409 own-subtree, 404s).
  Verify: `cargo test -p tesela-core -p tesela-server`
- Web: `moveSubtreeTo` unit tests alongside `tests/unit/block-tree-move.test.mjs`;
  Playwright e2e: drag block with child to another day, assert both days'
  persisted content.
  Verify: `pnpm --dir web check && pnpm --dir web test:unit && pnpm --dir web test:e2e`
- iOS: unit tests for `BlockSubtree` (ported fixtures).
  Verify: `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`

## Phasing

1. Rust: `tesela-core` splice helper + `POST /blocks/move` route (TDD).
2. Web: `moveSubtreeTo` + outliner/journal DnD UI + persistence wiring.
3. iOS: `BlockSubtree` + `BlockRow`/`GrDailyView` DnD + writeback wiring.

Each phase lands as its own commit(s) with its Verify green before the next
starts.
