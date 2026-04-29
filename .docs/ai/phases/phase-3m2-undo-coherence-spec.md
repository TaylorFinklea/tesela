# Phase 3M.2 — Undo Coherence Pass

## Product Overview

Phase 3M.1 unified vim's `u` so insert sessions are atomic alongside structural ops. Three coherence bugs remain in that workflow: (1) a freshly-redone empty block remounts into Insert mode unexpectedly, (2) cm6's per-block history retains the `externalSync` transactions that outliner-undo dispatches and so cm6's local `Cmd+Z` can walk back through the just-undone state, and (3) a debounced PUT in flight when the user hits `u` can race the restored state — server saves the pre-undo body, the WS echo arrives, and (in the lost-focus edge case) the external-body reparse reverts the undo.

Audience: Taylor, daily-driver. Goal: `u` and `Ctrl+R` always do exactly what vim users expect, with no remount-into-Insert surprises and no save-race regressions.

## Current State

Phase 3M.1 ships at commit `8ab8492`. The unified-undo plumbing already exists:

- `web/src/lib/components/BlockOutliner.svelte:166-192` — `OutlinerHistory` instance, `pushUndo`, `applySnapshot`, `undoOutliner`, `redoOutliner`.
- `web/src/lib/components/BlockOutliner.svelte:199-211` — `pendingInsertSnapshot`, `beginInsertSession`, `endInsertSession`.
- `web/src/lib/components/BlockOutliner.svelte:172-178` — `applySnapshot` calls `saveBlocks(blocks)` which sets `lastSentBody` and fires `onContentChange`.
- `web/src/lib/components/BlockOutliner.svelte:318-328` — external-body `$effect`: reparses only when `body !== lastSentBody && focusedIndex === null`.
- `web/src/lib/components/BlockOutliner.svelte:944` — `startininsert` heuristic: `(mountHint?.blockId === block.id && mountHint.startInInsert) || (focusedIndex === vi && block.raw_text === "" && !autoFocused)`.
- `web/src/lib/components/BlockEditor.svelte:205` — `externalSync = Annotation.define<boolean>()`.
- `web/src/lib/components/BlockEditor.svelte:607-616` — prop→cm6 sync `$effect` dispatches `changes` annotated with `externalSync.of(true)`.
- `web/src/lib/components/BlockEditor.svelte:807-835` — `updateListener` skips transactions whose `externalSync` annotation is `true`.
- `web/src/lib/components/BlockEditor.svelte:184-194` — vim `u` / `<C-r>` actions wired to `vimCtx.undoOutliner` / `vimCtx.redoOutliner` with cm-editor undo as fallback.
- `web/src/routes/p/[id]/+page.svelte:171,188-202` — `saveTimer` 500ms debounce. `handleContentChange(fullContent)` schedules `api.updateNote(noteId, fullContent)`. No AbortController; in-flight fetches are not cancellable today.
- `web/src/routes/timeline/+page.svelte:60-76` — duplicate save pattern, `saveTimers: Map<string, ReturnType<typeof setTimeout>>` per note.
- `web/src/lib/api-client.ts:46-55,68-69` — `put` uses `fetch(url, { method: "PUT", ... })` with no signal plumbing.
- `web/src/lib/ws-client.svelte.ts:131-141` — WS echo handler invokes `onNoteUpdated`, which in `+layout.svelte:50-54` invalidates `["note", id]`. The refetched body lands as the `body` prop on `BlockOutliner`, triggering the external-body `$effect`.

The `Transaction.addToHistory` annotation (from `@codemirror/state`) is the canonical primitive for excluding a transaction from cm6's history; it is already imported transitively via `EditorState`.

## Implementation Plan

The three sub-fixes are independent at the file level but share the workflow they affect. Implement in this order — each builds on the previous.

### Step 1 — Block remount-into-Insert after Ctrl+R (item 4)

Goal: when `redoOutliner` restores a newly-created empty block, BlockEditor must NOT auto-enter Insert on remount.

