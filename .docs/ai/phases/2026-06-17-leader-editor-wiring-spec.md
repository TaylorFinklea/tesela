# Leader → Editor wiring (Space → i / p run on the focused block)

**Decision:** Taylor chose "Wire leader→editor" (harness-deck `leader-ip-bucket`,
2026-06-17). Space → `i` (insert) and `p` (properties) buckets should run the
editor verbs on the **focused block** — completing the keyboard-first vision so
the leader is a peer of slash for block mutation.

**Why this is a sub-project, not a one-liner:** the editor verbs are coupled to
the **slash-trigger** flow. `editor.heading.run` calls
`ed.replaceTrigger(headingText(ed.before, ed.after), …)` — a whole-block replace
computed from `SlashContext.before` / `.after`, which are **frozen at the `/`
dispatch position** (`slash-context.ts`). The leader has no `/` trigger, so it
needs a *fresh trigger-less context* built by the focused editor, plus a presence
signal so the leader tree includes the editor commands, plus routing from the
leader action to the focused `BlockEditor`. The registry already half-supports
this (`CommandContext.editor?: SlashContext`; `available()` keeps `surface:'editor'`
commands when `ctx.editor` is set).

## Current state (verified on `main` @ d8b26944)

- `surfacesFor` (command-registry.svelte.ts:113-118): `surface:'editor'` → `{slash}`
  only — editor verbs never reach the leader. `editor.heading`/`editor.task` are
  `surface:'global'` → reach the leader but **no-op** (run bails `if (!ed) return`).
- `editor.property` is the ONLY `p`-leading chord; dropped from leader → the `p`
  bucket never renders (`buildChordTree` only emits a bucket with children).
- `GraphiteShell` `commandCtx` (lines 59-68) has no `editor`/presence field.
- Slash path builds context via `buildSlashCommandContext(editor)` (BlockEditor:1267),
  where `editor` is the rich `SlashContext` object (BlockEditor ~1190-1265).

## Design

### 1. Registry — make editor commands leader-visible when an editor is focused
- `surfacesFor`: `surface:'editor'` branch also `out.add('leader')` when
  `cmd.chord?.length`. (Keeps slash; adds leader.)
- `CommandContext`: add `editorFocused?: boolean`.
- `available()`: change the gate to
  `if (cmd.surface === 'editor' && !ctx.editor && !ctx.editorFocused) return false;`
  — so the leader tree (which sets `editorFocused`, not a full `editor`) includes
  editor commands, while the slash path (full `ctx.editor`) is unchanged.

### 2. Focused-editor store — `web/src/lib/stores/focused-editor.svelte.ts` (new)
- `let present = $state(false)`; `isEditorFocused()`.
- BlockEditor sets `present = true` on cm focus, `false` on blur/unmount.
- (No SlashContext in the store — execution builds a FRESH context at run time;
  a stored context would have stale `before`/`after`.)

### 3. GraphiteShell — feed presence into the leader ctx
- `commandCtx` gains `editorFocused: isEditorFocused()` (reactive). The leader
  overlay already gets `ctx={commandCtx}`, so the `i`/`p` buckets populate when a
  block is focused and vanish when none is (correct — they need a target block).

### 4. Leader action routing — `leader-tree.svelte.ts` `buildChordTree`
- A leaf whose `category === 'editor'` (catches heading/task AND the
  surface:'editor' verbs) must NOT call `leaf.run(undefined, ctx)` directly (ctx
  has no live editor). Instead its action dispatches
  `document.dispatchEvent(new CustomEvent('tesela:run-editor-command', { detail: { id: leaf.id } }))`.
  Non-editor leaves keep `leaf.run(undefined, ctx)`.

### 5. BlockEditor — handle the event with a fresh trigger-less context
- Add a `buildLeaderEditorContext()`: like the slash `editor` object but with the
  trigger region collapsed to the caret — `before = doc.slice(0, caret)`,
  `after = doc.slice(caret)`, and the internal `slashStartPos = caret` so
  `replaceTrigger` inserts at the caret rather than stripping a `/…` run. Reuse the
  existing `replaceTrigger` / `setProperty` / `addTag` / `openDatePicker` /
  `openTagPicker` / `openTemplatePicker` / `openPropertyValue` closures (they
  re-resolve the block address live and act on `view`).
- Listen for `tesela:run-editor-command`; handle ONLY when this editor is focused
  (`view?.hasFocus`), so multi-pane fires once on the right block. Look up
  `commandRegistry.get(id)` and `run(undefined, buildSlashCommandContext(leaderCtx))`.

## Per-verb verification (browser — required, one block per case)
Each must produce the right block mutation from `Space → i/p → <key>` on a focused
block. Heading/task/link are whole-block-text rewrites (low risk). Date/tag/
template/property OPEN a picker then commit via continuation writers that use the
caret position — verify the picker commits onto the focused block (not at a stale
`/` position). Query/collection insert their `query::`/`collection::` scaffold.

| verb | leader path | expect |
|---|---|---|
| heading | i h | `# ` prepended to block |
| task | i t | `Task` tag added |
| link | i l | `[[ ]]` inserted at caret |
| tag | i g | tag picker opens; commit adds tag |
| date | i d | date picker opens; commit sets bare-date prop |
| template | i m | template picker opens; commit expands |
| query | i q | `query:: ` scaffold |
| collection | i c | `collection:: ` scaffold |
| property (each def) | p `<key>` `<value>` | sets the property on the block |

## Acceptance
- `Space → i → h` prepends `# ` to the focused block; `Space → p → <prop> → <val>`
  sets the property. The `i`/`p` buckets render only when a block is focused.
- No double-run across panes; no no-op leaves.
- `npm run check` 0 errors; `npm run test:unit` green (+ a surfacesFor unit test
  that editor+chord commands include `leader`, and an `available()` test that
  `editorFocused` admits editor commands).
- Slash path unchanged (regression-checked: `/heading` still works).

## Risks / landmines
- The trigger-less context must NOT strip real text. Build `slashStartPos = caret`
  so `replaceTrigger` replaces an empty region.
- `view?.hasFocus` may be false if the leader overlay stole focus — confirm the
  cm editor keeps DOM focus while the leader is open (it does for the slash menu;
  the leader is a capture-phase document listener that doesn't take focus).
- Don't regress the slash path: `editorFocused` is additive; slash still passes a
  full `ctx.editor`.
