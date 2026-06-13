# Command Registry Foundation — Phase Spec

> Date: 2026-06-13  
> Driver: Taylor + orchestrator (Pi)  
> Implementers: `minimax-m3` (A1), `kimi-k2.7-code` (B1–B3)

## Goal

Build a single, unified command registry that palette (⌘K), leader chords (Space), slash menu (`/`), and colon ex-mode (`:`) all dispatch into. The registry is the architectural spine of the keyboard-first app — every action is named, introspectable, and eventually rebindable.

This spec covers the first four backlog items of the 2026-06-13 ralph batch. It does NOT cover Graphite parity bugs, sync/relay work, or the properties system.

## Background

The current surfaces are separate silos:

- `web/src/lib/v4/commands.ts` defines `V4Command[]` and powers the ⌘K palette.
- `web/src/lib/v5/leader-tree.svelte.ts` defines a hard-coded chord tree and powers Space leader.
- Slash commands in `web/src/lib/components/BlockEditor.svelte` are their own ad-hoc tree.
- Colon mode (`:`) in `web/src/lib/components/shell/ColonCommandLine.svelte` dispatches verbs from `findCommandByVerb` but does not share runtime state with the other surfaces.

The new `AGENTS.md` directive is: keyboard-first, command registry first. The registry must become the single source of truth.

## Design Principles

1. **One registry object.** All surfaces read from it; none hard-code command lists.
2. **Named, metadata-carrying commands.** Every command has `id`, `label`, `verb`, `category`, `run`, `keywords`, plus optional `shortcut`, `chord`, `context`, `argPrompt`.
3. **Context-aware.** Commands declare when they are available via a `when` predicate; unavailable commands are filtered from palette/leader.
4. **Introspectable.** A runtime index can list bindings, find conflicts, and answer "what's bound to this key?"
5. **Preserve existing UX.** B1–B3 must not remove shortcuts, chords, or behaviors users already rely on.
6. **No backend changes.** This is a web-client foundation project.

## Phase Breakdown

### A1 — Fix clippy errors (mechanical warm-up)

- **Scope:** Fix two clippy warnings that currently break `cargo clippy --workspace -- -D warnings`.
  - `crates/tesela-core/src/db/sqlite.rs:1129` — replace `|s| recurrence::parse(s)` with `recurrence::parse`.
  - `crates/tesela-core/src/query.rs:589` — rewrite `loop { let key = match self.bump() { ... } }` as `while let Some(Token::Word(k)) = self.bump()`.
- **Acceptance:** `cargo clippy --workspace -- -D warnings` passes.
- **Verify:** `cargo clippy --workspace -- -D warnings`
- **Model:** `minimax-m3`

### B1 — Unified registry shape + port palette/leader

- **Scope:**
  1. Create `web/src/lib/command-registry.svelte.ts` exporting a singleton `commandRegistry` and a `Command` type.
  2. `Command` type (minimum):
     ```ts
     type CommandContext = {
       route?: string;
       bufferKind?: 'page' | 'derived' | 'ambient' | null;
       vimMode?: 'normal' | 'insert' | 'visual' | null;
       focusedBlock?: { id: string; properties: Record<string, string> } | null;
       splitOpen?: boolean;
     };
     type Command = {
       id: string;
       verb?: string;
       label: string;
       glyph: string;
       category: 'pane' | 'tab' | 'tile' | 'create' | 'navigate' | 'derived' | 'ambient';
       shortcut?: string;
       chord?: string[];           // e.g. ['g','d'] for "g d"
       keywords: string[];
       argPrompt?: string;
       when?: (ctx: CommandContext) => boolean;
       run: (arg?: string) => void | Promise<void>;
     };
     ```
  3. Port every entry from `buildV4Commands()` to register itself into `commandRegistry` on first import.
  4. `GrCommandPalette` reads `commandRegistry.all()` and filters/ranks against it.
  5. `getLeaderTree()` derives its chord tree from registry entries that have `chord` metadata. Keep the existing nested-group presentation.
  6. Keep `findCommandByVerb` working by reading from the registry.
- **Files:** new `web/src/lib/command-registry.svelte.ts`; modify `web/src/lib/v4/commands.ts`, `web/src/lib/v5/leader-tree.svelte.ts`, `web/src/lib/graphite/shell/GrCommandPalette.svelte`, `web/src/lib/graphite/shell/GrLeaderOverlay.svelte`, `web/src/lib/components/shell/ColonCommandLine.svelte`.
- **Acceptance:**
  - ⌘K palette lists the same commands with the same shortcuts.
  - Space leader shows the same chord tree.
  - Colon mode can run the same verbs.
  - `pnpm --dir web check` is clean.
