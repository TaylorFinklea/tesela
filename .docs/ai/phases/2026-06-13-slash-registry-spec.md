# Spec — Slash-as-Registry Dispatcher (L2)

**North-star #1:** `:` colon commands, ⌘K palette, Space-leader, AND `/` slash all dispatch through ONE `commandRegistry`. Today slash verbs live in a *separate* hardcoded `applySlash()` switch in `BlockEditor.svelte` — this spec defines the contract that folds them into the registry.

Author: Opus (Lead), 2026-06-13. Informed by a 3-explorer / 3-proposal / synthesis design pass. Every line ref is `~approximate` (verify on read; the codebase drifts).

---

## Decision (locked)

**Verb-only `SlashContext`, additive `run` widening, surface-gated, migrated gut-don't-delete with a grep completion gate.**

Three live-code facts force this shape:

1. `run: (arg?: string) => void | Promise<void>` (`command-registry.svelte.ts:36`) — commands receive NO context today; every `run` body pulls from global stores. So widening to `run(arg?, ctx?)` is **purely additive**: every existing body compiles unchanged.
2. The `localApplyInProgress` guard is a documented footgun — a programmatic `view.dispatch` that forgets it double-fires `onChange` (the B6 bug; `block-editor-local-apply-guard.test.mjs` exists to enforce it). **Exposing the raw CM6 `view` to command authors would re-open that whole bug class.** A verb-only surface eliminates it by construction.
3. `available()` (`registry.svelte.ts:~66`) already filters by a `when()` predicate — a `surface:'editor'` gate folds in one line ahead of it, so editor-only verbs are filtered out of `:`/⌘K with zero new machinery.

Rejected: exposing `EditorView`/`spliceActiveBlock` to commands (CAPABLE proposal) — reintroduces the guard footgun and the splice offset-drift bugs. No slash verb splices today; none needs to.

---

## The contract — `SlashContext`

`web/src/lib/editor/slash-context.ts` (NEW, type-only). The editor-mutation **capability handle** a registry command receives ONLY when dispatched from inside a focused block editor. Produced by `BlockEditor.svelte` (sole owner of `view`, `slashStartPos`, the `on*` prop sinks). Exposes **verbs only** — no raw `EditorView`, no `spliceActiveBlock`, no `localApplyInProgress`. Every mutator internally wraps `dispatchWithLocalApplyGuard`, so a command body *cannot forget the guard*.

```ts
import type { PropertyDefinition } from "$lib/types";

export type SlashContext = {
  /** Focused block identity. `bid` present ⇒ may be Loro-bound; the parent's structured funnel resolves the address. */
  block: { id: string; bid: string | null; properties: Record<string, string> };
  /** Whole-block text BEFORE the `/` trigger: doc.slice(0, slashStartPos). Frozen at dispatch time. */
  before: string;
  /** Block text from the caret onward: doc.slice(cursorPos). Frozen at dispatch time. */
  after: string;
  /** Property defs in scope — drives `/p` leaves and status-verb hoisting. */
  propertyDefs: PropertyDefinition[];
  /** Status enum for this block (statusChoices ?? ["todo","doing","done"]). */
  statusChoices: string[];
  /** Tag name → its default property-key names, for tag-add auto-fill (PROP6). */
  autoFillNames: (tagName: string) => string[];

  /** Replace the trigger region (whole-doc replace) under the guard, then onChange. `caretFromEnd` chars before doc end; omit ⇒ caret at end (never collapses to 0). */
  replaceTrigger: (insert: string, caretFromEnd?: number) => void;
  /** Emit ONE structured container op (never a `key:: value` line); empty value clears. Wraps onSetProperty → setBlockPropertyStructured. Re-resolves address LIVE at call time. */
  setProperty: (key: string, value: string) => void;
  /** Add a tag AND fire onTagAdded so the parent emits tag property-default ops. Text-only no-op if already present. */
  addTag: (tagName: string) => void;
  /** Hand a template note-id to the parent (onInsertTemplate) to expand into child blocks. */
  insertTemplate: (noteId: string) => void;

  /** Strip the trigger, then open the date picker; on pick sets `propertyKey` (default prefs.bareDateField) to the ISO via setProperty. */
  openDatePicker: (propertyKey?: string) => void;
  /** Strip the trigger, then open the tag-manager autocomplete popover (multi-toggle; commit re-enters addTag). */
  openTagPicker: () => void;
  /** Strip the trigger, then open the template-pick popover (commit re-enters insertTemplate). */
  openTemplatePicker: () => void;
  /** Open a typed-property value picker/input (select/checkbox/text) for `def`; commit re-enters setProperty. */
  openPropertyValue: (def: PropertyDefinition) => void;

  /** Selection-only caret move within the focused block — never persisted, never splices. */
  moveCursor: (anchor: number, head?: number) => void;
  /** Shared tail: close slash menu, reset slashStartPos=-1, fire onSlashCommand(verb), refocus view. */
  finish: (verb: string) => void;
};
```

