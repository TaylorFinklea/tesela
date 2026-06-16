# Command-model redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Redesign Tesela's command-interaction model so the four surfaces have deliberate jobs — type-to-filter slash, a which-key bucketed leader (every command homed) reachable in insert mode, universal ⌘K, and a narrowed `:`.

**Architecture:** All four surfaces (`/`, `:`, leader, ⌘K) are filters over one `commandRegistry` (`web/src/lib/command-registry.svelte.ts`). Phase A adds a per-surface `surfaces` ownership field + `availableOn(surface, ctx)` (back-compat: no behavior change until later phases set explicit `surfaces`). Phase B rebuilds the leader as named which-key buckets in a wide multi-column grid, gives every command a bucket home, and adds a `Ctrl+,` insert-mode opener. Phase C makes slash type-to-filter (Logseq) with `Ctrl+letter` accelerators, pared to 8 insertion verbs + a context-aware `Properties` entry. Phase D narrows `:` to exact verbs and folds the hand-duplicated colon builtins into the registry.

**Tech stack:** Svelte 5 (runes), TypeScript, the unified `commandRegistry`, `node --test` for pure-logic TDD (tests import the `.ts`/`.svelte.ts` directly), Chrome DevTools MCP for UI/browser-QA steps.

**Design spec:** `.docs/ai/phases/2026-06-16-command-model-redesign-spec.md` (APPROVED 2026-06-16).

**Phase order:** A is the foundation (do first). B, C, D each depend on A's `surfaces`/`availableOn` and are otherwise independent — execute in any order after A. Each phase is independently committable + testable.

---

## Phase A — Registry per-surface ownership (foundation)

### Files

All paths absolute.

NEW
- (none) — Phase A adds types + functions to the existing registry module and extends the existing test file. No new files.

MODIFIED
1. /Users/tfinklea/git/tesela/web/src/lib/command-registry.svelte.ts
   - Add `export type Surface = 'slash' | 'colon' | 'leader' | 'palette';` near the `CommandContext`/`Command` type block (top, ~line 17-42).
   - Add one optional field to the `Command` type (currently lines 26-42): `surfaces?: ReadonlySet<Surface>;` (place after `surface?: 'global' | 'editor';` at line 35).
   - Add a free function `export function surfacesFor(cmd: Command | RegisteredCommand): ReadonlySet<Surface>` — the back-compat deriver. Returns `cmd.surfaces` verbatim when present; otherwise derives from today's fields (see Tasks for exact rules). Place it as a module-level export AFTER the `commandRegistry` singleton (so it can be a plain function; it needs no class state), e.g. just below line 96 `export const commandRegistry = ...`.
   - Add a method `availableOn(surface: Surface, ctx: CommandContext): RegisteredCommand[]` on `CommandRegistry` (the class, after `available()` at lines 71-82). It calls `this.available(ctx)` then `.filter(cmd => surfacesFor(cmd).has(surface))`. Keep `available()`'s signature/behavior untouched.

