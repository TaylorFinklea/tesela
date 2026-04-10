# Tesela Roadmap

## What Tesela Is

Keyboard-first note-taking system (org-mode successor). Rust backend + Next.js web frontend. Taylor's daily-driver tool — reliability matters more than features.

**Core principle:** Database-first, files are export format. Everything is a page.

**Design quality bar:** Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode default.

## Product Vision

Tesela is NOT just an outliner. The long-term vision is a personal knowledge operating system with:

1. **Block outliner with Vim mode** — Zed-quality keybindings, per-block editing, block drill-in
2. **Command palette (⌘K)** — Alfred/Raycast-style universal launcher: search pages, run commands, create notes, navigate
3. **Slash commands (/)** — in-block quick actions: change block type, insert template, add property, convert to task
4. **Space/Leader commands** — Neovim which-key-style hierarchical command menu from Normal mode: `Space f` → file commands, `Space s` → search, `Space g` → graph
5. **Anytype-style type system** — types, relations, and properties are all pages. Tags are classes. Properties are global entities. Blocks inherit property schemas from their tags. Table/kanban/list views per type.
6. **Sidebar + right panel** — Logseq DB layout: left sidebar (pages, recents, favorites, graph, tiles), right sidebar (backlinks, forward links, properties, pinned pages)
7. **Graph view** — force-directed note relationship graph with click-to-navigate
8. **Daily notes timeline** — scrollable tiles view with inline editing
9. **Search** — full-text search with highlighting, match counts, live results

## Architecture

- **Rust workspace** (`crates/`): tesela-core, tesela-cli, tesela-tui, tesela-mcp, tesela-server, tesela-plugins
- **Web app** (`web/`): Next.js 16 App Router + React + TypeScript + CodeMirror 6 + `@replit/codemirror-vim` + shadcn/ui + Tailwind + TanStack Query + Zustand + cmdk + Lucide
- **Type system**: Tags, Properties, and Values are pages with YAML frontmatter (Logseq DB + AnyType hybrid — see `memory/project_property_system_vision.md` for deep architecture)

## Rust Backend — stable, not blocked

The server + core library are mature. No immediate feature work needed beyond backlog items.

- Block outliner data model, wiki-link + tag + property parsing
- SQLite/FTS5 indexer with incremental reindex
- REST + WebSocket server (`tesela-server`) with ~95% API coverage
- MCP server for AI integration
- CLI, TUI, plugin system (Lua), backup/restore, LogSeq importer
- Type registry with Tag/Property/Value pages and inheritance

---

## Web Client — Phases

### Phase 1: Core Outliner (CURRENT)

Get to daily-driver outliner with Vim. This is the minimum before Taylor can use it.

#### M0 — Scaffold & Connect ✓ (2026-04-09)
- [x] ts-rs type bridge (11 Rust types → TS)
- [x] Next.js 16 + shadcn/ui + CM6 + TanStack Query scaffold
- [x] api-client.ts + ws-client.ts (exponential backoff reconnect)
- [x] Boot screen with live WS status, notes list

