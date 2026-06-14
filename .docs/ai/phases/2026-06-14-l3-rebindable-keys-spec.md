# L3 — User-Rebindable Keybindings

Author: Opus (Lead), 2026-06-14. Scoped via an Understand→Design→Synthesize workflow (7 agents, tree-verified). Builds on the now-complete unified `commandRegistry` (slash + colon + ⌘K + leader on one spine). Decisions made on recommended defaults ("dive in").

## Decision

**Registry-native resolution + a sparse per-command override** — NOT a full keymap-document.

The override store is `Record<Command.id, BindingOverride>`; the registry resolves the *effective* binding = override ?? compiled-in `Command` field. This wins because **4 of 5 surfaces already read registry fields** (leader tree, slash, palette badge, the `:keymap`/`findConflicts` report) — so rekeying `buildKeymapIndex` on effective bindings gives them rebinds *for free*. The keyboard-first payoff: **rebind once, takes effect everywhere.** We keep a per-channel tri-state but drop `version`/`context`/export-import (deferred; the flat shape upgrades to them by addition, not rewrite).

## Storage format

Client-local `localStorage['tesela:keybindings']` (JSON), a Svelte-5 `$state` singleton (`web/src/lib/stores/keybindings.svelte.ts`, mirroring `theme.svelte.ts`/`preferences.svelte.ts`).

```ts
type BindingOverride = { shortcut?: string | null; chord?: string[] | null };
// store: map: Record<string /*Command.id*/, BindingOverride>
```

**Tri-state per channel:** key ABSENT = inherit the compiled-in default; `null` = explicitly unbound (suppress without reassigning); a value = rebound. Reset-per-command = `delete map[id]`; reset-all = clear + `localStorage.removeItem`. Shortcut values are display-glyph strings byte-identical to `Command.shortcut` (`"⌘⇧K"`, `"⌘\\"`, `"⌘-"`) so the existing `buildKeymapIndex`/`findConflicts` validate them with zero change. Reassign the whole `map` on every mutation (`$state` proxy fires).

## Runtime resolution

