# Phase 3M.3 — Cmd+Z Outside Vim

## Product Overview

For users with Vim mode disabled (a settings toggle that already exists), the outliner-level undo / redo stack is currently unreachable — there is no `u`/`Ctrl+R` mapping outside Insert mode, and Cmd+Z inside the cm-editor only undoes intra-block typing. This spec wires Cmd+Z (and Cmd+Shift+Z) at the document level so it routes to the same `OutlinerHistory` undo whenever the user is not actively editing inside a cm-editor.

Audience: non-vim users. Goal: parity with the vim-on path — Cmd+Z reverts the last structural mutation across the whole outliner, falling through to cm-editor's local history when the outliner stack is empty (matching the vim `u` behavior in `BlockEditor.svelte:184-194`).

## Current State

Phase 3M (commit `29f77c9`) shipped the outliner-history primitive. Phase 3M.1 (`8ab8492`) added unified undo via vim's `u`. The mapping path:

- `web/src/lib/stores/outliner-history.svelte.ts` — `OutlinerHistory` class with `popUndo` / `popRedo`.
- `web/src/lib/components/BlockOutliner.svelte:166-192` — instance, `undoOutliner`, `redoOutliner` exposed to BlockEditor as `onUndoOutliner` / `onRedoOutliner` props.
- `web/src/lib/components/BlockEditor.svelte:184-194` — vim `u` and `<C-r>` mappings that try outliner first, fall through to cm-editor.
- `web/src/lib/components/BlockEditor.svelte:618-647` — `vimCtx` is set when a block is focused. `vimCtx.undoOutliner` / `vimCtx.redoOutliner` are populated.
- `web/src/lib/stores/pane-state.svelte.ts:135-143` — `isVimEnabled()` reads `localStorage["tesela:vimEnabled"]`, defaults `true`.
- `web/src/routes/+layout.svelte:44-194` — root keydown listeners. Pattern at lines 67-81 (space leader), 87-112 (panel handler), 122-182 (Ctrl+w). Each handler tests `isEditing` via `target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable || target.closest(".cm-editor")` to decide whether to swallow.
- `web/src/routes/settings/+page.svelte:17-28` — `vimEnabled` localStorage toggle.

The cm-editor currently swallows Cmd+Z via `historyKeymap` (`web/src/lib/components/BlockEditor.svelte:846`), so when the cm-content has DOM focus, browsers fire `Cmd+Z` to cm6 and our document listener never sees it (assuming we listen at bubble phase, which is the default). When the cm-content is NOT focused (e.g. user clicked into the page background, hit Esc to exit Insert+leave Normal-cursor on a block, or has no block focused), the document listener can act.

The challenge: vim's `u` mapping fires WHEN the cm-editor has focus AND vim is in Normal mode. That's an in-cm context. For non-vim users, `Cmd+Z` should ALSO work in the in-cm context if the user expects "platform-standard undo." Two approaches:

- **A. Document-level only.** Cmd+Z only works when no cm-editor is focused. This is the simpler path and matches the user's prompt ("when not inside an editor").
- **B. Bind Cmd+Z inside cm6 too**, conditional on `!isVimEnabled()`. More work; requires a Compartment-driven keybinding inside BlockEditor that swaps based on a vim-enabled facet.

Approach A is the right scope for this spec — it's separable, low-risk, and matches the user's explicit framing. If the daily-driver experience demands in-cm Cmd+Z later, file a follow-up. Document this explicitly in the implementer notes so they don't go further than asked.

## Implementation Plan

### Step 1 — Expose outliner undo / redo at the document level

The current architecture keeps `undoOutliner` / `redoOutliner` private to BlockOutliner. To call them from `+layout.svelte`, we have two choices:

- **A1. Use the existing `vimCtx` singleton.** It already holds `undoOutliner` / `redoOutliner` whenever a block is focused. But it's defined inside `BlockEditor.svelte`'s module scope and not exported, AND it goes null on blur. For a document-level Cmd+Z handler, we want a stable handle that survives blur.
- **A2. Define a new module-level singleton in a new store**, e.g. `web/src/lib/stores/outliner-actions.svelte.ts`, that BlockOutliner registers itself with on mount and unregisters on destroy.

Use A2. It's symmetric to existing per-pane state (`pane-state.svelte.ts`) and it keeps the registration explicit.