1. In `web/src/lib/components/BlockOutliner.svelte`, add a new `$state` flag near `autoFocused` (line 158):
   ```ts
   // True when the most recent focusedIndex change came from undo / redo —
   // suppresses the empty-block→Insert remount heuristic for one render so
   // restored empty blocks land in Normal, not Insert. Cleared on any user-
   // initiated focus change (click, navigate, new-block creation).
   let restoredFocus = $state(false);
   ```
2. In `applySnapshot` (line 172), set `restoredFocus = true;` immediately after assigning `focusedIndex = s.focusedIndex;`. Place the assignment so it lands in the same render tick as the new `focusedIndex`.
3. In the BlockEditor `startininsert` prop expression at line 944, add `&& !restoredFocus` to the second clause:
   ```svelte
   startininsert={(mountHint?.blockId === block.id && mountHint.startInInsert) || (focusedIndex === vi && block.raw_text === "" && !autoFocused && !restoredFocus)}
   ```
4. Clear `restoredFocus = false;` on every existing user-initiated focus change. The places that already set `autoFocused = false` are the right hooks:
   - `handleEnter` (line 524)
   - `handleNewBlockAbove` (line 656)
   - The per-row `onfocus` handler in the each-block render (line 935: `onfocus={() => { focusedIndex = vi; autoFocused = false; }}`)
   - The empty-state click handler (line 854)
   In each, append `restoredFocus = false;` next to the existing `autoFocused = false;`.