### Command-type change (additive)

`command-registry.svelte.ts` — both new `Command` fields optional, `CommandContext.editor` optional, `run` gains a trailing optional `ctx`. **Apply the SAME additions to `V4Command` in `v4/commands.ts:~77` — it's a structural dup cast at `commands.ts:~721` (`cmd as RegistryCommand`); skewing one breaks the cast or silently drops fields.**

```ts
export type CommandContext = {
  // ...existing route/bufferKind/vimMode/focusedBlock/splitOpen...
  editor?: SlashContext;                          // NEW — present ONLY inside a focused block editor
};
export type Command = {
  // ...existing...
  category: '...' | 'editor';                     // 'editor' NEW
  surface?: 'global' | 'editor';                  // NEW — 'editor' ⇒ requires ctx.editor (filtered from :/⌘K). Default 'global'.
  slashKey?: string;                              // NEW — slash chord key, e.g. 'h' for /heading
  run: (arg?: string, ctx?: CommandContext) => void | Promise<void>;   // WIDENED (arg first ⇒ old bodies compile)
};
```

`available()` gains one line before the `when()` check:
```ts
if (cmd.surface === 'editor' && !ctx.editor) return false;   // NEW
```

---

## Dispatch flow

**Slash (`/heading`):** `inputHandler` (`BlockEditor.svelte:~1639`) records `slashStartPos` → `getSlashTree()` (`~1017`) sources leaves from `commandRegistry.available({...baseCtx, editor: buildSlashContext()}).filter(c => c.slashKey)`, merged AHEAD of surviving hardcoded builtins, deduped by `slashKey` → leaf action `() => cmd.run(undefined, {...baseCtx, editor: slashCtx})`. The `heading` body calls `ctx.editor.replaceTrigger("# " + ed.before.trim() + ed.after)` then `ctx.editor.finish("heading")` — **byte-identical** to today's `case "heading"` + shared tail (`~1121, ~1254`).

**Same entry from `:` / ⌘K (no view):** `ColonCommandLine.runVerb` (`:136`) and `GrCommandPalette.runCommand` (`:~181`) change `cmd.run(arg)` → `cmd.run(arg, ctx)` where `ctx` has no `.editor`. A `surface:'editor'` verb was already dropped by `available()` (graceful filter, not error). **Leader:** thread the `ctx` that `getLeaderTree(ctx)` already receives (`leader-tree.svelte.ts:~106`) down through `buildChordTree(commands, depth, ctx)` and emit `leaf.run(undefined, ctx)` (`~70, ~78`) — currently `buildChordTree(commands, 0)` drops it (`~110`).

---

## Loro / cursor / guard handling

- **Bound vs plain: the command never branches on binding** — preserved exactly, because the contract methods ARE today's code relocated. All text edits route through `replaceTrigger` → guarded whole-doc replace + `onChange`; the guard makes the updateListener early-return (`~1923`), so the bound-vs-plain split stays downstream in the parent's `onChange`/`onLoroText` reconciliation. Properties route through `setProperty` → `onSetProperty` → `setBlockPropertyStructured` (`BlockOutliner.svelte:~1243`), which only changes *address form* by binding (`${note_id}:${bid}` vs line-id) — never the path, never a `key:: value` line (the reverted dual-write bug).
- **Guard internalized** in every dispatching method. A command body can't forget it because it never touches `view.dispatch`. The deliberately *unguarded* autocomplete commits (real edits that SHOULD flow through the listener) stay inside the component behind the `open*Picker` methods — the intentional asymmetry is preserved, not exposed.
- **Cursor:** `replaceTrigger` always passes `selection.anchor`, so a verb can never trip the "caret collapses to 0 on full-doc replace" bug. `caretFromEnd` is the typed knob (heading omits ⇒ caret at end; link passes 2 ⇒ inside `[[]]`).
- **⚠ Picker re-entrancy:** date/tag/template *open* via the command but *commit asynchronously* inside BlockEditor AFTER `slashStartPos` reset to -1 and `view.state` advanced. So `before`/`after` are **dispatch-time frozen reads only**; commit-path mutators (`setProperty`/`addTag`) MUST re-derive their address from `view`+`bid` live at call time. Reuse the EXISTING `DatePicker.onPick` (`~2160`) / `applyAutocomplete` (`~719`) commit paths untouched — migration changes *who opens the picker*, not *how it commits*.

---

## Implementation sub-items (B-impl decomposition)

Added to the backlog. Ordered by dependency; each is senior-tier.