1. Create `web/src/lib/stores/outliner-actions.svelte.ts`:
   ```ts
   /**
    * Module-level handle to the active outliner's undo / redo. Registered
    * by BlockOutliner on mount, cleared on destroy. Used by the document-
    * level Cmd+Z handler in +layout.svelte to drive outliner-level undo
    * for non-vim users.
    *
    * Only one BlockOutliner is "active" at a time on the page-level layout;
    * the timeline page renders multiple, but the user can only have focus
    * inside one at any given moment. We register on mount so the most-
    * recently-mounted outliner wins; the timeline's outliners all share
    * the same undo plumbing per-instance, so undo applies to whichever
    * one most recently registered.
    */
   let activeUndoOutliner: (() => boolean) | null = null;
   let activeRedoOutliner: (() => boolean) | null = null;

   export function registerOutlinerActions(
     undoFn: () => boolean,
     redoFn: () => boolean,
   ): () => void {
     activeUndoOutliner = undoFn;
     activeRedoOutliner = redoFn;
     return () => {
       if (activeUndoOutliner === undoFn) activeUndoOutliner = null;
       if (activeRedoOutliner === redoFn) activeRedoOutliner = null;
     };
   }

   export function tryUndoOutliner(): boolean {
     return activeUndoOutliner?.() ?? false;
   }

   export function tryRedoOutliner(): boolean {
     return activeRedoOutliner?.() ?? false;
   }
   ```
2. In `web/src/lib/components/BlockOutliner.svelte`, register the actions on mount. Add a new `onMount` block (line 790 has one already; add a sibling) — or fold into a new effect:
   ```ts
   import { registerOutlinerActions } from "$lib/stores/outliner-actions.svelte";

   onMount(() => {
     return registerOutlinerActions(undoOutliner, redoOutliner);
   });
   ```
   The returned cleanup is invoked on component destroy (Svelte 5 `onMount` returns the cleanup signature). Important: capture the function references at registration time — if a future refactor recreates `undoOutliner`/`redoOutliner` per render, this needs adjusting. Right now they're stable function declarations within `<script>`.

### Step 2 — Document-level Cmd+Z / Cmd+Shift+Z in +layout.svelte

In `web/src/routes/+layout.svelte`:

1. Add an import:
   ```ts
   import { tryUndoOutliner, tryRedoOutliner } from "$lib/stores/outliner-actions.svelte";
   ```
2. Inside `onMount` (around line 87 where `panelHandler` is defined), add an `undoHandler`:
   ```ts
   const undoHandler = (e: KeyboardEvent) => {
     // Only run when vim is OFF — vim users have `u` / `Ctrl+R` already.
     if (isVimEnabled()) return;

     // Match Cmd+Z (mac) / Ctrl+Z (other). Cmd+Shift+Z and Ctrl+Y for redo.
     const isMod = e.metaKey || e.ctrlKey;
     if (!isMod) return;

     const isUndo = e.key === "z" && !e.shiftKey;
     const isRedo = (e.key === "z" && e.shiftKey) || e.key === "y";
     if (!isUndo && !isRedo) return;

     // Skip when the user is editing in a cm-editor or native input. cm6
     // has its own Cmd+Z bound in historyKeymap; let it handle in-block
     // typing-undo. Outside cm-editors, we drive outliner undo.
     const target = e.target as HTMLElement;
     const isEditing =
       target.tagName === "INPUT" ||
       target.tagName === "TEXTAREA" ||
       target.isContentEditable ||
       target.closest(".cm-editor");
     if (isEditing) return;

     const handled = isUndo ? tryUndoOutliner() : tryRedoOutliner();
     if (handled) {
       e.preventDefault();
       e.stopPropagation();
     }
     // If not handled (no active outliner / empty stack), let the browser's
     // native undo do whatever it would do — usually nothing, since we're
     // not in an input.
   };
   ```
3. Register the handler at bubble phase (default), and add it to the cleanup:
   ```ts
   document.addEventListener("keydown", undoHandler);
   // ... (alongside existing addEventListener calls)
   ```
   And in the cleanup:
   ```ts
   document.removeEventListener("keydown", undoHandler);
   ```

### Step 3 — Surface the new shortcut in settings

In `web/src/routes/settings/+page.svelte`, the keyboard shortcuts list (line 121-145) already documents vim shortcuts. Add a new entry near the top, conditional on the vim toggle being OFF (or just append a "non-vim users" note). Implementation: leave the list as-is and add ONE new row immediately after the existing `["1", "Toggle sidebar"]` line:

```svelte
["⌘Z / ⌘⇧Z", "Outliner undo / redo (when Vim mode is off)"],
```

This is purely cosmetic; it keeps the docs honest without complicating the list with conditional rendering.

### Step 4 — Build verification

```sh
pnpm --dir web tsc --noEmit
pnpm --dir web lint
```

Both must pass.

## Interfaces and Data Flow

- New module: `web/src/lib/stores/outliner-actions.svelte.ts`. Public exports: `registerOutlinerActions(undoFn, redoFn): cleanup`, `tryUndoOutliner(): boolean`, `tryRedoOutliner(): boolean`.
- New BlockOutliner side effect: `onMount` registers undo/redo and returns the cleanup.
- New keydown listener at root `+layout.svelte` for Cmd+Z / Cmd+Shift+Z / Ctrl+Y, gated on `!isVimEnabled()` and `!isEditing`.

No API changes. No new props on existing components. No server changes.

## Edge Cases and Failure Modes

- **Vim mode ON**: handler is a no-op (early return on `isVimEnabled()`). Vim's `u` keeps working through the existing BlockEditor mappings. Verified by `if (isVimEnabled()) return;`.
- **Cm-editor focused, vim off**: cm6's `historyKeymap` claims `Mod-z` first (it's a cm-keymap, runs at the `keydown` event before our document listener with bubble-phase registration only if the user typed inside cm; since cm uses `EditorView.domEventHandlers` for low-level events but its keymap is a `keymap.of([...])` extension reacting to keys delivered via cm's own handler chain — these run before bubble-phase document listeners ON the cm-content node). The `isEditing` guard backs this up: even if cm somehow doesn't claim it, our handler bails on `target.closest(".cm-editor")`. Net behavior: in-block typing-undo via cm6 only.
- **Cm-editor focused, vim on**: still gated by `isVimEnabled() === true` early return. Vim handles undo via its mapping. (Today, vim's `u` mapping is only registered when a block is focused; the keymap re-registration on every block mount keeps vimCtx fresh — see BlockEditor.svelte:43-48 comment.)
- **Multiple BlockOutliner instances on the page** (timeline page): the registration is last-write-wins. When the user clicks into a different daily note's outliner, that outliner's most-recent mount registered; the previously-registered cleanup ran on its replacement. To improve correctness here, `registerOutlinerActions` returns a stable cleanup that only nulls the active fn IF it still matches — so unmount-A while B is active does NOT clear B's registration. The implementation above does this correctly via the identity check (`if (activeUndoOutliner === undoFn) activeUndoOutliner = null`). For multiple simultaneously-rendered outliners on timeline, focus-driven registration is the correct semantics: the user's mental model is "Cmd+Z affects what I'm looking at," and on timeline that's whichever daily note they last interacted with. Caveat: if the user clicks outside ALL outliners (sidebar, header), `tryUndoOutliner()` still uses the last-registered handle. Document this behavior in a code comment.

  **Stronger alternative for timeline**: register on focus, deregister on blur. Implementation cost: BlockOutliner gains a focus-tracking effect that toggles registration. Skip for now — last-mount semantics is acceptable for a behavior that's already an opt-in-feature for non-vim users on a feature-edge page.

- **No outliner mounted** (`/settings`, sidebar-only views): `activeUndoOutliner === null`, `tryUndoOutliner()` returns `false`, handler does NOT preventDefault, browser's native Cmd+Z runs (which is a no-op in most non-input contexts). Correct.
- **Empty outliner stack**: `undoOutliner()` returns `false`, the document handler returns false-handled, `e.preventDefault()` is NOT called. Browser-native Cmd+Z runs — usually nothing. Note: this DIFFERS from the vim path where `u` falls through to cm-editor's own history; here the user is NOT in cm-editor (else `isEditing` would have bailed). So no fall-through is needed.
- **Mac vs. non-mac modifier**: handler accepts both `metaKey` and `ctrlKey`. On macOS this means Ctrl+Z also works (which is non-standard Mac behavior). Acceptable and matches the existing leader-menu and command-palette patterns in this codebase.
- **Repeat fire**: holding Cmd+Z fires keydown repeatedly. Each repeat pops one snapshot. Matches platform expectation.
- **Native input (form fields)**: the `isEditing` guard skips them; native input undo works.
- **Order of handler registration**: the `undoHandler` is registered alongside the existing `spaceHandler`/`panelHandler`/`ctrlWHandler`. Bubble-phase, so it fires AFTER capture-phase listeners. The Ctrl+w handler is capture-phase but only triggers on the `w` key, so no conflict with `z`.
- **Settings change mid-session**: `isVimEnabled()` is read on every keydown — toggling the setting in `/settings` takes effect on the next keystroke without needing a reload. Verify `localStorage` reads aren't cached.

## Test Plan

### Build verification

```sh
pnpm --dir web tsc --noEmit
pnpm --dir web lint
```

### Manual QA — vim OFF path (the new feature)

1. Open `/settings`. Toggle Vim mode OFF.
2. Open a note in `/p/<id>` with multiple blocks.
3. Click into the page background (not into a block). The block-row hover state should be visible but the cm-editor should NOT have focus (no caret blink).
4. Press a structural action via clicking the bullet/status indicator chevron etc. — e.g. click the status chevron on a block to cycle status. Confirm status changed.
5. Press Cmd+Z. Status reverts to the previous value.
6. Press Cmd+Shift+Z. Status returns to the cycled value.
7. Click into a block. Press Cmd+Z. Expected: cm-editor's local history undoes intra-block typing (or no-op if no recent typing). Outliner stack must NOT pop.
8. Click out of the block (Esc, or click the page bg). Press Cmd+Z. Outliner stack pops — revert back to before the click-in.

### Manual QA — vim ON path (must not regress)

1. Toggle Vim mode ON in `/settings`.
2. Open a note. Focus a block (Insert default if empty, else Normal).
3. Press Esc to ensure Normal mode. Press `u`. Outliner / cm-editor undo runs as before.
4. With cursor in cm-editor, press Cmd+Z. **Expected**: cm-editor's local typing-undo runs (cm6 history). Outliner stack does NOT pop. Confirm the document handler did NOT fire — easiest way: type `aaa` in a block, Esc, `u` (insert session reverts via outliner), then `Cmd+Z` inside the same block — should be a no-op (cm6 history is empty after insert-session unification, see Phase 3M.2 step 2).

### Manual QA — settings doc

1. Open `/settings`. Confirm the new entry "⌘Z / ⌘⇧Z — Outliner undo / redo (when Vim mode is off)" appears in the keyboard shortcuts list.

### Regression QA

- **Leader menu (Space key)**: with vim OFF, press Space outside a block. Leader menu opens. Cmd+Z while leader menu open: handler runs but `tryUndoOutliner()` either pops or no-ops; the leader menu does NOT close on Cmd+Z (no menu-aware handling needed). If this is jarring, consider gating on `!showLeaderMenu` in the layout's undo handler — but the simpler default ships first.
- **Sidebar j/k navigation**: outside any cm-editor, vim off. Cmd+Z does NOT eat j/k. The handler is keyed on `z` only.
- **Form inputs in /settings**: fontSize slider, server URL input. Cmd+Z inside these inputs uses native browser undo. Verified by `isEditing` check.

## Handoff

**Recommended tier: Sonnet (medium reasoning).** The work is small (one new module file, two edits to existing files, one settings doc tweak). The decision points are:

1. **Document-level vs. in-cm Cmd+Z**: spec says document-level only. Do NOT add a Compartment to BlockEditor.
2. **Last-write-wins vs. focus-tracked registration**: spec says last-write-wins. Do NOT add focus-tracking to BlockOutliner; document the limitation in a code comment near `registerOutlinerActions`.

**Files likely touched:**
- `web/src/lib/stores/outliner-actions.svelte.ts` — NEW.
- `web/src/lib/components/BlockOutliner.svelte` — import + onMount registration.
- `web/src/routes/+layout.svelte` — import + new `undoHandler` + add/remove listener.
- `web/src/routes/settings/+page.svelte` — one new row in the shortcuts list.

**Constraints for the implementer:**
- Do NOT change BlockEditor's existing vim mappings.
- Do NOT add a localStorage write or a new pref. Reuse `isVimEnabled()`.
- Do NOT register at capture phase. Bubble is correct (lets cm6 claim Cmd+Z first when its content is focused).
- Run build + lint before declaring complete. Then prepare a commit titled `feat: Cmd+Z outliner undo for non-vim users (Phase 3M.3)` (only if Taylor confirms).
