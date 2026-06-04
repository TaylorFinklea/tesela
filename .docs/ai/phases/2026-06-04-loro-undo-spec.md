# Loro UndoManager — CRDT-native cross-block text undo (vim #12)

**Status:** in progress (2026-06-04). Decision: option **C** (Loro `UndoManager`),
chosen over snapshot-stack (B) for Savanne-safe, CRDT-correct text undo.

## Problem (vim bug #12)

`u` only undoes within the current block, is lost after navigating away, and
`ciw` isn't undoable. Root cause: text edits on bound blocks go through
`handleLoroText` which deliberately skips the `OutlinerHistory` snapshot stack
("Loro owns text history") — but no Loro `UndoManager` ever existed, so text
undo fell to per-block CodeMirror history (local to one block, partly bypassed
by `addToHistory:false` on synced dispatches).

## Scope

- **Text undo** → Loro `UndoManager` on the active note doc (this spec).
- **Structural undo** (block create/delete/move/indent/fold/status/tag) stays on
  the existing `OutlinerHistory` snapshot stack — structural ops are NOT local
  Loro commits on the web doc (they round-trip via HTTP block-ops and arrive as
  inbound imports), so the UndoManager can't see them without a much larger
  refactor. Accepted two-mechanism design (the user picked C knowing this).

## Grounded facts (verified in code)

- `note-doc.ts`: per-note `LoroDoc`, `"blocks"` movable tree + per-block
  `text_seq` LoroText. `spliceBlock()` = `text.delete/insert` + `doc.commit()`
  → **local commits** (UndoManager records these). `applyInbound()` = `doc.import`
  → remote ops, NOT recorded. No tree-mutation methods (structure is server-side).
- `active-note-doc.svelte.ts`: singleton holding ONE NoteDoc; opened per focused
  slug; `spliceActiveBlock` + coalesced outbound flush.
- `BlockEditor.svelte:1325` `applyRemoteTextEvent`: `if (batch.by === "local") return;`
  — skips our own splices (already in CM). Otherwise dispatches to CM
  (`externalSync` + `addToHistory:false`) AND calls `onLoroText` → outliner
  `handleLoroText` updates the ParsedBlock model. Every VISIBLE block mounts its
  own BlockEditor + `text_seq` subscription (line ~1861).
- `loro-crdt@1.12.3` `UndoManager(doc, {mergeInterval, maxUndoSteps,
  excludeOriginPrefixes, onPush, onPop})`; `.undo()/.redo()/.canUndo()/.canRedo()`.
  Undo applies inverse ops as **local** commits → events arrive `by: "local"`.
  `onPush(isUndo,range,event)→{value,cursors}` / `onPop(isUndo,meta,range)` carry
  transformed Loro `Cursor[]` for selection restoration.

## Design

1. **NoteDoc owns an UndoManager** created in `open()` (after `createLoroDoc`,
   before bootstrap import — imports aren't recorded). Config:
   `mergeInterval ~1000`, `maxUndoSteps 200`. Disposed/recreated per open.
   Expose `undo()/redo()/canUndo()/canRedo()`.
2. **active-note-doc** exports `undoActive()/redoActive()/canUndoActive()/
   canRedoActive()` and an `isLoroUndoApplying()` flag. `undo()/redo()` set the
   flag true, call the manager, then clear it on the next microtask (covers
   sync + microtask event delivery). After undo, force an outbound flush so the
   inverse ops sync to peers/server.
3. **BlockEditor binding** (`applyRemoteTextEvent`): change the guard to
   `if (batch.by === "local" && !isLoroUndoApplying()) return;` so undo-driven
   local events ARE applied to CM (+ `onLoroText` → blocks model). Every visible
   block's subscription fires → cross-block undo propagates automatically.
4. **`u` / `Ctrl-r` routing** (`undoBlockOp`/`redoBlockOp`): try Loro text undo
   FIRST (`undoActive()`), else `undoOutliner()` (structural), else cm history.
   Text-first matches the common create-then-type flow (undo reverses
   text→structure). KNOWN LIMITATION: type-then-structural-then-`u` mis-orders
   (undoes text before the structural op). A global text/struct timeline fixes
   this — deferred to increment 2.

## Increments

- **1 (core, this pass):** UndoManager in NoteDoc + active-note-doc accessors +
  binding flag + `u`/`Ctrl-r` routing. Verify: typing still syncs (no double
  apply), `ciw`+`u`, cross-block `u`, `Ctrl-r`. Extend the e2e harness.
- **2 (polish):** cursor restoration via onPush/onPop Loro Cursors; vim-accurate
  step boundaries (checkpoint on insert-leave / per normal-mode op).
- **3 (correctness):** global text/struct undo timeline for exact interleave.

## Known limitations (increment 1) — verified by e2e

- **Merge granularity is time-based** (`mergeInterval` 500ms), not per-vim-change.
  Fast typing merges into one undo step; a deliberate operator after a brief
  pause lands on its own step. But typing-then-operating with NO pause merges
  into one step (undo reverts both). Increment 2 = semantic boundaries
  (checkpoint on insert-leave / per normal-mode op).
  - **Increment-2 ATTEMPTED + reverted (2026-06-04).** Loro `UndoManager` has
    `groupStart()`/`groupEnd()`; wired them to vim insert-enter/leave via the
    existing `vim-mode-change` `modeListener` (+ a balance guard in
    active-note-doc, reset on note switch). Mechanically sound, but BLOCKED by
    the live-collab echo: the outbound flush (`scheduleOutboundFlush` per
    splice) makes the server echo the delta back as an inbound `import`, and a
    remote import INSIDE a group auto-ends it (loro's documented behavior) →
    slow typing across a flush still split into 2 steps; cross-block got an
    extra phantom step. So increment 2 first needs ONE of: (a) confirm/fix that
    the web never re-imports its OWN deltas (self-echo suppression on the WS
    path — verify `applyInboundToActive`/the server's per-conn-id echo gate for
    the binary-delta channel), or (b) defer the outbound flush out of the active
    insert group WITHOUT breaking live same-keystroke collab. Reverted to keep
    increment 1 clean; this is a focused WS-echo investigation, not a quick tweak.
- **Text-first routing** means a structural op done AFTER a text edit, then `u`,
  undoes the text first. Increment 3 = a global text/struct timeline.

## Risks

- Touches the recently-stabilized live-collab text path. MUST verify normal
  typing isn't double-applied and remote sync still works (existing
  vim-registers e2e exercises typing; add undo cases).
- Merge granularity: `mergeInterval` groups rapid keystrokes into one `u` step
  (insert-session-ish). Tune if it feels too coarse/fine.
- Journal view: the active doc is the focused page/today; undo is per-active-note.