- **B-impl-1 · widen types + thread ctx** (senior·M) — add `editor?`/`surface`/`slashKey` to `Command`+`CommandContext` and the `V4Command` dup; widen `run` to `(arg?, ctx?)`; fold the `surface==='editor' && !ctx.editor` filter into `available()`; thread `ctx` into `ColonCommandLine.svelte:136`, `GrCommandPalette.svelte:~181`, and `leader-tree.svelte.ts` (`buildChordTree(...ctx)` at `~52,70,78,110`). Add `slash-context.ts` (type only). All params optional ⇒ no existing `run` body edited. Verify: `pnpm --dir web check && node --test web/tests/unit/command-registry.test.mjs` (+ a new test: `available({})` excludes a `surface:'editor'` cmd, `available({editor:{}})` includes it).
- **B-impl-2 · `buildSlashContext()` producer** (senior·L) — author the `SlashContext` type + a closure-local `buildSlashContext()` in BlockEditor whose methods wrap existing inline code (`replaceTrigger`←shared tail `~1254`; `setProperty`←property-continuation body; `addTag`←`toggleBlockTag`+`onTagAdded`; `openDatePicker`←`openDatePickerForProperty` `~885`; `open*Picker`←the picker setTimeout blocks; `finish`←shared-tail menu-close). Do NOT expose `view`/`spliceActiveBlock`. Commit-path mutators re-read `view.state`. Verify: `pnpm --dir web check && node --test web/tests/unit/block-editor-local-apply-guard.test.mjs` (+ a temporary `editor.heading` driven through the context yields byte-identical doc + caret to `applySlash('heading')`).
- **B-impl-3 · port slash tree + 2 pilot verbs** (senior·M) — rewrite `getSlashTree()` (`~1017`) to merge registry leaves ahead of legacy builtins, deduped by `slashKey`. Register `editor.heading` + `editor.date` (`web/src/lib/editor/commands/*.ts`); gut their `applySlash` case bodies to an explicit `return;` (visibly dead, reversible). Other 11 verbs stay on the legacy switch. Verify: `pnpm --dir web check && node --test .../block-editor-local-apply-guard.test.mjs` + a Chrome DevTools MCP self-run pass (`/heading` prefixes `# `; `/date` opens picker + writes a STRUCTURED op with no `::` line; `/date` absent from `:`/⌘K).
- **B-impl-4 · migrate remaining verbs + delete switch under grep gate** (senior·L) — port task/status/link/query/collection (text+structured), then tag/template/property/`p` (pickers), each gutting its case to `return;`. The `property` raw-`key:: value` writer (`~1124`) migrates to `setProperty` (retires the last prose-property writer). When the final case is gutted, delete the `applySlash` switch + legacy slash-tree builtins. Verify: `pnpm --dir web check && ! grep -nE 'case "(task|tag|heading|date|query|widget|collection|template|link|property)"' web/src/lib/components/BlockEditor.svelte && node --test .../block-editor-local-apply-guard.test.mjs`.

---

## Risks

1. **Picker-commit re-entrancy / offset drift** — see ⚠ above. Mitigation: `before`/`after` frozen at dispatch; commit mutators re-derive address live; reuse existing picker commit paths untouched; B-impl-2/3 verify is a real DevTools multi-toggle pass, not just a build.
2. **Two-system limbo calcifying** — a migrated verb whose case is NOT gutted gets masked by the dedupe; a registry regression hides behind the still-working legacy case. Mitigation: gut-don't-delete (visibly-dead `return;`) + B-impl-4's grep gate IS the completion criterion.
3. **V4Command/Command drift** — the dup types cast at `commands.ts:~721`; add all fields to BOTH or the cast breaks / drops fields silently.
4. **Picker `$state` abstraction leak** — `open*Picker` must encapsulate strip-trigger + `coordsAtPos` placement + EVERY picker `$state` flag. Before migrating each picker verb, enumerate the `$state` its setTimeout block touches and confirm the `open*` method sets all of them.
5. **Degrade-branch scope creep** — `surface:'global'` verbs (e.g. `:heading`) need a non-editor branch mutating `ctx.focusedBlock`. Real work. Mitigation: default verbs to `surface:'editor'` (filtered off-editor, no branch needed); treat `:`/⌘K-targets-focused-block as a tracked follow-up, NOT a migration blocker.

---

## Open questions for Taylor (keyboard-first product shape — NON-blocking; my default in **bold**)

1. **Editor verb with no block focused** (`:heading` / ⌘K→Heading): hide / grey-disable / target last-focused block? → **Default `surface:'editor'` (hide).** Cheapest, no degrade branch. Promote specific verbs to `global` later if you want `:`-targets-focused-block.
2. **Unify `slashKey` + leader `chord` into one binding per verb** (`/h`, `Space h`, `:heading` all → same command)? → **Keep separate for now**; revisit under L3 (user-rebindable keys). This is the eventual north-star direction.
3. **Deferred-picker verbs (date/tag/template): keep the visual popover, or add a fully-typed path** (`:date 2026-07-01`)? → **Keep popover for B-impl; typed path = follow-up.**
4. **Is `widget` (HTTP note-create + nav) a slash verb at all,** or a top-level `:`/⌘K create command? I.e. is the slash menu strictly block-mutation or a general editor command surface? → **Keep as-is for B-impl (no behavior change);** taxonomy call deferred.

None of these block B-impl-1/2/3. Q1's default (`surface:'editor'`) is baked into the sub-items so B-impl-4 isn't gated either.