5. Also clear `restoredFocus = false;` inside `handleNavigate` (line 454) — j/k movement is user-initiated even though `autoFocused` is unaffected there. (Skip blocks that don't change focusedIndex; the assignment is harmless either way.)

Why a flag and not a per-mutation `viaRedoRestore` field on the snapshot: the snapshot is also used by `popUndo` for both undo AND redo, and the suppression applies symmetrically (undoing a paste-of-empty-block should also not enter Insert). A single flag set in `applySnapshot` covers both code paths.

### Step 2 — cm6 history coherence after outliner undo (item 3)

Goal: the `externalSync` transaction must NOT land in cm6's per-block history. After `u` (or any other source of an externalSync write), pressing cm6's local `Cmd+Z` (when the outliner stack is empty) must NOT walk back through the just-restored state.

1. In `web/src/lib/components/BlockEditor.svelte`, import `Transaction` from `@codemirror/state` (already imports `Annotation`, `Compartment`, `EditorState` from that package — line 200). Update the import to include `Transaction`:
   ```ts
   import { Annotation, Compartment, EditorState, Transaction } from "@codemirror/state";
   ```
2. In the prop→cm6 sync `$effect` at line 607-616, add the `Transaction.addToHistory.of(false)` annotation to the dispatch:
   ```ts
   v.dispatch({
     changes: { from: 0, to: v.state.doc.length, insert: initialText },
     annotations: [
       externalSync.of(true),
       Transaction.addToHistory.of(false),
     ],
   });
   ```
   `Annotation.define` returns a single annotation, but `dispatch` accepts an array of annotations. Convert the existing single value to the array form.

This is a one-line semantic change: cm6's `history()` extension reads `Transaction.addToHistory` and excludes any transaction marked `false` from being a history entry. No other code paths need to change — the existing `updateListener` skip-on-externalSync logic stays correct.

### Step 3 — Cancel in-flight saves on undo (item 2)

Goal: when `applySnapshot` runs, any in-flight or pending PUT for this note must be canceled and the restored body must be PUT immediately so the server's WS echo never carries the pre-undo state.

This requires plumbing a cancel-and-flush hook from BlockOutliner up to the page that owns the saveTimer, and adding AbortController support in the API client.

#### 3a. AbortController in api-client

In `web/src/lib/api-client.ts`:

1. Extend `put<T>` to accept an optional `signal: AbortSignal`:
   ```ts
   async function put<T>(path: string, body: unknown, signal?: AbortSignal): Promise<T> {
     const url = `${BASE_URL}${path}`;
     const res = await fetch(url, {
       method: "PUT",
       headers: { "Content-Type": "application/json", Accept: "application/json" },
       body: JSON.stringify(body),
       signal,
     });
     if (!res.ok) throw new ApiError(res.status, await res.text(), url);
     return (await res.json()) as T;
   }
   ```
2. Extend `api.updateNote` to accept and forward the signal:
   ```ts
   updateNote: (id: string, content: string, signal?: AbortSignal) =>
     put<Note>(`/notes/${encodeURIComponent(id)}`, { content }, signal),
   ```
   No existing caller passes the signal, so this is purely additive.

#### 3b. Cancel-and-flush API on the page

In `web/src/routes/p/[id]/+page.svelte`:

1. Replace the simple `saveTimer` with an in-flight tracker. After `let saveTimer: ReturnType<typeof setTimeout> | null = null;` (line 171), add:
   ```ts
   let inFlightController: AbortController | null = null;
   let pendingContent: string | null = null;
   ```
2. Refactor `handleContentChange` (line 188-202) to track `pendingContent` and run the PUT with the controller:
   ```ts
   function handleContentChange(fullContent: string) {
     pendingContent = fullContent;
     if (saveTimer) clearTimeout(saveTimer);
     setSaving();
     saveTimer = setTimeout(() => flushSave(), 500);
   }

   async function flushSave() {
     if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
     if (pendingContent === null) return;
     const content = pendingContent;
     pendingContent = null;
     // Cancel any in-flight PUT — its result would race ours.
     if (inFlightController) inFlightController.abort();
     const controller = new AbortController();
     inFlightController = controller;
     try {
       const updated = await api.updateNote(noteId, content, controller.signal);
       if (controller.signal.aborted) return;
       queryClient.setQueryData(["note", noteId], updated);
       setSaved();
     } catch (e) {
       if ((e as { name?: string })?.name === "AbortError") return;
       const msg = e instanceof Error ? e.message : "Unknown error";
       setSaveError(msg);
       console.error("Save failed:", e);
     } finally {
       if (inFlightController === controller) inFlightController = null;
     }
   }

   /**
    * Cancel any pending or in-flight PUT and immediately PUT `fullContent`.
    * Called by BlockOutliner from applySnapshot so the server's WS echo
    * carries the restored body, not the pre-undo body. No-op if no save
    * was pending and no in-flight save can be displaced.
    */
   function cancelAndFlush(fullContent: string) {
     pendingContent = fullContent;
     if (saveTimer) { clearTimeout(saveTimer); saveTimer = null; }
     if (inFlightController) {
       inFlightController.abort();
       inFlightController = null;
     }
     // Fire immediately — bypass the 500ms debounce.
     flushSave();
   }
   ```
3. Pass `cancelAndFlush` as a new prop to BlockOutliner. Add to the `<BlockOutliner ...>` invocation around line 298:
   ```svelte
   onCancelAndFlush={cancelAndFlush}
   ```

#### 3c. Wire BlockOutliner to call cancel-and-flush

In `web/src/lib/components/BlockOutliner.svelte`:

1. Add the new prop to the props destructure (after `onContentChange?:` at line 35):
   ```ts
   onCancelAndFlush?: (fullContent: string) => void;
   ```
   And to the destructure variable list at the top of `$props()`. Use the same rename pattern as `onContentChange`.
2. In `applySnapshot` (line 172-178), replace the existing `saveBlocks(blocks)` call with the cancel-and-flush version. The trick: `saveBlocks` builds the body string and updates `lastSentBody`, but does NOT actually PUT — it calls `onContentChange?.(fullContent)` which the page handles. We need both the immediate flush AND the `lastSentBody` update.

   The cleanest refactor: extract a `buildFullContent(blocks)` helper, set `lastSentBody`, then call `onCancelAndFlush?.(fullContent)` instead of `onContentChange?.(fullContent)`. Implementation:
   ```ts
   function buildFullContent(updated: ParsedBlock[]): { full: string; bodyOnly: string } {
     const bodyLines = updated
       .map((b) => {
         const indent = "  ".repeat(b.indent_level);
         const lines = b.raw_text.split("\n");
         const first = `${indent}- ${lines[0]}`;
         const rest = lines.slice(1).map((l: string) => `${indent}  ${l}`);
         return [first, ...rest].join("\n");
       })
       .join("\n");
     return { full: `${frontmatter}${bodyLines}\n`, bodyOnly: `${bodyLines}\n` };
   }

   function saveBlocks(updated: ParsedBlock[]) {
     const { full, bodyOnly } = buildFullContent(updated);
     lastSentBody = bodyOnly;
     onContentChange?.(full);
   }

   function saveBlocksImmediate(updated: ParsedBlock[]) {
     const { full, bodyOnly } = buildFullContent(updated);
     lastSentBody = bodyOnly;
     // Prefer the cancel-and-flush path so the in-flight pre-undo PUT cannot
     // win the race; if the parent didn't wire it, fall through to the
     // debounced path so behavior degrades gracefully.
     if (onCancelAndFlush) onCancelAndFlush(full);
     else onContentChange?.(full);
   }
   ```
3. In `applySnapshot` (line 176), replace `saveBlocks(blocks);` with `saveBlocksImmediate(blocks);`. Leave every other call to `saveBlocks` alone — they are user-typed-mutation paths that should keep the debounce.

#### 3d. Optional: timeline page parity

The timeline page (`web/src/routes/timeline/+page.svelte`) hosts BlockOutliner too. It already passes `onContentChange` per-note via `handleContentChange(noteId, content)`. To keep behavior consistent:

1. Mirror the pattern at line 60-76: maintain `inFlightControllers: Map<string, AbortController>` and `pendingContent: Map<string, string>`.
2. Add a `cancelAndFlush(noteId, fullContent)` and pass it as `onCancelAndFlush={(content) => cancelAndFlush(note.id, content)}` to `<BlockOutliner>` at line 127.

If time-boxed, ship without this and add a `// TODO(3M.2)` comment near line 60 — undo on the timeline page will degrade to the existing focus-guarded behavior, which is the status quo.

### Step 4 — Test build

1. `pnpm --dir web tsc --noEmit` — must pass; the new prop and types are additive.
2. `pnpm --dir web lint` — must pass.

## Interfaces and Data Flow

- New API client signature: `api.updateNote(id, content, signal?: AbortSignal)`. Backwards-compatible.
- New BlockOutliner prop: `onCancelAndFlush?: (fullContent: string) => void`. Optional; if absent, `applySnapshot` falls through to the existing debounced `onContentChange` path.
- New BlockEditor transaction annotation: prop→cm6 sync now sends both `externalSync.of(true)` AND `Transaction.addToHistory.of(false)`. No new exports.
- New private state on BlockOutliner: `restoredFocus: boolean`. No public surface.

No server-side changes. No new endpoints. No schema or migration changes.

## Edge Cases and Failure Modes

- **Aborted PUT mid-flight**: the AbortController abort throws `AbortError`. The `flushSave` wrapper checks for `e.name === "AbortError"` and returns silently — does NOT call `setSaveError`. Critical: a stale aborted PUT must not trip the UI into a "save failed" state.
- **Cancel-and-flush with no pending save**: `pendingContent === null` and `inFlightController === null` is the steady state. `cancelAndFlush` still PUTs (the snapshot's restored body); set `pendingContent = fullContent` first, then fire `flushSave()`.
- **WS echo of the cancel-and-flushed PUT**: lands with `body === lastSentBody` (we updated it before the PUT) → reparse skipped by the existing equality guard. No regression.
- **WS echo of the aborted pre-undo PUT**: the server may or may not have processed the aborted request before abort took effect. If it did, the WS event broadcasts the pre-undo state. Two protections:
  1. The existing `focusedIndex !== null` guard skips the reparse while the user is focused (the common case during typing+undo).
  2. The cancel-and-flush PUT lands shortly after; the server's NEXT WS event carries the restored body, which equals `lastSentBody` → no-op.
- **`u` with a snapshot that has no content delta** (e.g. tag toggle on already-tagged block): still a valid undo — `applySnapshot` runs, `saveBlocksImmediate` PUTs the (semantically identical) restored body, the AbortController machinery is still exercised. No-op for the user, correct for invariants.
- **`Ctrl+R` on the redo of an empty newly-created block while focused on a different block**: the per-row `onfocus` clears `restoredFocus`; if focus lands on the restored empty block via the snapshot's `focusedIndex`, the `restoredFocus` flag suppresses Insert. Verified by step-1's `restoredFocus = true` set immediately after `focusedIndex = s.focusedIndex`.
- **Insert-session promotion happens AFTER `Ctrl+R`**: the user redoes, lands in Normal on the restored block, types `i` to enter Insert manually. `vim-mode-change` listener fires `beginInsertSession`; `restoredFocus` is no longer relevant. Subsequent typing promotes the session normally.
- **Compound undo across page navigation**: not a regression — `history.clear()` already runs on noteId change (line 333-337) and external-body reparse (line 326). Step 3's cancel-and-flush only runs when `applySnapshot` is invoked, which only happens within the current page.
- **Item 3 collateral**: cm6's local undo previously could "undo" through an externalSync transaction. After step 2, the externalSync transaction is invisible to cm6 history — local `Cmd+Z` walks back through real user edits only. Verify that cm6's `historyKeymap` (line 846 in BlockEditor) still binds `Cmd+Z` at all (it does: `historyKeymap` includes `{ key: "Mod-z", run: undo }`); only the contents of history change.
- **Mode-change race during applySnapshot**: applySnapshot does NOT touch vim mode. If the user is in Insert when they trigger an outliner-undo by some path that doesn't go through vim's `u` (shouldn't happen via mappings, but could via a keyboard shortcut layer added later) — pendingInsertSnapshot is cleared by the next `vim-mode-change` to non-insert. No code change required, but document this in the implementer's mental model.

## Test Plan

### Build verification

```sh
pnpm --dir web tsc --noEmit
pnpm --dir web lint
```

Both must pass. No Rust changes; skip cargo.

### Manual QA — item 4 (block remount into Insert after Ctrl+R)

1. Open a note in `/p/<id>`. Place focus on any block.
2. Press `o` to create an empty block below; vim should be in Insert. Press Esc → Normal.
3. Press `dd` to delete the new empty block.
4. Press `u` — block returns; cursor lands on it; mode is Normal. Verify the StatusBar reads `NORMAL`.
5. Press `Ctrl+R` — block deleted again.
6. Press `u` again — block returns. **Confirm StatusBar shows `NORMAL`, not `INSERT`.**
7. Repeat step 5–6 several times. Mode must remain Normal across all redo→undo cycles.
8. Press `i` — manually enters Insert. Type a character. Press Esc → Normal. Press `u` — typing reverts (insert session undo). Press `u` again — block deletion reverts (now empty block). Mode stays Normal.

### Manual QA — item 3 (cm6 history coherence)

1. Open a note. Focus a block with multi-character text, e.g. `"hello world"`.
2. Place cursor at end of `"hello "`. Type `cool ` — block now reads `"hello cool world"`.
3. Press Esc.
4. Press `u` (vim normal-mode undo). The insert session reverts: block reads `"hello world"` again.
5. **Inside the same block**, press `i` to enter Insert. Press `Cmd+Z` (cm6's local undo).
6. **Expected**: nothing happens (cm6 history empty for this block, since the typing was wiped by the externalSync replace and that transaction is now excluded from history).
7. Without leaving Insert, type `x`. Press `Cmd+Z`. Expected: the `x` is removed (cm6 history works for new typing within a fresh block state).

### Manual QA — item 2 (cancel-and-flush on undo)

This race is hard to reproduce reliably, but the structural verification is:

1. Open Network tab in DevTools, filter for `notes/`.
2. Open a note. Focus a block. Type `aaa` quickly.
3. Within 500ms (before the debounce fires), press Esc then `u`.
4. **Expected network sequence**: a PUT for the restored body (no PUT for `aaa`), or a PUT for `aaa` immediately marked **(canceled)** in DevTools, followed by a PUT for the restored body.
5. Wait for the WS echo. Block content stays as the restored body — no flicker, no revert.
6. Stress test: type a long string (`abcdefghij`), Esc, `u` repeatedly within the same 500ms window. Network should show at most one canceled PUT and one successful PUT per `u`.

### Regression QA

1. **Typing without undo**: type continuously into a block; only ONE PUT per 500ms-quiet window. The new debounce path must not over-fire.
2. **Save-state UI**: while the in-flight PUT is running, the StatusBar shows "Saving"; after success it shows "Saved". An aborted PUT must NOT show "Save failed".
3. **WS echo from an external client** (open the same note in a second browser tab; type in tab A; tab B should reparse): `lastSentBody` mismatch + `focusedIndex === null` in the inactive tab → reparse fires. Tab B updates correctly.
4. **Page navigation mid-save**: navigate away from the note while a save is pending. The pending timer is cleared on component unmount (the existing `saveTimer` clear pattern survives via Svelte's component teardown — verify the new `inFlightController` is also aborted in an `onDestroy` block; if not, add one):
   ```ts
   import { onDestroy } from "svelte";
   onDestroy(() => {
     if (saveTimer) clearTimeout(saveTimer);
     if (inFlightController) inFlightController.abort();
   });
   ```
   Add this alongside the cancel-and-flush plumbing.
5. **Outliner undo of structural ops** (dd, indent, fold, status cycle, tag toggle): all must still behave as before — single `u` reverts, single `Ctrl+R` redoes, mode stays Normal except where Step 1's `restoredFocus` should suppress an Insert that would have fired.
6. **Insert-session unified undo** (Phase 3M.1's headline behavior): `o<text><Esc>u` reverts typing first, then `u` again reverts the block creation. Must still work after these changes.

## Handoff

**Recommended tier: Sonnet (medium reasoning).** All three sub-fixes are decision-bounded by the spec: the primitives (`Transaction.addToHistory`, AbortController, a boolean state flag) are named explicitly. The trickiest call is the `inFlightController` lifecycle in `flushSave`/`cancelAndFlush`/`onDestroy`; the spec walks through every code path.

**Files likely touched:**
- `web/src/lib/components/BlockOutliner.svelte` — new `restoredFocus` flag, `saveBlocksImmediate` helper, `onCancelAndFlush` prop, `applySnapshot` rewires.
- `web/src/lib/components/BlockEditor.svelte` — import `Transaction`, add `Transaction.addToHistory.of(false)` to externalSync dispatch.
- `web/src/lib/api-client.ts` — `put<T>` and `updateNote` accept optional `AbortSignal`.
- `web/src/routes/p/[id]/+page.svelte` — `handleContentChange` rewrite, new `flushSave` + `cancelAndFlush`, `onDestroy` hook.
- `web/src/routes/timeline/+page.svelte` — optional parity (Step 3d). Mark with `TODO(3M.2)` if skipped.

**Constraints for the implementer:**
- Do NOT change the snapshot data model (`OutlinerSnapshot`). The fixes are at the consumer, not the producer.
- Do NOT remove the existing `focusedIndex !== null` guard in the external-body `$effect` (line 322). It's belt-and-braces for the WS-echo race even with cancel-and-flush.
- Do NOT introduce a new annotation; reuse `externalSync` plus `Transaction.addToHistory`.
- Preserve the fall-through pattern: outliner-undo stack empty → cm-editor's `undo` runs (BlockEditor.svelte:185-187). Step 2 doesn't change that fall-through; it changes what cm-editor's history contains.
- After implementing, run the build + lint commands. Then prepare a commit titled `feat: undo coherence — cancel saves on undo, seal cm6 history, no remount-into-Insert (Phase 3M.2)` (only if Taylor confirms; per repo CLAUDE.md, do not commit without instruction).