Pure exports in `command-registry.svelte.ts`: `effectiveShortcut(cmd, overrides)`, `effectiveChord(cmd, overrides)`, `resolveShortcut(e, ctx, overrides)`, `checkRebind(...)`; a new `shortcut-glyph.ts` with `eventToShortcutGlyph(e)` (modifier order `⌃⌥⌘⇧`, single chars uppercased, `\`/`-` literal). Dispatch sites that must consult them:
- **`GraphiteShell.onKey`** ⌘-combo block (`:252-292`) → inverted to `resolveShortcut(e, ctx, keybindings.map)` — **reads the map at call-time per keystroke** (the `onMount` listener is non-reactive).
- **`getLeaderTree`/`buildChordTree`** (`leader-tree.svelte.ts:55-109`) → `effectiveChord`.
- **`buildKeymapIndex`** (optional `overrides` param) → transitively flows to `findConflicts`/`formatKeymap`/settings.
- **Palette/colon badges** (`GrCommandPalette.svelte:139`, `ColonCommandLine.svelte:75`) → `effectiveShortcut` for display.

NOT consulted (correct): `findByVerb` (colon verbs are typed names, not keys); `getSlashTree` slashKey (separate `assignChords` system — deferred).

## Conflict UX

Reuse `findConflicts` + `BROWSER_RESERVED_KEYS` verbatim. A pure `checkRebind` validator + 3-tier policy:
- **Browser-reserved (⌘T/⌘W/⌘N/⌘Q/⌘R)** → HARD BLOCK (the only true block — `preventDefault` provably can't intercept these, so the binding would be dead).
- **Collision with another command** → SOFT WARN + "Rebind anyway" (last-writer-wins; the live `⚠` from `findConflicts` self-documents which command is live).
- **Self / none** → silent ok.

No runtime registration is ever blocked — conflict detection stays a UI-time concern, exactly as `formatKeymap` is today.

## Settings UI

Replace the hardcoded list at `settings/general/+page.svelte:145-179` (a `{#each [["⌘K",…]]}` literal) with a registry-driven `<section>`, same `<h2>`/`<kbd>` styling, in the General page (no new tab in v1). Rows = `commandRegistry.all().filter(c => c.shortcut || c.chord?.length)`: label · effective badge · per-row Rebind (one-shot `onkeydown` → `eventToShortcutGlyph` → `checkRebind` → `setShortcut`/null-on-Backspace/Esc-cancel) · per-row Reset (when overridden) · live `⚠`. One "Reset all". Shortcut capture v1; chord-capture one step later (same row infra).

## Scope (IN / OUT / DEFERRED)

- **IN:** ⌘-shortcuts + leader chords (the two pure-key, registry-backed channels).
- **OUT:** vim normal-mode keys (owned by `@replit/codemirror-vim` singleton + the editor CM6 keymap — a separate Compartment subsystem); colon verbs (typed names, not keys — rebinding = aliasing, a different feature); leader/colon TRIGGER keys (Space/`:`/Ctrl-W stay hardcoded in `onKey` — overlay entry with INSERT-mode + cm-vim `<Space>` entanglement; the trees BEHIND them are fully reboundable).
- **DEFERRED:** slash keys (`assignChords`/`chord_key` system, not `Command.slashKey`); per-context (editor vs global) bindings (the flat `Record<id, BindingOverride>` extends to `Record<id, {global?, editor?}>` by addition); device-sync (no prefs-sync infra; keymaps are device-class-specific — a later Loro-prefs-doc milestone).

## Decisions on open questions (defaults)

1. **Capture UX** — instant capture-on-keydown + Esc-cancel + a visible "press keys…" state (fastest; inline conflict/reserved warning before persist = no footgun).
2. **Vim keys reboundable** — NO (separate Compartment subsystem; a later "Editor keymap" milestone).
3. **Device-sync** — client-local localStorage only (no sync infra; keymaps are device-class-specific).
4. **Collision policy** — warn-and-allow (last-writer-wins + live `⚠`) for command-vs-command; hard-block only browser-reserved.

## Sub-items (ordered)

- [~] **KB1** (senior·M) — override store + `eventToShortcutGlyph` + `effectiveShortcut`/`effectiveChord`/`resolveShortcut`/`checkRebind` + `buildKeymapIndex(overrides)`. NO dispatch wired. ⚠ Fix the `scratch` outlier (`commands.ts:527` has `shortcut:"Space n s"` — a chord in a shortcut field) so the round-trip test is exhaustive. **Dispatched wave11 (Sonnet vs Qwen head-to-head).** Verify: `pnpm --dir web check && node --test web/tests/unit/{shortcut-glyph,keybindings,command-registry}.test.mjs`.
- [ ] **KB2** (senior·L, **highest risk**) — invert the `GraphiteShell.onKey` ⌘-ladder (`:252-292`) into `resolveShortcut`. **PRE-REQ: reconcile the divergent `run`s FIRST** — `peek` (`commands.ts:633`, ladder *toggles* `togglePeek(focusedLeaf)` vs run *opens* `openPeek('backlinks-of-page')`) and `command-station` (`:655`, ladder passes `priorPaneId: focusedLeaf` vs run passes `undefined`). Make each `run` shell-context-complete behind a verified per-command allowlist; leave entry keys (Space/`:`/Ctrl-W) hardcoded. Read `keybindings.map` at call-time (listener is non-reactive). Verify: `pnpm --dir web check` + Opus DevTools QA (each ⌘-combo byte-identical, then rebind takes effect).
- [ ] **KB3** (senior·M) — `effectiveChord` into leader tree + effective badges into palette/colon. Verify: check + DevTools (Space→g→d default, then setChord rebinds).
- [ ] **KB4** (senior·M) — settings UI (registry-driven rebindable section). Verify: check + DevTools (rebind persists, ⌘W refused, collision ⚠, reset).

## Risks

1. **KB2 ladder-`run` divergence** (load-bearing) — a blind swap silently changes ⌘I/⌘K semantics with no error. Mitigation: reconcile each divergent `run` BEFORE deleting its branch; per-command allowlist for incremental landing.
2. **Event→glyph bijection** — must round-trip every hand-typed `Command.shortcut` or a rebind silently no-ops. Mitigation: KB1's exhaustive round-trip test lands first; fix the `scratch` outlier.
3. **Non-reactive listener staleness** — read `keybindings.map` at call-time, not a closure snapshot.
4. **Scope creep toward the full keymap-document** — explicitly cut; flat override is a strict subset.
5. **Chord-capture UX** is fiddlier than single-shortcut — KB4 ships shortcut capture first.