#### M1 — Block Outliner ✓ (2026-04-10)
- [x] Block parser ported to TypeScript
- [x] Indented block rendering with bullet dots
- [x] Tag pills (#Task, #urgent) linking to tag pages
- [x] Wiki-links ([[Person]]) as clickable navigation
- [x] Property display (status:: doing) below blocks
- [x] Click-to-edit with inline CM6, debounced PUT save
- [ ] Arrow-key navigation between blocks (up/down moves focus)
- [ ] Escape to exit block editor back to reading mode

#### M2 — Block Operations
- [ ] Enter creates a new sibling block below
- [ ] Tab indents block (becomes child of previous sibling)
- [ ] Shift-Tab outdents block
- [ ] Backspace at start of empty block deletes it
- [ ] Backspace at start of non-empty block merges with previous
- [ ] Copy/paste blocks (preserving hierarchy)

#### M3 — Vim Engine + Command Palette
- [ ] Vim mode toggle (start in Normal mode)
- [ ] Normal mode: `h`/`j`/`k`/`l`, `w`/`b`/`e`, `gg`/`G`, `0`/`$`
- [ ] Operators: `d`/`c`/`y` with motions and text objects
- [ ] Visual mode (character + line)
- [ ] Dot-repeat, count prefix
- [ ] `/` search with `n`/`N` navigation and highlighting
- [ ] Cross-block `j`/`k` (exit current block's CM6, focus prev/next block)
- [ ] **⌘K Command Palette** (cmdk) — search pages, run commands, create notes
- [ ] Command palette actions: "New note", "Go to daily", "Search all notes", "Toggle sidebar"

### Phase 2: Navigation & Views

#### M4 — Sidebar
- [ ] Left sidebar: Pages list, Recents, Favorites, Graph nav
- [ ] Sidebar search/filter
- [ ] Sidebar collapse toggle
- [ ] Favorite/unfavorite pages

#### M5 — Tag Page Views
- [ ] Tag pages show table of all blocks/notes with that tag
- [ ] Table view with sortable columns based on tag properties
- [ ] Filter by property values
- [ ] Kanban view (group by a select property like Status)
- [ ] Property editor on tag pages (add/remove/reorder tag_properties)

#### M6 — Right Sidebar
- [ ] Backlinks panel (grouped by source page with context)
- [ ] Forward links panel
- [ ] Properties panel for focused block
- [ ] Pin any page to right sidebar (split view)
- [ ] Table of contents / page structure

#### M7 — Daily Notes & Tiles
- [ ] Daily notes timeline (virtualized scrolling)
- [ ] Click to open, inline editing in timeline
- [ ] "Go to today" shortcut
- [ ] Daily note auto-creation

#### M8 — Graph & Search
- [ ] Force-directed graph view (Cytoscape.js or similar)
- [ ] Click node → navigate to note
- [ ] Graph filters (by tag, by connection depth)
- [ ] Global search modal with live results, highlighting, match counts
- [ ] Search result snippets with context

### Phase 3: Power Features

#### M9 — Slash Commands
- [ ] Type `/` at start of block → command menu appears
- [ ] `/task` — convert block to Task (add #Task tag + properties)
- [ ] `/heading` — convert to heading block
- [ ] `/todo`, `/doing`, `/done` — set task status
- [ ] `/template` — insert a template (from template pages)
- [ ] `/property` — add an inline property to this block
- [ ] `/date` — insert date picker
- [ ] `/link` — search and insert wiki-link
- [ ] Extensible — new slash commands via config or plugin pages

#### M10 — Space/Leader Commands
- [ ] In Normal mode, press Space → which-key-style popup appears
- [ ] `Space f` → file: new, open, recent, favorites
- [ ] `Space s` → search: full-text, tags, properties, backlinks
- [ ] `Space g` → graph: open graph, focus current page in graph
- [ ] `Space b` → buffer: switch between open pages, close page
- [ ] `Space t` → tasks: list all tasks, filter by status
- [ ] `Space d` → daily: go to today, yesterday, tomorrow
- [ ] `Space p` → properties: edit current block/page properties
- [ ] Hierarchical — each category opens a sub-menu with more options
- [ ] Discoverable — key hints shown in popup, searchable

#### M11 — Anytype-Style Types & Relations
- [ ] **Property pages** — Status, Priority, Deadline are pages with `type: "Property"`, `value_type`, `choices`
- [ ] **Global property registry** — when adding a property to a tag, search existing property pages first
- [ ] **Tag inheritance** — `extends` chain (Task → Root Tag), child inherits parent's tag_properties
- [ ] **Property configuration UI** — value type, default, choices, hide-by-default, position
- [ ] **Property value types** — Text, Number, Date, DateTime, Checkbox, Select, URL, Node (link to another page)
- [ ] **Node references** — property value links to another page (bidirectional)
- [ ] **Type creation UI** — name, icon, properties, default layout (page/list/table/kanban)
- [ ] **Queries / Sets** — saved filters by type + property values, displayed as table/list/kanban
- [ ] **Collections** — manual groupings of pages (complement to query-based Sets)

### Phase 4: Polish & Distribution

#### M12 — Theme & Settings
- [ ] Settings page (theme, accent color, server URL, Vim toggle)
- [ ] Dark/light/auto theme
- [ ] Accent color picker
- [ ] Empty/loading/error states for every view
- [ ] **Linear/Logseq/Zed craft bar** — every screen held to this design standard

#### M13 — (Optional) Tauri Wrap
- [ ] Tauri shell serving `web/out/`
- [ ] Menu bar, global hotkeys
- [ ] `tesela web` CLI subcommand
- [ ] System tray with quick capture

**Deferred past Phase 4:** Whiteboards (Excalidraw), long-form prose mode, mobile/iOS, multi-device sync (CRDTs), App Store distribution, plugin marketplace, collaborative editing.

---

## Backlog (parallel, tiered by model capability)

<!-- tier3_owner: claude -->

### Haiku (mechanical, no judgment)

- [ ] Replace one-off `regex::Regex::new(r"#[...]")` in `crates/tesela-server/src/routes/notes.rs:179` with cached `INLINE_TAG_RE`
- [ ] Replace `std::env::current_dir().unwrap()` in `crates/tesela-cli/src/main.rs:196` with `?` + `.context()`
- [ ] Replace 2 `plist_file.to_str().unwrap()` calls in `crates/tesela-cli/src/main.rs:666,690` with `.context()`
- [ ] Replace 3 `serde_json::to_string_pretty(&results).unwrap()` calls in `crates/tesela-mcp/src/tools.rs:150,236,260` with `.expect("reason")`
- [ ] Annotate 2 regex-capture unwraps in `crates/tesela-cli/src/import_logseq.rs:202,244` with `.expect("reason")`
- [ ] Annotate `cap.get(0).unwrap()` in `crates/tesela-core/src/link.rs:38` with `.expect("reason")`
- [ ] Extract hardcoded server bind address `"127.0.0.1:7474"` into a named constant
- [ ] Extract hardcoded backup-retention magic numbers into named constants

### Sonnet (some architectural judgment)

- [ ] Split `crates/tesela-core/src/db/sqlite.rs` (1126 lines) into db/migrations.rs, db/search.rs, db/links.rs, db/types.rs
- [ ] Split `crates/tesela-cli/src/main.rs` (826 lines) into `src/commands/` submodule
- [ ] Extract duplicated backup logic into shared `tesela_core::backup` module

### Opus (design skill, cross-cutting)

- [ ] API endpoint integration tests (server routes)
- [ ] New server endpoints needed for web client: `GET /notes/:id/blocks`, `POST /notes/:id/blocks` (block-level CRUD)

---

## Constraints

- Design quality bar: Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode default
- No business logic in the web client — only in `tesela-core` traits
- Database-first; files are export format
- Everything is a page — types, properties, tags are all note files
- Icons: Lucide in web client
- Command palette is the primary discovery surface for commands

## Non-Goals (for now)

- iOS/iPadOS app
- Multi-device sync (CRDTs)
- App Store distribution
- Plugin marketplace
- Collaborative editing
- Whiteboards / infinite canvas
