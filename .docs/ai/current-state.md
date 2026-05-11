# Current State

*Last updated: 2026-05-11*

## Active Branch

`main`

## Architecture at a Glance

- **Rust workspace** (`crates/`): `tesela-core`, `tesela-cli`, `tesela-tui`, `tesela-mcp`, `tesela-server`, `tesela-plugins`. Stable, well-tested.
- **Web client** (`web/`): **SvelteKit 2 + Svelte 5** (runes) + CodeMirror 6 + `@replit/codemirror-vim` + Tailwind v4 + TanStack Query (@tanstack/svelte-query v6) + Tabler Icons. SSR disabled (`export const ssr = false` in `+layout.ts`).
- TypeScript types generated from Rust via `ts-rs` — run `cargo test -p tesela-core --lib export_bindings`.
- WebSocket client with exponential backoff reconnect, wired to TanStack Query cache invalidation.

**Design quality bar:** Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode-first.

**Theme system:** "Warm Study" — Newsreader serif display + Source Sans 3 body. Day (warm cream) and Evening (warm charcoal) themes, plus 4 alternate themes (Woven, Tile Grid, Depth Layers, Neon Glow). CSS custom properties applied via inline styles on `<html>`.

## Web Client Feature State

### Core (all working)
- Block outliner with always-editable CM6 instances
- Vim mode via `@replit/codemirror-vim` with custom block operators (dd, yy, p, o, O, >>, <<)
- Cross-block j/k navigation
- Slash commands (/task, /todo, /doing, /done, /heading, /property, /link, /date)
- Space leader menu (hierarchical, Neovim which-key style)
- Inline autocomplete for #tags and [[wiki-links]]
- Debounced auto-save (500ms PUT)

### Navigation & Discovery
- **Sidebar**: Today, Timeline, Graph, Pages nav links + Favorites section + Recents section + Settings footer
- **Command palette** (⌘K): Raycast-style with sections (Recent, Actions, Create, Notes, Search), context-aware commands on note pages, keyboard shortcuts as kbd badges, Ctrl+j/k navigation, search highlighting with bold matches
- **Favorites**: localStorage-persisted, star toggle on note pages, sidebar section, command palette "Toggle Favorite" action
- **Right sidebar**: Properties panel (tags, type, custom properties) + Backlinks + Forward links

### Views
- **Note page** (Focus Mode): Large Newsreader title, tag pills, breadcrumb nav, flat block styling (no cards), star/delete buttons
- **All Notes** (/): Paginated list with timestamps and tag badges
- **Timeline** (/timeline): Logseq-style journal with inline editable BlockOutliner per daily note
- **Graph** (/graph): Canvas-based force-directed graph with tag filter dropdown, depth slider, theme-aware colors
- **Tag pages**: Table view with sortable columns, per-column text filters (AND logic), inline property editing
- **Settings**: Theme picker, font size, Vim toggle, server URL, keyboard shortcuts reference

### Layout
- h-screen viewport-pinned layout — sidebar + main content + status bar all fixed, content scrolls internally
- Status bar showing Vim mode, current note, connection status

## Build Status

- Rust: `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings` — green on 2026-05-11.
- Web: `pnpm --dir web check`, `pnpm --dir web exec tsc --noEmit`, `pnpm --dir web test:perf` — green on 2026-05-11. `pnpm --dir web run lint` has no configured script.
- Dev server: `pnpm --dir web dev` (Vite)

## Recent Session Notes

- Phase 14.2 frontend perf smoke suite is in place under `web/tests/perf/`, with a runner that creates a medium fixture mosaic, starts `tesela-server` and Vite on dynamic localhost ports, runs Playwright, and records JSONL timings.
- `tesela-fixtures` now seeds built-in Task/Status/Priority/Deadline/Scheduled pages so generated mosaics have task board property metadata before the server's initial index.
- Phase 14.3 perf workflow is in `.github/workflows/perf.yml`: nightly/main uploads Criterion baselines, PRs diff with `critcmp`, and comments only when a benchmark regression exceeds 10%.

## Blockers

None.