- **Verify:** `pnpm --dir web check` + manual palette/leader/colon QA.
- **Model:** `kimi-k2.7-code`

### B2 — Keymap introspection + conflict detection

- **Scope:**
  1. Build `buildKeymapIndex(registry): KeymapIndex` that indexes:
     - palette shortcuts (`Command.shortcut`) → commands
     - leader chords (`Command.chord`) → commands
     - slash chords (read from `BUILTIN_SLASH_CHORDS` for now)
     - browser-reserved keys (static list: ⌘T, ⌘W, ⌘⇧W, ⌘N, ⌘Q, ⌘R)
  2. Detect collisions where two commands claim the same shortcut/chord, or a command claims a browser-reserved key.
  3. Expose `:keymap` colon command that prints all commands + bindings + conflicts to the console (or a small dev overlay if trivial).
- **Files:** `web/src/lib/command-registry.svelte.ts`; new unit tests `web/tests/unit/command-registry.test.mjs`; update `ColonCommandLine.svelte`.
- **Acceptance:**
  - `:keymap` lists every registered command and its shortcut/chord.
  - Conflicts (e.g., two commands claiming `⌘\\`) are flagged.
  - Browser-reserved shortcuts are flagged.
- **Verify:** `node --test web/tests/unit/command-registry.test.mjs`; `pnpm --dir web check`; manual `:keymap` QA.
- **Model:** `kimi-k2.7-code`

### B3 — Context-aware dispatch

- **Scope:**
  1. Add `CommandContext` capture in `GraphiteShell`/`+layout`:
     - current route (`$page.route.id`)
     - focused buffer kind
     - vim mode of the focused editor
     - selected/focused block metadata
     - whether a split is open
  2. Add `when` predicates to commands that need them (e.g., `skip-occurrence` only when a focused block has `recurring`; `convert-to-tag` only when focused page is a note).
  3. `GrCommandPalette` calls `commandRegistry.available(ctx)` and hides unavailable commands.
  4. `GrLeaderOverlay` prunes branches whose children are all unavailable.
  5. Colon mode dispatches verbs from the registry; unknown verbs warn.
  6. Slash commands optionally register as registry entries ( Phase 3.6 ); for now just design the registry so slash can migrate later.
- **Files:** `web/src/lib/command-registry.svelte.ts`, `web/src/lib/graphite/shell/GraphiteShell.svelte`, `web/src/lib/graphite/shell/GrCommandPalette.svelte`, `web/src/lib/graphite/shell/GrLeaderOverlay.svelte`, `web/src/lib/components/shell/ColonCommandLine.svelte`.
- **Acceptance:**
  - Commands that don't apply in the current context are hidden from palette/leader.
  - Context changes (focus move, route change) update availability reactively.
  - No regression in existing behavior.
- **Verify:** `pnpm --dir web check` + full keyboard QA matrix (see below).
- **Model:** `kimi-k2.7-code`

## Manual QA Checklist

After each B phase, run through:

- Palette (⌘K): open, type `daily`, press Enter → jumps to daily.
- Palette quick-select (⌘1..⌘9): run a command.
- Leader (Space): open, `n` → `s` → new scratch page.
- Leader (Space): `g` → `d` → today's daily.
- Leader chord from NORMAL-mode editor: `g` → `f` → follow wiki-link.
- Colon (`:`): `:daily` → today's daily.
- Colon (`:`): `:vsplit` → splits pane.
- Slash (`/`): `/t` → task.
- Context filtering: in Inbox widget, leader should still work; palette should still list global commands.

## Out of Scope

- Rebindable keymaps (UI/settings).
- Plugin-provided commands.
- Rust-side command registry.
- Sync/relay/FFI changes.
- Graphite parity bug fixes (separate batch).

## Notes for Implementers

- Do not delete `lib/v4/commands.ts` wholesale in B1; migrate it incrementally.
- Keep `V4Command` export shape if other consumers import it; alias to `Command` if possible.
- The registry singleton can live in a Svelte 5 rune-based module or a plain TS module. Prefer plain TS unless reactivity is strictly needed.
- Tests should not require a running server; use pure unit tests for indexing/filtering.