2. /Users/tfinklea/git/tesela/web/tests/unit/command-registry.test.mjs (existing, 503 lines)
   - Append new `test(...)` blocks for `surfacesFor` (back-compat derivation cases) and `availableOn` (per-surface filter + delegation to available's when/editor gate). Mirror the existing `_reset()`-then-`register()` pattern. Import `surfacesFor` from `mod` (extend the destructure at lines 23-32).

3. /Users/tfinklea/git/tesela/web/src/lib/v5/leader-tree.svelte.ts:112 — switch base call from `commandRegistry.available(ctx)` to `commandRegistry.availableOn('leader', ctx)`; KEEP the existing `effectiveChord` presence filter (lines 112-117) on top.

4. /Users/tfinklea/git/tesela/web/src/lib/components/BlockEditor.svelte:1427 — `getSlashTree`: change `.available(baseCtx)` to `.availableOn('slash', baseCtx)`. The `.filter(cmd => cmd.slashKey)` on line 1428 stays (no-op redundancy today; harmless and keeps the slashKey contract explicit).

5. /Users/tfinklea/git/tesela/web/src/lib/graphite/shell/GrCommandPalette.svelte:67 — change `commandRegistry.available(ctx)` to `commandRegistry.availableOn('palette', ctx)`.

6. /Users/tfinklea/git/tesela/web/src/lib/components/shell/ColonCommandLine.svelte:51 — change `commandRegistry.available(ctx)` to `commandRegistry.availableOn('colon', ctx)`.

### Tasks

All test commands run from /Users/tfinklea/git/tesela/web . Runner: `node --test 'tests/unit/command-registry.test.mjs'` (the repo's existing pattern; node imports the .svelte.ts directly). One commit per task.

PRECEDENCE: Task 1 (deriver) before Task 2 (availableOn) before Tasks 3-6 (consumers). Consumers can be done in any order after Task 2.

═══════════════════════════════════════════════════════════════════
TASK 1 — `Surface` type + `surfacesFor` back-compat deriver (pure)
═══════════════════════════════════════════════════════════════════
Files: command-registry.svelte.ts, command-registry.test.mjs

1a. WRITE FAILING TEST — append to command-registry.test.mjs. First extend the destructure at the top to pull `surfacesFor`:
    add `surfacesFor,` inside the `const { ... } = mod;` block (lines 23-32).
Then append these tests (REAL code, grounded in the contract):

```js
// ── surfacesFor (back-compat derivation) ──────────────────────────────────

test("surfacesFor returns explicit surfaces verbatim when present", () => {
  const cmd = {
    id: "x", label: "X", glyph: "x", category: "editor",
    surfaces: new Set(["slash", "leader"]),
    slashKey: "h", surface: "global", chord: ["i", "h"], // all ignored
    keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("slash"), true);
  assert.equal(s.has("leader"), true);
  assert.equal(s.has("colon"), false);
  assert.equal(s.has("palette"), false);
});

test("surfacesFor derives slash from slashKey + always palette/colon (back-compat)", () => {
  // A bare editor insertion verb today: slashKey present, no surface flag.
  const cmd = {
    id: "ins", label: "Ins", glyph: "+", category: "editor",
    slashKey: "h", keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("slash"), true);
  assert.equal(s.has("palette"), true);
  assert.equal(s.has("colon"), true);
});

test("surfacesFor: surface:'global' yields all four surfaces", () => {
  // The editor.heading shape: surface:'global' + slashKey — leaks everywhere today.
  const cmd = {
    id: "editor.heading", label: "Heading", glyph: "#", category: "editor",
    surface: "global", slashKey: "h", keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.deepEqual(
    [...["slash", "colon", "leader", "palette"]].filter((k) => s.has(k)).sort(),
    ["colon", "leader", "palette", "slash"],
  );
});

test("surfacesFor: surface:'editor' yields slash only", () => {
  const cmd = {
    id: "ed", label: "Ed", glyph: "e", category: "editor",
    surface: "editor", keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("slash"), true);
  assert.equal(s.has("colon"), false);
  assert.equal(s.has("leader"), false);
  assert.equal(s.has("palette"), false);
});

test("surfacesFor: chord puts command in the leader bucket", () => {
  const cmd = {
    id: "go-daily", label: "Daily", glyph: "d", category: "navigate",
    chord: ["g", "d"], keywords: [], run: () => {},
  };
  assert.equal(surfacesFor(cmd).has("leader"), true);
});

test("surfacesFor: plain command (no flags) is palette + colon", () => {
  const cmd = {
    id: "plain", label: "Plain", glyph: "p", category: "navigate",
    keywords: [], run: () => {},
  };
  const s = surfacesFor(cmd);
  assert.equal(s.has("palette"), true);
  assert.equal(s.has("colon"), true);
  assert.equal(s.has("slash"), false);
  assert.equal(s.has("leader"), false);
});
```

1b. RUN — FAILS (surfacesFor undefined):
    `node --test 'tests/unit/command-registry.test.mjs'`
    Expected: `ReferenceError`/`undefined is not a function` for `surfacesFor`, several `fail`.

1c. MINIMAL IMPL — in command-registry.svelte.ts:
    (i) Add the type after the imports (~line 16):
        `export type Surface = 'slash' | 'colon' | 'leader' | 'palette';`
    (ii) Add the field to the `Command` type, after `surface?: 'global' | 'editor';` (line 35):
        `surfaces?: ReadonlySet<Surface>;`
    (iii) Add the deriver below the `commandRegistry` export (~line 96). The derivation must reproduce TODAY's behavior exactly (verify against available() at lines 71-82 and the four consumers):

```ts
/**
 * Per-surface visibility for a command. When `cmd.surfaces` is set it is
 * authoritative; otherwise derive back-compat defaults from today's fields so
 * Phase A is a no-op until later phases set explicit `surfaces`.
 */
export function surfacesFor(cmd: Command | RegisteredCommand): ReadonlySet<Surface> {
  if (cmd.surfaces) return cmd.surfaces;
  const out = new Set<Surface>();
  if (cmd.surface === 'editor') {
    // editor-only command — slash menu only (today it never reaches the others
    // because available() drops it without ctx.editor, and only slash builds ctx.editor)
    out.add('slash');
    return out;
  }
  // surface 'global' or unset → visible to palette + colon today.
  out.add('palette');
  out.add('colon');
  if (cmd.slashKey) out.add('slash');
  if (cmd.chord && cmd.chord.length > 0) out.add('leader');
  return out;
}
```

1d. RUN — PASSES: `node --test 'tests/unit/command-registry.test.mjs'`
    Expected: all prior 24 + the 6 new pass, `fail 0`.

1e. COMMIT: `feat(web/registry): add Surface type + surfacesFor back-compat deriver (Phase A)`

═══════════════════════════════════════════════════════════════════
TASK 2 — `availableOn(surface, ctx)` per-surface filter (pure)
═══════════════════════════════════════════════════════════════════
Files: command-registry.svelte.ts, command-registry.test.mjs

2a. WRITE FAILING TEST — append to command-registry.test.mjs:

```js
// ── availableOn (per-surface filter over available) ───────────────────────

test("availableOn filters available() by derived surface", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "slashy", label: "Slashy", glyph: "+", category: "editor",
    slashKey: "h", keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "leadery", label: "Leadery", glyph: "g", category: "navigate",
    chord: ["g", "d"], keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "plain", label: "Plain", glyph: "p", category: "navigate",
    keywords: [], run: () => {},
  });

  // slashy has slashKey → slash; also palette+colon. NOT leader.
  assert.deepEqual(
    commandRegistry.availableOn("slash", {}).map((c) => c.id),
    ["slashy"],
  );
  // leadery has chord → leader. plain has none.
  assert.deepEqual(
    commandRegistry.availableOn("leader", {}).map((c) => c.id),
    ["leadery"],
  );
  // palette/colon include every non-editor command.
  assert.deepEqual(
    commandRegistry.availableOn("palette", {}).map((c) => c.id),
    ["slashy", "leadery", "plain"],
  );
  assert.deepEqual(
    commandRegistry.availableOn("colon", {}).map((c) => c.id),
    ["slashy", "leadery", "plain"],
  );
});

test("availableOn respects explicit surfaces (authoritative)", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "scoped", label: "Scoped", glyph: "s", category: "navigate",
    surfaces: new Set(["leader"]),
    slashKey: "x", chord: ["g", "z"], // would-be derivations ignored
    keywords: [], run: () => {},
  });
  assert.deepEqual(commandRegistry.availableOn("leader", {}).map((c) => c.id), ["scoped"]);
  assert.deepEqual(commandRegistry.availableOn("slash", {}).map((c) => c.id), []);
  assert.deepEqual(commandRegistry.availableOn("palette", {}).map((c) => c.id), []);
});

test("availableOn still honors when() and editor gate from available()", () => {
  commandRegistry._reset();
  commandRegistry.register({
    id: "page-only", label: "Page", glyph: "p", category: "navigate",
    chord: ["g", "p"], when: (ctx) => ctx.bufferKind === "page",
    keywords: [], run: () => {},
  });
  commandRegistry.register({
    id: "editor-ins", label: "Ins", glyph: "+", category: "editor",
    surface: "editor", slashKey: "h", keywords: [], run: () => {},
  });

  // when() gate: leader bucket hides page-only off a page.
  assert.deepEqual(commandRegistry.availableOn("leader", {}).map((c) => c.id), []);
  assert.deepEqual(
    commandRegistry.availableOn("leader", { bufferKind: "page" }).map((c) => c.id),
    ["page-only"],
  );
  // editor gate: editor-ins absent without ctx.editor, present with it.
  assert.deepEqual(commandRegistry.availableOn("slash", {}).map((c) => c.id), []);
  assert.deepEqual(
    commandRegistry.availableOn("slash", { editor: {} }).map((c) => c.id),
    ["editor-ins"],
  );
});
```

2b. RUN — FAILS: `node --test 'tests/unit/command-registry.test.mjs'`
    Expected: `availableOn is not a function`, the 3 new tests fail.

2c. MINIMAL IMPL — add the method to the `CommandRegistry` class right after `available()` (line 82):

```ts
  availableOn(surface: Surface, ctx: CommandContext): RegisteredCommand[] {
    return this.available(ctx).filter((cmd) => surfacesFor(cmd).has(surface));
  }
```

2d. RUN — PASSES: `node --test 'tests/unit/command-registry.test.mjs'` → `fail 0`.

2e. COMMIT: `feat(web/registry): add availableOn(surface, ctx) per-surface filter (Phase A)`

═══════════════════════════════════════════════════════════════════
TASK 3 — leader consumer → availableOn('leader', ctx)
═══════════════════════════════════════════════════════════════════
File: web/src/lib/v5/leader-tree.svelte.ts:112

3a. EDIT (real call-site change): in `getLeaderTree` (line 112), replace
    `const commands = (ctx ? commandRegistry.available(ctx) : commandRegistry.all()).filter(`
    with
    `const commands = (ctx ? commandRegistry.availableOn('leader', ctx) : commandRegistry.all()).filter(`
    KEEP the trailing `.filter(cmd => { const chord = effectiveChord(cmd, overrides); return chord && chord.length > 0; })` exactly — it's the override-aware chord gate and must stay on top (see Risks).

3b. VERIFY (build/typecheck) — from web/: `npx svelte-check --tsconfig ./tsconfig.json --threshold error 2>&1 | tail -5` → no NEW errors introduced by this file. (If svelte-check is slow, `npx tsc --noEmit -p tsconfig.json 2>&1 | grep leader-tree` → empty.)

3c. BROWSER-QA (named, NOT a unit test — leader render is UI): with the dev server up (`npm run dev` in web/), open /g, press the leader (Space in normal mode), descend a bucket (e.g. `g`), confirm the which-key menu shows the SAME commands as before this change (no bucket missing, no extra). Chrome DevTools: navigate_page → take_snapshot of the open leader overlay; eyeball parity. Mark `[?] awaiting human verify` only if you cannot drive it.

3d. COMMIT: `refactor(web/leader): read commands via availableOn('leader') (Phase A)`

═══════════════════════════════════════════════════════════════════
TASK 4 — slash consumer → availableOn('slash', baseCtx)
═══════════════════════════════════════════════════════════════════
File: web/src/lib/components/BlockEditor.svelte:1426-1428

4a. EDIT: in `getSlashTree`, change line 1427 from
    `      .available(baseCtx)`
    to
    `      .availableOn('slash', baseCtx)`
    Leave `.filter((cmd) => cmd.slashKey)` on line 1428 unchanged.

4b. VERIFY: from web/: `npx tsc --noEmit -p tsconfig.json 2>&1 | grep BlockEditor` → empty (no new type errors). (svelte-check also fine.)

4c. BROWSER-QA (named): dev server up, open a journal note, focus a block, type `/` — confirm the slash menu lists the same insertion verbs (Heading, Task, Link, …) and the `/p` Properties entry as before. On a #Task block, confirm the context props still hoist (this change doesn't touch them — propNodes are derived from `defs`, not the registry). Chrome DevTools snapshot of the open slash menu; eyeball parity vs main.

4d. COMMIT: `refactor(web/slash): read commands via availableOn('slash') (Phase A)`

═══════════════════════════════════════════════════════════════════
TASK 5 — palette consumer → availableOn('palette', ctx)
═══════════════════════════════════════════════════════════════════
File: web/src/lib/graphite/shell/GrCommandPalette.svelte:67

5a. EDIT: change line 67 from
    `  const allCommands = $derived(commandRegistry.available(ctx));`
    to
    `  const allCommands = $derived(commandRegistry.availableOn('palette', ctx));`

5b. VERIFY: from web/: `npx tsc --noEmit -p tsconfig.json 2>&1 | grep GrCommandPalette` → empty.

5c. BROWSER-QA (named): dev server up, open ⌘K palette on /g, confirm the command rows match pre-change (notes still appear — they come from `notesQuery`, unaffected). Snapshot; eyeball command-row parity.

5d. COMMIT: `refactor(web/palette): read commands via availableOn('palette') (Phase A)`

═══════════════════════════════════════════════════════════════════
TASK 6 — colon consumer → availableOn('colon', ctx)
═══════════════════════════════════════════════════════════════════
File: web/src/lib/components/shell/ColonCommandLine.svelte:51

6a. EDIT: change line 51 from
    `  const allCommands = $derived.by(() => (open ? commandRegistry.available(ctx) : []));`
    to
    `  const allCommands = $derived.by(() => (open ? commandRegistry.availableOn('colon', ctx) : []));`

6b. VERIFY: from web/: `npx tsc --noEmit -p tsconfig.json 2>&1 | grep ColonCommandLine` → empty.

6c. BROWSER-QA (named): dev server up, open `:` colon line, type a few chars, confirm the same verb suggestions appear as before (the hand-coded peek/graph BUILTINS at lines 54-57 are separate rows and unaffected by this change). Snapshot; eyeball suggestion parity.

6d. COMMIT: `refactor(web/colon): read commands via availableOn('colon') (Phase A)`

═══════════════════════════════════════════════════════════════════
FINAL GATE (after all 6): from web/ run the FULL unit suite once —
`node --test 'tests/unit/**/*.test.mjs' 2>&1 | tail -8` → `fail 0`. This proves the
slash-heading-command.test.mjs and any other registry-touching tests still pass.

### Risks / ordering / verify-against-live

ORDERING / DEPENDENCIES
- Task 1 → Task 2 → Tasks 3-6 is strict for the first two (availableOn calls surfacesFor; consumers call availableOn). 3-6 are independent of each other.
- This is the FOUNDATION phase for the whole command-model redesign (spec §"Registry changes this implies" item 1). Phase B (bucket metadata), Phase C (giving the ~30 chord-less commands a chord), and the slash paring all depend on `surfaces`/`availableOn` existing. Do NOT set any explicit `surfaces` on real commands in Phase A — that's later phases. Phase A must be a behavior no-op.

BACK-COMPAT FIDELITY (the load-bearing risk — verify against live code, do not trust this prose)
- The deriver's "editor → slash only" branch encodes a SUBTLE current truth: `available()` (registry lines 71-82) drops `surface:'editor'` commands whenever `ctx.editor` is absent, and ONLY the slash consumer builds a ctx with `.editor` set (BlockEditor `buildSlashCommandContext`). So editor commands never actually reach colon/palette/leader today even though nothing explicitly scopes them. The deriver returns `{slash}` for them — confirm by reading `buildSlashCommandContext`/`buildSlashContext` in BlockEditor.svelte (referenced at line 1423-1424) and confirming the palette/colon/leader ctx objects do NOT set `.editor`. If some other consumer DOES set ctx.editor, the deriver's editor→slash-only rule would HIDE an editor command that previously showed there → revisit.
- `editor.heading` (web/src/lib/editor/commands/heading.ts) is `surface:'global'` + `slashKey:'h'` — so it derives to ALL FOUR surfaces and STILL leaks to colon/palette today. Spec §4 wants that leak gated, but ONLY in a later phase via explicit `surfaces`. Phase A preserves the leak. The Task-1 test "surface:'global' yields all four" pins this intended no-op; if a reviewer expects heading gated now, that's out of scope for Phase A.

LEADER OVERRIDE INTERACTION (Task 3)
- The leader keeps its `effectiveChord(cmd, overrides)` presence filter ON TOP of `availableOn('leader', ...)`. Today a chord-carrying command derives to 'leader', and the effectiveChord filter also keeps it — consistent. Edge: a user override that ADDS a chord to a chord-less command, or REMOVES the only chord. `surfacesFor` derives 'leader' from the command's STATIC `cmd.chord`, ignoring overrides; the effectiveChord filter is override-aware. Net today: switching is a no-op because the effectiveChord filter is the tighter gate and runs second. Do NOT try to make availableOn override-aware in Phase A — leave overrides entirely to the existing effectiveChord filter. Flagged so the implementer doesn't "fix" the duplication.

SLASH REDUNDANT FILTER (Task 4)
- After `availableOn('slash', baseCtx)`, the trailing `.filter(cmd => cmd.slashKey)` is redundant (every slash-surface command back-compat-derives FROM slashKey). Keep it — it's harmless, and once later phases set explicit `surfaces` a command could be on 'slash' WITHOUT a slashKey, at which point this filter would (correctly, for now) still require slashKey because the slash menu keys off `cmd.slashKey`. Removing it is a separate decision, not Phase A.

TEST RUNNER
- Tests run from web/ (cwd matters: the glob `tests/unit/**` is relative). node imports the `.svelte.ts` module directly — confirmed working (existing 24 tests pass). The test file mocks KeyboardEvent globally (lines 5-15); don't remove that when appending.
- `ReadonlySet` is a TS type only; at runtime it's a plain `Set` — the tests construct `new Set([...])` and call `.has`, which is correct.

UI-ONLY, NO UNIT TEST
- Per the SHARED CONTRACT, grid layout / menu render / keyboard nav are NOT unit-tested. Tasks 3-6 each carry a named Chrome DevTools browser-QA parity check instead. The repo's pattern is self-QA via Chrome DevTools MCP (memory: feedback_self_qa) — drive it yourself; only mark `[?] awaiting human verify` if the dev server / device truly can't be driven.

---

## Phase B — Leader buckets + wide which-key grid + insert-mode chord

### Files

Phase B — leader buckets + wide which-key grid + insert-mode chord. All paths absolute.

CREATE:
- /Users/tfinklea/git/tesela/web/tests/unit/leader-tree.test.mjs
  Sole responsibility: assert getLeaderTree() builds the named-bucket structure with EVERY chord-carrying command homed and no orphans. First leader-tree unit test in the repo (none exists today — grep confirmed). Must mock `globalThis.$state` (and `$derived` if node evaluates the module-level `$derived` — leader-tree.svelte.ts has none at module scope, only inside fns, but mock both to be safe), import the real `.svelte.ts`, and `commandRegistry._reset()` + register a controlled fixture set per test (mirrors command-registry.test.mjs which resets per test).

MODIFY:
- /Users/tfinklea/git/tesela/web/src/lib/v5/leader-tree.svelte.ts
  (a) CHORD_GROUP_LABELS (lines 48-52) — replace the 3-entry table {n,g,b} with the full 10-bucket label table from the spec: g go to · w windows · b buffers · n new · i insert · p properties · v views · a actions · t toggle · , config. Owns the bucket display names.
  (b) BLOCKER FIX — change its two imports from `$lib/...` to relative (`./../command-registry.svelte.ts`, `./../stores/keybindings.svelte.ts`) so the new unit test can resolve the module under `node --test` (node cannot resolve the `$lib` alias — verified: probe failed with ERR_MODULE_NOT_FOUND '$lib'). No existing unit-tested module uses `$lib`; this is the first, so the alias was never a problem before.
- /Users/tfinklea/git/tesela/web/src/lib/v4/commands.ts
  Assign a `chord` to EVERY currently-chordless registered command (real edits to the inline command objects + the DERIVED_RENDERERS/SETTINGS_PAGES/AMBIENT helper arrays). Chordless inventory below.
- /Users/tfinklea/git/tesela/web/src/lib/editor/commands/{heading,task,link,tag,date,template,query,collection,property,widget}.ts
  Add `chord: ["i", <letter>]` to the 10 editor insertion commands (the `i insert` bucket). They currently have NO chord and are `surface:"global"` with a `slashKey` (verified heading.ts:13-14). Read each before editing.
- /Users/tfinklea/git/tesela/web/src/lib/graphite/shell/GrLeaderOverlay.svelte
  Multi-column WIDE which-key grid (CSS/layout only). The `.gr-leader-body` already uses `grid-template-columns: 1fr 1fr` (lines 242-247) — widen the popup (`.gr-leader` width line 210 is `min(460px,92%)`) and bump to 2-3 columns so it's wide-and-short. Browser-QA only.
- /Users/tfinklea/git/tesela/web/src/lib/components/BlockEditor.svelte
  Add a `Ctrl-,` entry to the cm6 `blockKeymap` array (ends ~line 2122-2123), mirroring the existing `Ctrl-d`/`Ctrl-u`/`g` entries (lines 2060-2102): read `cm.state.vim.insertMode`, dispatch `tesela:leader` (or `tesela:open-leader-at` with empty path) → reuses GraphiteShell's existing listener (GraphiteShell.svelte:254-255). Unlike the existing Ctrl-d/g entries that GUARD against insert mode (`if vs.insertMode return false`), this one must FIRE in insert mode (that's the whole point — open leader without leaving insert) and only yield if a real vim insert binding owns `Ctrl-,` (none does — confirm during impl). The dead-comment promise at ChordMenu.svelte:11 ("Ctrl+, from any mode") becomes real.

### Tasks

PRELIM (do first, no test): Verify Phase A landed. The shared contract says command-registry.svelte.ts should already have `Surface` type, `surfaces?: ReadonlySet<Surface>` on Command, and `availableOn(surface, ctx)`. As of the code I read it still has only `surface: 'global'|'editor'` and `available(ctx)` (command-registry.svelte.ts:31, 71-82) — Phase A is NOT yet present. If still absent when you start, Phase B's leader consumer must keep calling `available(ctx)` (as getLeaderTree does today, line 112) and NOT assume `availableOn` exists; flag the ordering to the orchestrator. The bucket work is independent of the surfaces field, so Phase B can proceed either way — just don't invent `availableOn`.

TASK 1 — Fix the $lib import blocker so leader-tree is unit-testable (2 min)
- Files: leader-tree.svelte.ts.
- Edit: change line 12-17 import from `"$lib/command-registry.svelte"` → `"../command-registry.svelte.ts"` and line 18 `"$lib/stores/keybindings.svelte"` → `"../stores/keybindings.svelte.ts"`. (Relative paths verified via node path.relative.)
- Run-it-fails-first is N/A (pure refactor enabling the next test). Verify nothing else broke: `cd /Users/tfinklea/git/tesela/web && npm run test:unit 2>&1 | tail -5` — still all pass. Also `npm run check` (svelte-check) must stay green — the `.ts` extension on a relative `.svelte.ts` import resolves fine in svelte-kit.
- Commit: "refactor(leader): relative imports in leader-tree so it's node-testable".

TASK 2 — Failing test: bucket label table + no-orphans (5 min)
- Files: CREATE leader-tree.test.mjs.
- Write REAL failing test. Header mirrors keybindings.test.mjs:5-21 (mock $state) + command-registry.test.mjs reset pattern:
  ```
  import test from "node:test";
  import assert from "node:assert/strict";
  globalThis.$state = (v) => v;
  globalThis.$derived = (v) => v; globalThis.$derived.by = (fn) => fn();
  const { commandRegistry } = await import("../../src/lib/command-registry.svelte.ts");
  const { getLeaderTree } = await import("../../src/lib/v5/leader-tree.svelte.ts");

  function reg(id, chord, label) {
    commandRegistry.register({ id, label: label ?? id, glyph: "x", category: "navigate", chord, keywords: [], run: () => {} });
  }
  test("named buckets get spec labels, not joined child labels", () => {
    commandRegistry._reset();
    reg("daily", ["g","d"], "Today's daily note");
    reg("graph", ["g","g"], "Fullscreen graph");
    reg("vsplit", ["w","v"], "Split vertically");
    const tree = getLeaderTree();
    const g = tree.find((n) => n.key === "g");
    assert.equal(g.label, "go to…");        // FROM CHORD_GROUP_LABELS, not "Today's daily note / Fullscreen graph"
    const w = tree.find((n) => n.key === "w");
    assert.equal(w.label, "windows…");
  });
  test("every chord-carrying command is homed under exactly one bucket (no orphans)", () => {
    commandRegistry._reset();
    reg("a", ["g","d"]); reg("b", ["w","v"]); reg("c", ["n","n"]);
    const tree = getLeaderTree();
    // collect every leaf action's owning top-level key; assert each top key is a known bucket
    const known = new Set(["g","w","b","n","i","p","v","a","t",",","/"," "]);
    for (const node of tree) assert.ok(known.has(node.key), `bucket key "${node.key}" not in taxonomy`);
  });
  ```
- Run-it-fails: `cd /Users/tfinklea/git/tesela/web && node --test 'tests/unit/leader-tree.test.mjs' 2>&1 | tail -15`. Expect the first test to FAIL: today CHORD_GROUP_LABELS has no "w" entry and "g" maps to "go to…" already — so seed the failing assertion on a NEW bucket the current table lacks (`w` → "windows…", or `v`/`a`/`t`). Confirm red before impl.
- Commit after green (with Task 3).

TASK 3 — Minimal impl: full bucket label table (2 min)
- Files: leader-tree.svelte.ts CHORD_GROUP_LABELS (lines 48-52).
- REAL replacement:
  ```
  const CHORD_GROUP_LABELS: Record<string, string> = {
    g: "go to…", w: "windows…", b: "buffers…", n: "new…", i: "insert…",
    p: "properties…", v: "views…", a: "actions…", t: "toggle…", ",": "config…",
  };
  ```
- Run-passes: re-run Task 2 command — green.
- Commit: "feat(leader): full which-key bucket label table (g/w/b/n/i/p/v/a/t/,)".

TASK 4 — Failing test: chordless commands get a bucket home (the big one) (5 min)
- Files: leader-tree.test.mjs (append).
- This test asserts the REAL registry (loaded by importing v4/commands.ts + editor/commands) leaves NO command chordless. Write:
  ```
  test("real registry: no registered command is left without a chord (every command homed)", async () => {
    await import("../../src/lib/v4/commands.ts");        // side-effect registers V4 set
    await import("../../src/lib/editor/commands/heading.ts"); // ...import all 10 editor cmds
    // (import each editor/commands/*.ts so they register)
    const { effectiveChord } = await import("../../src/lib/command-registry.svelte.ts");
    const chordless = commandRegistry.all().filter((c) => {
      const ch = effectiveChord(c, {});
      return !ch || ch.length === 0;
    });
    assert.deepEqual(chordless.map((c) => c.id), [], `chordless: ${chordless.map((c)=>c.id).join(", ")}`);
  });
  ```
  NOTE: this test must NOT call `_reset()` (it needs the real side-effect-registered set). Put it in its own file OR run after the fixture tests with a fresh import — since `_reset()` in earlier tests clears the singleton, re-importing v4/commands.ts won't re-register (the `v4CommandsRegistered` guard at commands.ts:344,723-727 blocks re-run). SAFEST: put this real-registry assertion in a SEPARATE test file (leader-tree-real.test.mjs) that never calls _reset, so the module-load registration is intact. Flag this guard interaction.
- Run-it-fails: `node --test 'tests/unit/leader-tree-real.test.mjs'`. Expect FAIL listing the ~20 chordless ids: jump, promote, delete-tag, convert-to-tag, convert-to-note, rename-slug, prune-scratches, keymap, skip-occurrence, tabnew, tab-close, instances-of-tag, backlinks-of-tag, settings-devices/sync/mosaic/data, the 5 DERIVED_RENDERERS (backlinks/outline/properties/tasks/graph-local), and the 10 editor.* insertion cmds.

TASK 5 — Minimal impl: assign chords to all chordless commands (5 min, may split per file)
- Files: commands.ts + the 10 editor/commands/*.ts. Read each before editing.
- Bucket assignments (spec §2 taxonomy; pick non-colliding 2nd letters):
  - i insert (editor/commands/*): heading→["i","h"], task→["i","t"], link→["i","l"], tag→["i","g"], date→["i","d"], template→["i","m"], query→["i","q"], collection→["i","c"], property(editor)→ route to p bucket: ["p","p"] OR ["i","p"] (spec puts properties under `p`; editor.property is the context-aware props entry — home it under `p`), widget→ new bucket per spec ("New widget → leader new bucket"): ["n","w"].
  - w windows (in commands.ts): vsplit/hsplit/close-pane currently use ["b",...] (b=buffer today) — RE-HOME to w per spec: vsplit→["w","v"], hsplit→["w","s"], close-pane→["w","q"], + add chords to the 4 move-{left,right,up,down} (commands.ts:384-394, currently chordless): ["w","h"/"l"/"k"/"j"]. CAUTION: re-homing vsplit/hsplit from b→w changes existing muscle-memory + the BlockEditor comment at line 144 references "Space b p"; update/verify no doc references break.
  - b buffers/tabs: tabnew→["b","t"], tab-close→["b","c"], jump→["b","j"].
  - n new: scratch (["n","s"] already), new-note (["n","n"] already), promote→["n","p"].
  - v views (DERIVED_RENDERERS array, commands.ts:61-67 — add a chord field + thread it through the .map at 436-444): backlinks→["v","b"], outline→["v","o"], properties→["v","p"], tasks→["v","t"], graph-local→["v","g"]; instances-of-tag→["v","i"], backlinks-of-tag→["v","k"].
  - a actions: convert-to-tag→["a","t"], convert-to-note→["a","n"], rename-slug→["a","r"], prune-scratches→["a","p"], delete-tag→["a","d"], skip-occurrence→["a","s"], keymap→["a","k"].
  - t toggle: peek currently chord ["p"] (commands.ts:639) — RE-HOME to ["t","p"] per spec (peek belongs in toggle; frees bare `p` for the properties bucket). Verify peek's ⌘I shortcut + PeekPopover wiring still fire (shortcut is separate from chord).
  - , config: settings-general already [","]; settings-devices/sync/mosaic/data → [",","d"/"s"/"m"/"a"] (thread chord through SETTINGS_PAGES map at commands.ts:53-59 + 608-617).
  - Leave-as-is: command-station chord ["/"] (SPC / station leaf), daily/goto/yesterday/tomorrow/graph under g, AMBIENTS under g.
- Run-passes: Task 4 test goes green (chordless list empty).
- COLLISION GATE: after assigning, run `findConflicts()` via a quick assertion or the keymap overlay — two commands must not share an identical full chord path. Add a test: `assert.equal(findConflicts().filter(c=>c.kind==="chord").length, 0)`. The leader tree's buildChordTree handles leaf+subtree-sharing-a-key gracefully (lines 76-91) but a true duplicate full-path is a bug.
- Commit: "feat(leader): home every command in a which-key bucket (g/w/b/n/i/p/v/a/t/,)".

TASK 6 — Browser-QA: wide multi-column grid (CSS, no unit test)
- Files: GrLeaderOverlay.svelte (`.gr-leader` width line 210; `.gr-leader-body` grid lines 242-247).
- Impl: widen `.gr-leader` (e.g. `min(720px, 94%)`) + set `.gr-leader-body` to `grid-template-columns: repeat(3, 1fr)` (or `repeat(auto-fill, minmax(200px,1fr))`) so buckets lay out wide-and-short like which-key.nvim. Keep navigation untouched (handleKeydown lines 94-129 is key-driven, layout-agnostic).
- Named browser-QA step "leader-grid-qa": rebuild web dev server (I own the dev shell), Chrome DevTools MCP → navigate to /g → press Space → take_screenshot. CONFIRM: top-level shows g/w/b/n/i/p/v/a/t/, in a wide multi-column grid (not a tall single column); press `g` → descends into go-to bucket; Esc → ascends; press `i` opens the FILTER (reserved, ChordMenu.svelte:289) — NOTE this is the GrLeaderOverlay, which does NOT implement the `i`-filter (only ChordMenu does); verify `i` instead descends into the insert bucket here (GrLeaderOverlay has no searchOpen branch — confirm `i` matches the insert bucket node, not a filter). This is a real divergence between the two renderers — flag which one /g actually mounts (GraphiteShell mounts GrLeaderOverlay, line 314).
- Commit: "feat(leader): wide multi-column which-key grid layout".

TASK 7 — Browser-QA: Ctrl+, insert-mode handler (no unit test — cm6/DOM)
- Files: BlockEditor.svelte blockKeymap array (before its closing `]);` ~line 2123).
- Impl, mirroring the Ctrl-d entry (lines 2060-2069) but INVERTING the insert guard:
  ```
  {
    key: "Ctrl-,",
    run: (v) => {
      // Opens the SAME leader overlay WITHOUT leaving insert mode (spec §3).
      // Unlike Ctrl-d/g above, we fire in insert mode — that's the point.
      document.dispatchEvent(new CustomEvent("tesela:leader"));
      return true;
    },
  },
  ```
  Reuses GraphiteShell's existing `tesela:leader` → openLeader() listener (GraphiteShell.svelte:254-255). Verify cm6 accepts the `Ctrl-,` key string (cm6 keymap uses `Ctrl-` prefix; comma key name may need `Ctrl-Comma` — TEST both in browser-QA, the existing entries use single letters so the comma is unverified). If cm6 won't bind it, fall back to a capture-phase document keydown in GraphiteShell's onKey (lines 167-247) gated on `e.ctrlKey && e.key === ',' ` — but try the cm6 keymap first (it's the editor-scoped, insert-safe path).
- GUARD against vim insert bindings: spec §86 says confirm Ctrl+, doesn't collide. cm-vim's default insert-mode bindings are C-w/C-u/C-r/C-o etc — `Ctrl-,` is not among them, so no guard needed beyond verifying in browser. Do NOT add an `if vs.insertMode return false` (that would defeat the feature).
- Named browser-QA step "ctrl-comma-qa": Chrome DevTools MCP → /g → click into a block → press `i` (enter INSERT, confirm status shows INSERT) → press Ctrl+, → CONFIRM the leader overlay opens AND the editor stays in insert mode (cursor still in block, no mode flip). Then Esc closes leader, cursor still live. Also confirm typing a literal comma in insert mode (no Ctrl) still types `,`.
- Commit: "feat(editor): Ctrl+, opens leader from insert mode".

ORDERING: Task 1 → 2 → 3 (label table) can land independently. Task 4 → 5 (chord assignment) is the bulk and depends only on Task 3's labels. Tasks 6 & 7 are independent CSS/handler work, browser-QA gated, can land last in either order.

### Risks / ordering / verify-against-live

SHARED-CONTRACT / PHASE-A ORDERING (highest risk): The shared contract describes a Phase-A `Surface` type + `surfaces?: ReadonlySet<Surface>` + `availableOn(surface, ctx)`. The live command-registry.svelte.ts I read has NEITHER — it still has binary `surface: 'global'|'editor'` and only `available(ctx)` (lines 31, 71-82). So Phase A has not landed yet. Phase B must NOT assume `availableOn` exists; getLeaderTree today filters via `available(ctx)` (leader-tree.svelte.ts:112) — keep that. If Phase A lands first and switches the leader consumer to `availableOn('leader', ctx)`, the bucket tests still hold (they assert structure, not surface filtering). Verify which is live before writing the consumer call.

$lib ALIAS BLOCKER (verified, must fix in Task 1): `node --test` cannot resolve `$lib` — probe of importing leader-tree.svelte.ts failed with `ERR_MODULE_NOT_FOUND: Cannot find package '$lib'`. No existing unit-tested module uses `$lib` (command-registry/chord-keys/keybindings all use relative imports), so leader-tree is the first. The relative-import switch is mandatory for the unit test to exist at all. svelte-check (`npm run check`) must stay green after — `.svelte.ts` relative imports with explicit extension resolve in svelte-kit.

REGISTRATION GUARD (verified): commands.ts has `v4CommandsRegistered` (line 344) gating re-registration (723-727). After any `commandRegistry._reset()` in a test, re-importing v4/commands.ts will NOT re-register the set. So the real-registry no-orphans assertion (Task 4) must live in its OWN test file that never calls `_reset()`, relying on module-load side-effect registration. Mixing fixture (_reset) tests and real-registry tests in one file will silently empty the registry.

TWO LEADER RENDERERS DIVERGE: GraphiteShell mounts GrLeaderOverlay (line 314), NOT ChordMenu. GrLeaderOverlay has a 2-col grid (lines 242-247) but NO `i`-filter / search mode — only ChordMenu.svelte does (lines 112-115, 289). So on /g, pressing `i` descends into the insert bucket (a real chord now), it does NOT open a filter. ChordMenu's SLASH_RESERVED_CHORDS reserving `i` (chord-keys.ts:81) is a slash-menu concern, not the /g leader. The wide-grid CSS (Task 6) goes in GrLeaderOverlay. If Taylor wants the `i`-filter on the leader too, that's out of scope for Phase B — flag it.

RE-HOMING MUSCLE MEMORY: Task 5 moves vsplit/hsplit/close-pane from the `b` bucket (current chords ["b","v"]/["b","h"]/["b","q"], commands.ts:356/367/377) to `w` per spec, and peek from bare ["p"] to ["t","p"]. BlockEditor.svelte:144 has a comment referencing "Space b p" — verify it's stale/harmless after re-home. The peek ⌘I shortcut + PeekPopover (GraphiteShell:322) are driven by `shortcut`/store, independent of the chord, so peek still works via ⌘I — but confirm.

CHORD COLLISIONS: ~30 new chords across 10 buckets risks duplicate full-paths. buildChordTree tolerates a leaf+subtree sharing one key (leader-tree.svelte.ts:76-91) but two leaves with the identical full chord is a bug. Add a `findConflicts()` chord-conflict==0 assertion (Task 5) and check the keymap overlay. The proposed 2nd letters in Task 5 are first-pass; adjust on collision (e.g. within `a` bucket, convert-to-tag/convert-to-note both want sensible letters — t/n chosen to avoid clash).

cm6 KEY STRING for comma (Task 7): existing blockKeymap entries use single letters (`Ctrl-d`, `g`). `Ctrl-,` as a cm6 KeyBinding `key` is unverified — cm6 may need `Ctrl-Comma`. Test in browser-QA; if neither binds, fall back to a capture-phase handler in GraphiteShell.onKey. Do NOT add an insert-mode guard (the feature REQUIRES firing in insert mode — opposite of the Ctrl-d/g entries which guard it out).

EDITOR-BUCKET VISIBILITY: the 10 editor.* insertion commands need `ctx.editor` to pass `available()` (registry line 73: `surface==='editor'` filtered without editor ctx) — BUT heading.ts is `surface:"global"` (verified line 13), not "editor", so it shows even without editor ctx. The insert bucket's membership in the leader tree therefore depends on each command's surface + the ctx GraphiteShell passes (commandCtx has no `editor` field — GraphiteShell.svelte:59-68). Verify the `i` bucket actually populates in the /g leader (browser-QA Task 6) — if editor.* are surface:"editor" they'd be filtered out there. This is exactly the "gate the surface:'global' leak" concern from spec §90; resolve per the shared-contract back-compat defaults, coordinating with Phase A.

---

## Phase C — Slash type-to-filter + Ctrl-accelerators + pared context-aware

### Files

All paths absolute. Phase C = three concerns: (A) pure slash matcher, (B) pared context-aware getSlashTree, (C) the type-to-filter + Ctrl-accelerator UI in ChordMenu (browser-QA only).

NEW — `/Users/tfinklea/git/tesela/web/src/lib/editor/slash-filter.ts`
  - ONE responsibility: pure `slashFilter<T extends { label: string }>(items, query)` — given the flat slash rows and the user's typed query, return the matched+ranked subset (Enter picks `[0]`). Thin wrapper over the EXISTING `scoreFuzzy` from `$lib/fuzzy` (do NOT reinvent ranking). Empty query → return items unchanged (full list, original order). Non-empty → keep score>0, sort by score desc, stable tie-break on original index (so equal-score rows keep tree order). Also export the per-item score/positions if the menu wants highlight runs (mirror GrCommandPalette's `scoreFuzzy(label,q).positions` usage). Pure, no Svelte, no DOM.

NEW — `/Users/tfinklea/git/tesela/web/tests/unit/slash-filter.test.mjs`
  - `node --test` unit tests for slashFilter (import the `.ts` directly via top-level `import` like slash-heading-command.test.mjs:4 does — the runner supports it, just confirmed green).

MODIFY — `/Users/tfinklea/git/tesela/web/src/lib/components/BlockEditor.svelte`
  - `getSlashTree()` (1407-1490): REWRITE to the pared shape. Drop: the hoisted `propNodes` (1461-1466), the `fallbackStatus` / `/s` block (1468-1487), and the `assignChords` top-level pass (1454-1466). Keep: registry leaves (1426-1437) but source them from `commandRegistry.availableOn('slash', baseCtx)` (Phase A's new method) instead of `.available(baseCtx).filter(c=>c.slashKey)` — and drop `editor.widget` from slash (it moves to leader `new`; verify it's excluded either by its Phase-A `surfaces` set OR by an explicit id guard here). Add ONE `Properties` node `{ key:'p', label:'Properties', children: getPropertyChildren() }` (getPropertyChildren at 1379 already returns the context-aware, tag-scoped defs — Task→Status/Priority/… or the Manual-key leaf when untyped). Net tree = 8 verbs + Properties.
  - Slash menu render (2346-2362): pass the new type-to-filter mode to `<ChordMenu>` (a flag/prop, e.g. `filterMode` or `headLabel`-driven) so slash opens in type-to-filter while the leader stays chord-press. The tree value stays `slashOverrideTree ?? getSlashTree()`.

MODIFY — `/Users/tfinklea/git/tesela/web/src/lib/components/ChordMenu.svelte`
  - Add a `filterMode`/`accelerators` prop (default off → leader keeps today's chord-press + `i`-filter behavior UNCHANGED). When on: typed bare keys go to a query string that runs `slashFilter(currentLevel, query)`; ↑/↓ navigate; Enter picks highlighted; `Ctrl+letter` is an express accelerator that jumps to the node whose accelerator letter matches (active only while open). This REPLACES the bare-single-letter chord match (296-302) for the slash surface only. Reconcile the existing `i`-filter (289-294) and `searchOpen` machinery — in filterMode the whole menu IS the filter (no separate `i` trigger). UI-only; covered by browser-QA, not a unit test.

MODIFY — `/Users/tfinklea/git/tesela/web/src/lib/chord-keys.ts`
  - `BUILTIN_SLASH_CHORDS` (83-94): drop `["w","New widget"]` and `["p","All properties"]` rows (widget leaves slash; `p` is now the single context-aware Properties entry, not "All properties"). This map also feeds `buildKeymapIndex` (command-registry 161-171) so keep it the source of truth for the slash keymap docs. Leave `SLASH_RESERVED_CHORDS`/`assignChords` intact (still used by getPropertyChildren value submenus).

### Tasks

TASK 1 — slashFilter: full list on empty query (TDD)
  Files: NEW web/tests/unit/slash-filter.test.mjs, NEW web/src/lib/editor/slash-filter.ts
  Write failing test:
    ```js
    import assert from "node:assert/strict";
    import test from "node:test";
    import { slashFilter } from "../../src/lib/editor/slash-filter.ts";
    const tree = [
      { label: "Heading" }, { label: "Task" }, { label: "Link" },
      { label: "Tag picker" }, { label: "Date" }, { label: "Template" },
      { label: "Query" }, { label: "Collection" }, { label: "Properties" },
    ];
    test("empty query returns all items in original order", () => {
      const out = slashFilter(tree, "");
      assert.deepEqual(out.map((i) => i.label), tree.map((i) => i.label));
    });
    ```
  Run-it-fails: `cd web && node --test 'tests/unit/slash-filter.test.mjs'` → fails (module not found / slashFilter undefined).
  Minimal impl (slash-filter.ts):
    ```ts
    import { scoreFuzzy } from "$lib/fuzzy";
    export type SlashFilterItem = { label: string };
    export function slashFilter<T extends SlashFilterItem>(items: T[], query: string): T[] {
      const q = query.trim();
      if (!q) return [...items];
      return items
        .map((item, i) => ({ item, i, score: scoreFuzzy(item.label, q).score }))
        .filter((r) => r.score > 0)
        .sort((a, b) => b.score - a.score || a.i - b.i)
        .map((r) => r.item);
    }
    ```
    NOTE: confirm the test runner resolves the `$lib` alias under node --test; slash-heading-command.test.mjs imports `../../src/lib/...` by relative path, NOT `$lib`. If `$lib/fuzzy` fails to resolve in node, change the import to the relative `../fuzzy.ts`. Verify by running the test; if it errors on the alias, switch to relative and re-run.
  Run-passes: same command → 1 pass.
  Commit: "feat(web): pure slashFilter matcher over scoreFuzzy (Phase C)".

TASK 2 — slashFilter: prefix beats subsequence + ranking/tie-break (TDD)
  Files: web/tests/unit/slash-filter.test.mjs (append)
  Write failing test:
    ```js
    test("prefix match ranks above subsequence; Enter target is [0]", () => {
      const out = slashFilter(tree, "ta");
      // "Tag picker" + "Task" are prefix; "Template" (T-a via subseq) ranks lower
      assert.equal(out[0].label === "Task" || out[0].label === "Tag picker", true);
      assert.ok(out.length >= 2);
    });
    test("query 'prop' surfaces Properties", () => {
      assert.equal(slashFilter(tree, "prop")[0].label, "Properties");
    });
    test("no-match query returns empty", () => {
      assert.deepEqual(slashFilter(tree, "zzzz"), []);
    });
    test("equal-score ties keep original tree order (stable)", () => {
      const eq = [{ label: "Date" }, { label: "Deadline" }];
      assert.deepEqual(slashFilter(eq, "d").map((i) => i.label), ["Date", "Deadline"]);
    });
    ```
  Run-it-fails first if impl lacked stable tie-break — but TASK 1 impl already includes `|| a.i - b.i`, so these should pass once written; if any fails, that's a real ranking bug to fix in slash-filter.ts (don't weaken the test).
  Run-passes: `cd web && node --test 'tests/unit/slash-filter.test.mjs'`.
  Commit: "test(web): slashFilter ranking + stable tie-break".

TASK 3 — pared getSlashTree shape: untyped block (TDD-as-feasible)
  Files: NEW web/tests/unit/slash-tree.test.mjs (if getSlashTree can be extracted as a pure helper), OR a browser-QA fallback if it can't.
  FIRST verify: getSlashTree (BlockEditor.svelte:1407) closes over component-local state (propertyDefs, statusChoices, view, buildSlashContext). To unit-test the SHAPE, extract the pure tree-assembly into a testable function, e.g. NEW web/src/lib/editor/slash-tree.ts exporting `buildSlashTree({ registryLeaves, propertyChildren })` that returns `[...verbLeaves, { key:'p', label:'Properties', children: propertyChildren }]`. The Svelte getSlashTree becomes a thin caller passing `commandRegistry.availableOn('slash', ctx)`-derived leaves + `getPropertyChildren()`. Read 1421-1489 before extracting to keep the registry-leaf mapping (key=slashKey, label, action, hint=glyph) identical.
  Write failing test (slash-tree.test.mjs):
    ```js
    import assert from "node:assert/strict";
    import test from "node:test";
    import { buildSlashTree } from "../../src/lib/editor/slash-tree.ts";
    const verbLeaves = ["Heading","Task","Link","Tag picker","Date","Template","Query","Collection"]
      .map((label, i) => ({ key: "htltdtqc"[i], label, action: () => {} }));
    test("untyped block: 8 verbs + Properties→Manual leaf, no hoisted props, no /s", () => {
      const tree = buildSlashTree({ verbLeaves, propertyChildren: [{ key: "k", label: "Manual key:: value", action: () => {} }] });
      assert.equal(tree.length, 9);
      assert.equal(tree[8].label, "Properties");
      assert.equal(tree[8].key, "p");
      assert.deepEqual(tree[8].children.map((c) => c.label), ["Manual key:: value"]);
      assert.equal(tree.find((n) => n.key === "s"), undefined); // /s fallback dropped
      assert.equal(tree.find((n) => n.label === "New widget"), undefined); // widget gone
    });
    ```
  Run-it-fails: `cd web && node --test 'tests/unit/slash-tree.test.mjs'` → module missing.
  Minimal impl: slash-tree.ts with the pure assembler; then rewrite BlockEditor.svelte getSlashTree to call it (drop propNodes 1461-1466, fallbackStatus 1468-1487, assignChords pass).
  Run-passes: same command.
  Commit: "feat(web): pare slash tree to 8 verbs + context-aware Properties (Phase C)".

TASK 4 — pared getSlashTree shape: #Task block (context-aware Properties) (TDD)
  Files: web/tests/unit/slash-tree.test.mjs (append)
  Write test (propertyChildren simulates a #Task block's tag defs — Status/Priority/Deadline/Scheduled/Points; in the real component getPropertyChildren derives these from `propertyDefs`):
    ```js
    test("#Task block: Properties children are the tag-scoped defs, NOT hoisted to top level", () => {
      const taskProps = ["Status","Priority","Deadline","Scheduled","Points"]
        .map((label, i) => ({ key: "spdsp"[i], label }));
      const tree = buildSlashTree({ verbLeaves, propertyChildren: taskProps });
      // Properties is ONE top-level row; the 5 defs live UNDER it, not hoisted.
      assert.equal(tree.length, 9);
      const props = tree.find((n) => n.label === "Properties");
      assert.deepEqual(props.children.map((c) => c.label), ["Status","Priority","Deadline","Scheduled","Points"]);
      // none of the defs leaked to the top level
      for (const label of ["Status","Priority","Deadline"]) {
        assert.equal(tree.filter((n) => n.label === label).length, 0);
      }
    });
    ```
  Run-it-fails / Run-passes: `cd web && node --test 'tests/unit/slash-tree.test.mjs'` (passes once TASK 3's assembler is in; if a def leaks to top level the assert catches the regression).
  Commit: "test(web): slash Properties node is context-aware, not hoisted".

TASK 5 — wire registry surface + drop widget/All-properties from BUILTIN_SLASH_CHORDS (TDD via command-registry surface)
  Files: web/src/lib/chord-keys.ts, web/tests/unit/command-registry.test.mjs (append) OR slash-tree integration
  PRECONDITION: depends on Phase A's `availableOn('slash', ctx)`. If Phase A has NOT landed yet (verified: surfaces/availableOn absent from command-registry.svelte.ts today), STOP and flag — Phase C's getSlashTree rewrite must call `availableOn('slash', baseCtx)` per the shared contract; do not fork a slash-only filter. If Phase A is present, proceed.
  Write failing test (append to command-registry.test.mjs, mirroring its style):
    ```js
    test("availableOn('slash') excludes editor.widget", () => {
      // widget's surfaces set must omit 'slash' (moves to leader new bucket)
      const slash = commandRegistry.availableOn("slash", { editor: {} }).map((c) => c.id);
      assert.ok(!slash.includes("editor.widget"));
      assert.ok(slash.includes("editor.task"));
    });
    ```
  Run-it-fails: `cd web && node --test 'tests/unit/command-registry.test.mjs'` (fails if widget still surfaces on slash).
  Minimal impl: in editor/commands/widget.ts set `surfaces` to omit 'slash' (or rely on Phase A's derivation + an explicit exclusion); edit chord-keys.ts BUILTIN_SLASH_CHORDS to drop the `w`/`p` rows. Re-run command-registry.test.mjs (existing "buildKeymapIndex includes builtin slash chords" test will now iterate the smaller map — confirm it stays green).
  Run-passes: `cd web && node --test 'tests/unit/command-registry.test.mjs' && node --test 'tests/unit/slash-filter.test.mjs' 'tests/unit/slash-tree.test.mjs'`.
  Commit: "feat(web): drop widget+All-properties from slash keymap (Phase C)".

TASK 6 — ChordMenu type-to-filter + Ctrl-accelerator (BROWSER-QA, no unit test)
  Files: web/src/lib/components/ChordMenu.svelte, web/src/lib/components/BlockEditor.svelte
  Impl: add a `filterMode` prop to ChordMenu (default false → leader unchanged). When true: bare keystrokes append to a live query → render `slashFilter(currentLevel, query)`; ↑/↓ move highlight; Enter runs the highlighted node's action (or descends if it has children, e.g. Properties); `Ctrl+letter` jumps to the node carrying that accelerator (active only while open). BlockEditor passes `filterMode` when rendering the slash `<ChordMenu>` (2346-2361); leader callers do not.
  NAMED BROWSER-QA STEP (Chrome DevTools MCP, the repo's pattern — drive a sim/dev server, not Taylor): 
    1. Start the web dev server; open a journal note; focus a block; type `/` → menu opens showing 8 verbs + Properties.
    2. Type `head` → list narrows to Heading highlighted; press Enter → `# ` inserted. (type-to-filter + Enter-picks-[0])
    3. Type `/` again, press `Ctrl+P` → Properties submenu opens directly (accelerator express-lane).
    4. On a `#Task` block: `/` → `Ctrl+P` → submenu shows Status/Priority/Deadline/Scheduled/Points (context-aware, scoped to the block's tag).
    5. `/` then `↓ ↓ Enter` navigates and picks the 3rd row.
    6. Open the LEADER (Space) and confirm it STILL chord-presses (single letters descend) — filterMode did not regress it.
  No commit until QA passes; then: "feat(web): slash type-to-filter + Ctrl accelerators in ChordMenu (Phase C)".
  Final full-suite gate: `cd web && npm run test:unit`.

### Risks / ordering / verify-against-live

ORDERING / PHASE-A DEPENDENCY (hard): Phase C's getSlashTree rewrite and TASK 5 consume `commandRegistry.availableOn('slash', ctx)` and the `surfaces` field — VERIFIED ABSENT today (grep of command-registry.svelte.ts found no `surfaces`/`availableOn`/`Surface`). Phase A must land first. If executing Phase C before A: either (a) block on A, or (b) temporarily keep the current `.available(baseCtx).filter(c=>c.slashKey)` leaf source and add an explicit `editor.widget` id-exclusion, leaving a `<!-- TODO: switch to availableOn('slash') when Phase A lands -->` — but do NOT invent a parallel slash-only ownership scheme (violates the shared contract).

WIDGET REMOVAL: editor.widget currently has `surface: "editor"` + `slashKey: "w"` (widget.ts:9,12). Dropping it from slash means it must still reach the leader `new` bucket — that's a LEADER-phase concern, not Phase C. Phase C only ensures it stops appearing in slash; don't delete the command or its run().

$lib ALIAS IN NODE TESTS: slash-heading-command.test.mjs imports by RELATIVE path (`../../src/lib/...`), not `$lib`. The new slash-filter.ts imports `$lib/fuzzy` — confirm node --test resolves `$lib` (it may not without the Vite/svelte-kit alias). If it errors, change slash-filter.ts to `import { scoreFuzzy } from "../fuzzy.ts"` (or "../fuzzy"). Verify by running the test, not by assuming.

getSlashTree EXTRACTION: getSlashTree closes over component state (propertyDefs, statusChoices, view, buildSlashContext, commandRegistry). To unit-test the tree SHAPE (TASKS 3-4) you must extract a PURE assembler (slash-tree.ts) that takes already-built `verbLeaves` + `propertyChildren` and returns the array. Do NOT try to render the Svelte component in node. The action closures aren't asserted (untestable in node) — tests assert structure (count, keys, labels, nesting, absence of hoisted props / `/s` / widget) only. If extraction proves infeasible without dragging in Svelte runes, fall back to a browser-QA assertion of the rendered menu rows and note it.

getPropertyChildren UNCHANGED: it already returns the context-aware tag-scoped defs (Manual-key leaf when `propertyDefs` empty; one node per def otherwise — 1379-1404). Phase C must NOT rewrite it; just call it for the single Properties node's children. The OLD `getPropertyChildren` was `/p`'s "All properties" children — same function, now the only Properties surface. Confirm the `p` key doesn't collide once the hoisted props + assignChords pass are gone (with no top-level defs competing, `p` is free).

ChordMenu REGRESSION: the leader reuses ChordMenu with its `i`-filter/`searchOpen` flow (112-115, 289-294, 253-286) and bare-letter chord match (296-302). Adding `filterMode` must default OFF so the leader's chord-press + `i`-filter behavior is byte-identical. The slash surface turns filterMode ON and the WHOLE menu becomes the filter (no separate `i` trigger, no `searchOpen` toggle). Verify both paths in the browser-QA step (step 6 guards the leader).

BUILTIN_SLASH_CHORDS DOWNSTREAM: this map also feeds buildKeymapIndex (command-registry 161-171) and the keymap config UI. Dropping `w`/`p` rows shrinks the slash keymap docs — the existing test "buildKeymapIndex includes builtin slash chords" iterates the map so it auto-adapts, but re-run command-registry.test.mjs to confirm nothing asserted the old count.

SHIFT-KEY CHORDS: BUILTIN_SLASH_CHORDS has both `t`(Task) and `T`(Tag picker) — case-sensitive. Under type-to-filter, the bare `t`/`T` distinction disappears (you type "tag" vs "task"). For Ctrl-accelerators, decide whether Ctrl+T maps to Task or Tag (collision) — pick one and document; the other is reachable by typing. Don't silently map both to one letter.

---

## Phase D — `:` narrowing to exact-verbs + fold colon builtins

### Files

Phase D — `:` narrowing + fold colon builtins. One responsibility per file. Depends on Phase A (`type Surface`, `Command.surfaces`, `commandRegistry.availableOn(surface, ctx)` must already exist + be exported from `web/src/lib/command-registry.svelte.ts`).

MODIFY — `web/src/lib/v4/commands.ts`
  - Responsibility: make `peek`/`graph` colon-surfaced so `:` resolves them from the registry (no builtins).
  - The two commands already exist: `peek` (id `peek`, verb `peek`, ~L633-643) and `fullscreen-graph` (id `fullscreen-graph`, verb `graph`, ~L644-654). Add `surfaces` sets that include `'colon'` to BOTH (plus their existing homes: peek has chord `['p']`+shortcut `⌘I` → also `'leader'`/`'palette'`; graph has chord `['g','g']`+shortcut `⌘G` → `'leader'`/`'palette'`). Per Phase-A back-compat defaults a chord-carrying command already derives `'leader'` and everything derives `'palette'`/`'colon'` — so if the derived default already yields `'colon'`, NO explicit `surfaces` is needed here; verify against the Phase-A derivation before adding. The acceptance is that `availableOn('colon', ctx)` includes both verbs.

MODIFY — `web/src/lib/components/shell/ColonCommandLine.svelte`
  - Responsibility: narrow `:` to exact-verbs-only via `availableOn('colon', ctx)` and delete the hand-duplicated builtins.
  - L51: `commandRegistry.available(ctx)` → `commandRegistry.availableOn('colon', ctx)`.
  - L54-57: delete the `BUILTINS` array entirely.
  - L67-95 (`suggestions` $derived): delete the `for (const b of BUILTINS)` loop (L71-80); keep the `allCommands` loop (L82-93) which already calls `matchesV4Command` over `availableOn('colon')` rows. The `if (!cmd.verb) continue` gate at L83 stays (colon is verb-only).
  - L121-131 (`runVerb`): delete the `if (verb === "peek")`/`if (verb === "graph")` special-cases; let everything fall through to `findCommandByVerb(verb)` (L132) which resolves the registry `peek`/`graph` commands.
  - L162: `if (exact || typedVerb === "peek" || typedVerb === "graph")` → `if (exact)` (the `||` peek/graph clauses are now redundant — `findCommandByVerb` makes `exact` truthy for them).
  - L25-26: remove now-unused imports `openPeek` (peek store) and `openFullscreenGraph` (fullscreen-overlay store) IF nothing else in the file uses them (grep the file first — they're only referenced inside the deleted `runVerb` branches).

NO new file for the unit test — EXTEND the existing suite:
MODIFY — `web/tests/unit/command-registry.test.mjs`
  - Responsibility: assert `:`-surface resolution from the registry (not builtins) + palette-only exclusion. Pure-logic tests over `availableOn` + `findByVerb`; this file already imports `command-registry.svelte.ts` directly and uses `commandRegistry._reset()` per test (L20-32 import block, the `_reset()` idiom). Add the new tests here, NOT a `.svelte` component test.

### Tasks

Run all tests from web/: `cd web` then `node --test 'tests/unit/**/*.test.mjs'`. (Per package.json L14 `test:unit`.) Commit message convention ends with the Co-Authored-By trailer.

TASK 1 — `:` colon surface resolves peek/graph from the registry (RED first)
  Files: web/tests/unit/command-registry.test.mjs (add tests), then web/src/lib/v4/commands.ts (+ Phase-A surfaces if needed).
  1a. Write failing test. Append to command-registry.test.mjs (uses the existing `commandRegistry`, `_reset()` idiom; ALSO import `availableOn`-related export — confirm Phase A exports it as `commandRegistry.availableOn`):
      ```js
      test("availableOn('colon') includes peek and graph verbs", () => {
        commandRegistry._reset();
        // register two colon-surfaced verbs + one palette-only command
        commandRegistry.register({
          id: "peek", verb: "peek", label: "Toggle Peek popover", glyph: "i",
          category: "tile", chord: ["p"], shortcut: "⌘I",
          keywords: ["peek"], run: () => {},
        });
        commandRegistry.register({
          id: "fullscreen-graph", verb: "graph", label: "Fullscreen graph", glyph: "✦",
          category: "navigate", chord: ["g", "g"], shortcut: "⌘G",
          keywords: ["graph"], run: () => {},
        });
        const colon = commandRegistry.availableOn("colon", {});
        const verbs = colon.map((c) => c.verb);
        assert.ok(verbs.includes("peek"), "colon surface includes :peek from registry");
        assert.ok(verbs.includes("graph"), "colon surface includes :graph from registry");
        // resolution path the component uses (findByVerb) hits the registry command
        assert.equal(commandRegistry.findByVerb("peek")?.id, "peek");
        assert.equal(commandRegistry.findByVerb("graph")?.id, "fullscreen-graph");
      });
      ```
  1b. Run it — expect RED. `cd web; node --test 'tests/unit/**/*.test.mjs'`. If Phase A's `availableOn` already derives `'colon'` for chord-carrying commands, this test may PASS immediately on registration alone — in that case it's a regression guard (still keep it). If it FAILS because the derived default excludes these from `'colon'`, that's the signal to add explicit `surfaces` in commands.ts.
  1c. Minimal impl. In web/src/lib/v4/commands.ts, only if 1b is RED: add `surfaces: new Set(["colon", "palette", "leader"])` (use the exact `Surface` literals from Phase A) to the `peek` (~L633) and `fullscreen-graph` (~L644) command objects. Match the back-compat default shape Phase A defined — read Phase A's derivation in command-registry.svelte.ts FIRST and only override what the default gets wrong.
  1d. Run — expect GREEN. Same command.
  1e. Commit: `fix(colon): peek/graph colon-surfaced so : resolves them from registry`.

TASK 2 — a palette-only command is NOT a colon verb (RED first)
  Files: web/tests/unit/command-registry.test.mjs.
  2a. Write failing test (append):
      ```js
      test("availableOn('colon') excludes a palette-only command", () => {
        commandRegistry._reset();
        commandRegistry.register({
          id: "palette-only", verb: "palonly", label: "Palette Only", glyph: "x",
          category: "navigate",
          surfaces: new Set(["palette"]),
          keywords: [], run: () => {},
        });
        const colonVerbs = commandRegistry.availableOn("colon", {}).map((c) => c.verb);
        assert.ok(!colonVerbs.includes("palonly"), "palette-only command is not a colon verb");
        // but it IS available on palette
        const palVerbs = commandRegistry.availableOn("palette", {}).map((c) => c.verb);
        assert.ok(palVerbs.includes("palonly"));
      });
      ```
  2b. Run — expect GREEN if Phase A's explicit-surfaces path is correct (`surfaces` present ⇒ authoritative; `'colon'` absent ⇒ excluded). This is a pure Phase-A-contract guard. If RED, the bug is in Phase A's `availableOn` (explicit set not honored) — STOP and flag, don't patch Phase A from here.
  2c. Commit (fold into Task 1's commit if 2b needed no impl change): include both tests in one commit.

TASK 3 — strip the builtins + switch ColonCommandLine to availableOn('colon') (real edits)
  Files: web/src/lib/components/shell/ColonCommandLine.svelte.
  3a. Edit L51: `commandRegistry.available(ctx)` → `commandRegistry.availableOn('colon', ctx)`.
  3b. Delete the `BUILTINS` const (L54-57) and the `for (const b of BUILTINS)` block inside `suggestions` (L71-80).
  3c. Delete the `if (verb === "peek")` and `if (verb === "graph")` branches in `runVerb` (L122-131) — leave the `findCommandByVerb`-based body (L132+).
  3d. Edit L162: drop `|| typedVerb === "peek" || typedVerb === "graph"` → `if (exact) {`.
  3e. Remove unused imports `openPeek`/`openFullscreenGraph` (L25-26) — grep the file to confirm no other use before deleting.
  3f. Verify: `cd web; node --test 'tests/unit/**/*.test.mjs'` still green (component isn't unit-tested, but registry tests must stay green and the build must compile). Then `cd web; npm run build` (or the repo's svelte-check) to confirm no dangling-import / type errors from the removed code.
  3g. Commit: `refactor(colon): narrow : to availableOn('colon'), fold peek/graph builtins`.

TASK 4 (browser-QA, NOT a unit test) — confirm `:` shows verbs only, no note interleave, peek/graph still work
  This is UI behavior → Chrome DevTools MCP, the repo's pattern. Named manual step:
  - Launch the web app (the repo's dev server) and open the /g (Graphite) route where `<ColonCommandLine>` mounts (GraphiteShell.svelte:315).
  - Press `:` to open the ex-line. Type nothing → the suggestion list shows ONLY registry verbs (incl. `:peek`, `:graph`); assert NO note/page titles appear (contrast GrCommandPalette which interleaves `NoteRow`s — `:` must not).
  - Type `peek` → run; assert the Peek popover toggles (now via the registry `togglePeek`, not the old `openPeek`). Type `graph` → run; assert fullscreen graph opens.
  - Type a known palette-only verb (e.g. `station` if it's palette-only, or the test's `palonly` analog in real data) → assert it does NOT appear as a `:` suggestion.
  - Screenshot the suggestion list as evidence. Mark the Plan checkbox `[?] awaiting human verify` per AGENTS.md phase-loop if a human sign-off is required; otherwise the DevTools screenshot + assertions self-certify.

### Risks / ordering / verify-against-live

ORDERING / DEPENDENCY:
  - Hard-depends on Phase A. `commandRegistry.availableOn(surface, ctx)`, `type Surface`, and `Command.surfaces?: ReadonlySet<Surface>` MUST already exist + be exported. If Phase A is not merged, Task 1/3 cannot compile — STOP and flag rather than re-implementing Phase A here. The Task 2 test doubles as a Phase-A-contract regression guard (explicit `surfaces` set is authoritative).
  - Verify against live code: confirm the exact export shape of `availableOn` (method on `commandRegistry` vs free function). The spec says "add `availableOn(surface, ctx)`"; read Phase A's actual signature before writing the test import.

BEHAVIORAL CHANGE (intended, but flag for the implementer to confirm):
  - The deleted `peek` builtin called `openPeek('backlinks-of-page')` (ColonCommandLine L123). The registry `peek` command calls `togglePeek(getFocusedLeafId())` (commands.ts L642) — a TOGGLE, different store fn, different default behavior. After the fold, `:peek` runs the registry command (toggle), NOT open-with-default-arg. This is the spec's "fold back into the registry" intent, but it changes `:peek`'s runtime behavior and its label ("Open Peek popover" → "Toggle Peek popover"). Confirm this is acceptable in Task 4 browser-QA; it is the only non-mechanical behavior delta.
  - The old builtin `runVerb('peek', arg)` accepted an arg (`openPeek(arg ?? 'backlinks-of-page')`). The registry `peek.run` ignores its arg. So `:peek <something>` no longer parameterizes peek. Likely fine (no UI advertised it) but note it.

DERIVED-VS-EXPLICIT SURFACES:
  - Whether Task 1c (adding explicit `surfaces`) is even needed depends entirely on Phase A's back-compat derivation. Per the SHARED CONTRACT, a chord-carrying command derives `'leader'` and "everything → 'palette'/'colon' as today". Both peek and graph carry chords AND have verbs, so the DERIVED default very likely already includes `'colon'` — meaning Task 1 may be a pure regression guard with ZERO commands.ts edit. Read the derivation first; do not add redundant explicit sets that diverge from the default and accidentally DROP a surface (e.g. forgetting `'leader'`).

NOTE INTERLEAVE:
  - ColonCommandLine NEVER interleaved notes to begin with (only GrCommandPalette does, via `notesQuery` + `NoteRow`). So "confirm no note interleave on `:`" is a guard that the narrowing didn't accidentally add one — it's verified by the absence of any notes query in ColonCommandLine (there is none) + the Task 4 visual check. No code change makes notes appear; this is a confirm-only acceptance, not a fix.

UNUSED-IMPORT / DEAD-CODE:
  - After removing the peek/graph branches, `openPeek` and `openFullscreenGraph` imports (L25-26) become dead. svelte-check / the build will error or warn on unused imports — remove them (Task 3e) but grep first; `findCommandByVerb` and `matchesV4Command` (L24) stay in use.

---
