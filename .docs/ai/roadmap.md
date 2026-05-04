# Tesela Roadmap

## Now / Next / Later

Active items. Trim as completed.

### Now
The web client is feature-complete through Phase 2 (Navigation & Discovery): outliner, Vim, slash commands, leader menu, sidebar, command palette, graph, timeline, tag tables, settings, themes, favorites, search highlighting, tag-table filtering, right-sidebar properties, graph filters. Pick a Phase 3 candidate based on daily-driver need; otherwise drain the Backlog.

### Next — Phase 3 candidates (pick one)

**3A: Type System Depth (Anytype vision)**
- Kanban view on tag pages (group blocks by a select property like Status).
- Queries / Sets — saved filters by type + property values, displayed as table/list/kanban.
- Collections — manual groupings of pages.
- Node references — property value links to another page (bidirectional).
- Tag inheritance — `extends` chain (Task → Root Tag), child inherits parent properties.
- Global property registry — search existing property pages when adding to a tag.

**3B: Editor Power Features**
- Visual mode in Vim (character + line selection).
- Block merge on Backspace at start of non-empty block.
- Multi-block selection and operations.
- `/template` slash command — insert from template pages.
- `/date` slash command — date picker UI.
- Block drill-in — focus on a single block and its children.

**3C: Polish & Edge Cases**
- Empty/loading/error states for every view (audit).
- Keyboard shortcuts for favorites (e.g., `f` to toggle).
- Graph: click node → navigate, drag to reposition.
- Right sidebar: inline property editing (not just display).
- Breadcrumb improvements — clickable path segments.
- Mobile/responsive layout considerations.

### Later
Rust backlog (parallel work) lives in the Backlog section below — Mechanical and Architectural items are safe for parallel work.

### When Picking Up Work
1. Read `.docs/ai/current-state.md` and the section above.
2. `git log --oneline -10` to see recent changes.
3. Start `tesela-server`: `cargo run -p tesela-server`.
4. Start web dev server: `pnpm --dir web dev`.
5. Pick a phase or ask Taylor what to prioritize.

## What Tesela Is

Keyboard-first note-taking system (org-mode successor). Rust backend + SvelteKit web frontend. Taylor's daily-driver tool — reliability matters more than features.

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
- **Web app** (`web/`): SvelteKit 2 + Svelte 5 (runes) + TypeScript + CodeMirror 6 + `@replit/codemirror-vim` + Tailwind v4 + TanStack Query (@tanstack/svelte-query) + Tabler Icons
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

### Phase 1: Core Outliner ✓

Daily-driver outliner with Vim. Migrated from Next.js/React to SvelteKit/Svelte 5 on 2026-04-10.

#### M0–M2 — Core Outliner ✓ (2026-04-09 → 2026-04-11)
- [x] SvelteKit 2 + Svelte 5 scaffold (migrated from Next.js)
- [x] Block parser, always-editable CM6, block operations
- [x] Vim mode + block operators (dd, yy, p, o, O, >>, <<)
- [x] ⌘K Raycast-style command palette with sections, search highlighting
- [x] Slash commands (/task, /todo, /doing, /done, /heading, /property, /link, /date)
- [x] Space leader menu (hierarchical, which-key style)
- [x] Inline autocomplete for #tags and [[wiki-links]]

### Phase 2: Navigation & Views ✓ (2026-04-12 → 2026-04-14)

- [x] Sidebar: Today/Timeline/Graph/Pages nav, Favorites, Recents, collapse toggle
- [x] Tag page table views: sortable columns, per-column filters, inline property editing
- [x] Right sidebar: properties panel (tags, type, custom), backlinks, forward links
- [x] Logseq-style journal timeline with inline editable blocks per day
- [x] Canvas force-directed graph with tag filters, depth slider, theme-aware colors
- [x] Full-text search with bold match highlighting in command palette
- [x] Favorites system (localStorage, star toggle, sidebar section, command palette)
- [x] Settings page (themes, font size, Vim toggle, server URL, shortcuts reference)
- [x] 6 themes: Day, Evening, Woven, Tile Grid, Depth Layers, Neon Glow

### Phase 9: v9 Redesign (IN PROGRESS)

Full redesign vision: `.docs/ai/phases/v9-redesign-vision.md`. Tokyo Night replaces all 6 themes; left+right sidebars become rail+bottom-drawer; rail is the surface for the planned Queries/Sets feature.

#### Phase 9.0 — Columns Shell + Tokyo Night ✓
- [x] 4-region grid (rail / middle / focus / bottom drawer + crumb + status) in `+layout.svelte`
- [x] Tokyo Night palette as the only theme; legacy CSS-var aliases route every component automatically
- [x] JetBrains Mono + Inter Tight typography; Newsreader/Source Sans 3 retired
- [x] `Ctrl+w h/j/k/l` traverses rail / middle / focus / bottom; `1` and `b` toggle bottom drawer
- [x] Right sidebar contents (backlinks, properties) ported into bottom drawer tabs; History + Linked Tasks stubbed
- [x] `themes.ts` deleted; theme picker removed from settings; "Toggle Theme" command removed from ⌘K

#### Phase 9.1 — Saved-Query Widgets (Queries/Sets) ✓
- [x] DSL parser in `tesela-core::query` (Rust + ts-rs export → web/src/lib/types) — supports `kind:` `tag:` `status:` `has:` properties, comparison ops, negation
- [x] `SearchIndex::execute_query` trait method + `SqliteIndex` impl (block-kind via broad SQL prefilter + in-memory refine; page-kind via frontmatter parse)
- [x] `POST /search/query` endpoint in tesela-server
- [x] 9 system widgets (Today, Pages, Tasks, Projects, People, Inbox, Calendar, Recent, Pinned) auto-created on app load
- [x] User-authored saved queries appear as widgets in the rail's Saved section (rail consumes `note_type: Query` notes via `widget-registry`)
- [x] Middle column renders grouped query results with parent breadcrumbs and kind badges (`.kind-badge.kind-task` / `kind-project` / etc.)
- [x] ⌘K "New Query" command + `/widget` slash command + rail footer button
- [→] Block kind glyphs (TASK/PROJECT) in focus pane — deferred to 9.4 (would require restyling cm-decorations away from current "tags hidden" model)

#### Phase 9.2 — Calendar + Inbox Widgets ✓
- [x] Mini calendar pinned in rail (`MiniCalendar.svelte`); per-day rose/teal/amber markers from `GET /calendar/marks`; click-to-navigate-to-daily-note (auto-creates if missing); month nav
- [x] "Event" inferred from `scheduled::` (teal) and "task" from `deadline::` (rose) block properties; ISO date extracted from bare or wiki-wrapped (`[[YYYY-MM-DD]]`) values
- [x] Inbox widget: post-DSL filter excludes blocks from daily notes + Tag/Property/Query/Template pages via the new `page_note_type` field on `QueryItem`
- [x] Triage flow: `t/d/x` single-key handlers in middle column when widget is `inbox` — sets `status::` continuation line, PUTs note, row drops out via WS invalidation
- [x] `note_type` SQL column now populated by `upsert_note` (was previously NULL for all rows; backfilled via `cargo run -p tesela-cli reindex`)
- [→] Project attachment (`p` triage key) — deferred to 9.4

#### Phase 9.3 — History + Linked Tasks Tabs ✓
- [x] SQLite migration `003_note_versions` (note_id, version_number, content, prev_content, created_at)
- [x] `SearchIndex::record_version` / `list_versions` / `get_version` trait methods + SqliteIndex impl. Cap at 200 versions per note (prune oldest in same tx).
- [x] PUT /notes/:id writes a version row before reindex (best-effort; failure logs but doesn't fail the PUT)
- [x] GET /notes/:id/versions and /notes/:id/versions/:version_id endpoints
- [x] `has-link:<id>` DSL predicate in both Rust and TS parsers
- [x] HistoryTab.svelte — timeline list with relative time + +N/−M line counts
- [x] HistoryDiff.svelte modal — side-by-side diff using local LCS line-diff helper; Restore button issues PUT with historical content
- [x] LinkedTasksTab.svelte — reuses /search/query with `kind:block tag:Task has-link:<focused-id>`, grouped by status

#### Phase 9.4 — Polish ✓ (mostly)
- [x] Dynamic per-view keyboard hints in crumb bar — context table by route + widget id
- [x] Mini calendar keyboard nav — arrows / hjkl / PgUp / PgDn / `g t` (today) / Enter
- [x] Drag-to-rearrange widget rail — HTML5 drag-drop on rail rows; persists to `tesela:railOrder`
- [x] Block kind glyphs (TASK/PROJECT badge prefix) in focus pane — `KindBadgeWidget` decoration via new `primaryTagFacet`
- [x] Project attachment (`p` triage key in inbox) — opens `ProjectPicker` modal, sets `project::` block property
- [x] Cmd+Z bleed-through fix — when vim is enabled, document-level Cmd+Z is suppressed inside cm-editor (vim's `u` is the canonical undo)
- [x] Drawer tab badge counts — History tab shows real version count, Linked tasks shows real task count
- [x] Column-view navigation (drilling auto-creates a 2-pane split: previous on left, current on right) — shipped as Phase 9.5b

#### Phase 9.5b — Column-View Navigation ✓
- [x] Replaces the explicit `^w v` toggle from 9.5 with auto-split-on-drill. The vision (Finder column view / yazi / Larkline) is "left = where you came from, right = current."
- [x] Drill rule: source pane content → new left, target → new right, non-source pane is dropped. Every navigation drills (block drill-in, wiki-link click, ⌘K palette, rail click).
- [x] URL spec: `path = right (current); ?back=<noteId>&backBlock=<id>` for left. URL is the single source of truth — reload preserves both panes; browser back unwinds drills.
- [x] `gotoNote(target, block?)` in `active-pane-nav.svelte.ts` rewrites the URL per the drill rule and shifts active side to "right" after every drill. New helpers: `goBack()` (full-screen the left, drop right), `collapseSplit()` (drop ?back=, full-screen the right; used for kanban-mutex).
- [x] `^w v` removed. `^w q` and `^w h` both call `goBack()` (full-screen the left, drop the right) when split is shown. Mental model: left pane is "where I came from"; pressing `h`/left = go there. Esc when right pane is active + vim NORMAL also collapses split. `^w l` flips active side back to right when user has clicked into the left pane.
- [x] Default ratio is 30 (left = 30% width, right = 70%) — back-context is a condensed preview, current pane gets the bulk of the screen. Storage key bumped to `tesela:vSplitRatio:v2` so values from 9.5 (where the meaning of the number was inverted) don't carry over.
- [x] Middle column is the persistent rail-widget result list (Pages / Today / Tasks / Inbox / etc.). When the focus pane lands on a Query note, that becomes the anchored widget — drilling from there into individual pages keeps the same list visible in the middle column. New `current-rail.svelte.ts` store persists the anchor across reloads. Old "Backlinks fallback" branch in `MiddleColumn.svelte` removed (backlinks live in the bottom drawer's Backlinks tab; never duplicated in the middle column).
- [x] Centralized drill via `beforeNavigate` in `+layout.svelte`: every internal `<a href="/p/...">` link click is rewritten through `gotoNote` so the column-view split appears regardless of which component rendered the link. Programmatic gotos (gotoNote / goBack / collapseSplit) are tagged with an `isInternalNavInFlight` flag so they pass through unchanged. This means rail clicks, middle-column row clicks, in-editor wiki links, and any future link surface all drill consistently with no per-component plumbing.

#### Phase 9.5c — Drill is opt-in; middle column removed ✓
- [x] Middle column deleted entirely. The 4-region grid (rail / middle / focus / bottom) becomes a 3-region grid (rail / focus / bottom). Layout grid template updated to `232px 1fr`. `MiddleColumn.svelte` and `current-rail.svelte.ts` removed.
- [x] Query widgets render their result list inline inside the focus pane via the new `QueryWidgetView.svelte` component. Drilling from a result row calls `gotoNote()` directly. Same component is used for the back-pane (left) when the back-context note is a Query.
- [x] Drilling is now opt-in: only block drill-in, wiki-link click in NORMAL mode, and query-result row click create the column-view split. Rail clicks and ⌘K palette picks are plain SvelteKit navigations that replace the focus area full-screen. The global `beforeNavigate` interceptor from 9.5b removed; the `internalNavInFlight` flag and `isInternalNavInFlight` export remain (still used to suppress double-firing when programmatic helpers call `goto`).
- [x] `^w h/l` traversal updated: focus ↔ rail directly (no middle stop). All other chord behavior unchanged.
- [x] Reasoning: the middle column duplicated query results that the focus pane could render natively, and rail-click drilling produced an unwanted 4-pane layout (rail + middle + back-pane + current-pane). One pane in the focus region is the default; drilling adds a second.
- [x] `+page.svelte` swaps roles: path-driven content is now the **right** pane; new `?back=` query drives the **left (back-context)** pane. Save plumbing renamed `right* → back*`. Removed `initialMountChecked` + cleanup-effect race; URL is authoritative.
- [x] Wiki-link click in cm6 (`BlockEditor.svelte` mousedown handler) navigates via `gotoNote` when vim is in NORMAL mode; INSERT mode falls through so the click places the cursor.
- [x] Pane-state store slimmed: removed `openVSplit`/`closeVSplit`/`toggleVSplit`/`vSplitOpen` (URL is truth). Kept active-side + ratio. Kanban `openSplit()` calls `collapseSplit()` first when `?back=` is present.

#### Phase 9.6 — Logseq-style continuous "Dailies" journal ✓
- [x] Replaced the read-only "Today" Query widget with a "Dailies" anchor: rail label "Dailies", URL `/p/dailies`. Clicking lands on a continuous, editable multi-day journal (today on top, older days below).
- [x] Both `/p/dailies` and `/p/<YYYY-MM-DD>` (any note tagged `daily`) render the new `JournalView.svelte`. The route just differs in the anchor — page scrolls to today on `/p/dailies`, to the date on `/p/<YYYY-MM-DD>`. Mini-calendar clicks "scroll the journal to that day" exactly per the Logseq model.
- [x] `JournalView` fetches the latest 500 daily-tagged notes (single API call), sorts descending by date, renders the most recent 30 by default, expands as you scroll past the bottom (IntersectionObserver sentinel + manual "Load older entries" button). Each section is its own `BlockOutliner` with per-noteId debounced save (with `cancelAndFlush`), so edits in any day's section save to that day's file.
- [x] Today is auto-created if missing (call `getDailyNote()` on first mount); same for the anchor date if the URL named a real `YYYY-MM-DD` but the file didn't exist.
- [x] Drill-in (`?block=`) on any block opts back into the standard outliner so the user can focus on a single block; the journal scroll is the un-drilled view.
- [x] Detection in `+page.svelte`: `isDailyJournal = !drillBlockId && (noteId === "dailies" || note_type === "Daily" || tags.includes("daily"))`. Same branch added to the back-pane (column-view left).
- [x] Files: new `web/src/lib/components/JournalView.svelte`; `web/src/lib/system-widgets.ts` (today → dailies); `web/src/lib/widget-registry.svelte.ts` (system-widget-id set updated); `web/src/routes/p/[id]/+page.svelte` (isDailyJournal branch); `notes/today.md` deleted.

#### Phase 9.7 — Daily-driver polish ✓
- [x] **Drill source from left pane click**: row clicks inside the left pane were drilling with `source = right pane content` because the row's `onclick` (bubble) fired before the wrapper's `setVSplitActiveSide('left')`. Switched both pane wrappers to Svelte 5's `onclickcapture` so `vSplitActiveSide` updates BEFORE descendants receive the click. Both wrappers now also have `data-pane="left"|"right"` markers for focus targeting (see below).
- [x] **Focus-shift on drill**: `gotoNote()` now dispatches a `tesela:focus-pane` custom event (with `{ side: "right" }`) two RAFs after the goto resolves; a global handler in `+layout.svelte` finds `[data-pane="right"] .cm-editor .cm-content`, blurs any cm-editor outside the target pane, and focuses the new one. Cursor lands in the right pane after every drill.
- [x] **Cmd+Z bleed-through (vim-on)**: hardened `cmdZHandler` in `+layout.svelte` with `stopImmediatePropagation` and routed Cmd+Z to the new `tesela:outliner-undo` event (Cmd+Shift+Z → `tesela:outliner-redo`). `BlockOutliner` listens and only acts when its root contains the focused element. Cmd+Z now matches the vim `u` chord — full insert-session undo, no per-keystroke cm6 walking.
- [x] **Cancel-and-flush vs redo race**: `flushSave` (and `flushBackSave`, plus the JournalView per-note save) now does an optimistic `setQueryData({ ...note, content })` BEFORE awaiting `api.updateNote`. A WS echo from a prior PUT can no longer overwrite the cache with stale pre-undo body; the post-await `setQueryData` still wins for server-side derived fields.
- [x] **Keyboard-driven Properties tab**: `BottomDrawer` adds `selectedPropertyIndex` state and a `flatProperties` derivation (block-context list when a block is focused, page-context list otherwise). j/k cycles, Enter on a text-typed property toggles into inline edit (existing autofocus + onblur flow), Enter on select/multi-select/date/checkbox focuses the native control. Tab inside the inline input commits via `savePageProperty/saveBlockProperty(..., advance=true)` and advances to the next chip. Visual: `.pchip.selected` adds an amber inset shadow + border.
- [x] Files: `web/src/routes/p/[id]/+page.svelte` (data-pane markers, onclickcapture, optimistic flushSave); `web/src/lib/stores/active-pane-nav.svelte.ts` (focus-pane dispatch); `web/src/routes/+layout.svelte` (focusPaneHandler, harder cmdZHandler with outliner-undo dispatch); `web/src/lib/components/BlockOutliner.svelte` (rootEl bind, undo/redo event listeners); `web/src/lib/components/BottomDrawer.svelte` (keyboard nav for Properties tab, Tab commit-and-advance); `web/src/lib/components/JournalView.svelte` (optimistic pre-set in flushSave); `web/src/app.css` (`.pchip.selected` style).

### Phase 3: Power Features (paused — folded into Phase 9)

#### Anytype-Style Types & Relations
- [x] Kanban view on tag pages (group by select property like Status)
- [→] Queries / Sets — moved into Phase 9.1 (rail widgets ARE the Queries/Sets surface)
- [ ] Collections — manual page groupings
- [ ] Node references — property value links to another page (bidirectional)
- [ ] Tag inheritance — `extends` chain, child inherits parent properties
- [ ] Global property registry — search existing property pages when adding to a tag

#### Editor Power Features
- [x] Visual mode (block-level — V to enter, j/k to extend, d/y/T/J/K)
- [x] Block merge on Backspace at start of non-empty block
- [x] Multi-block selection and operations (visual delete / yank / indent / status / tag)
- [x] `/template` — insert from template pages
- [x] `/date` — date picker UI (with Todoist-style natural-language input)
- [x] Block drill-in (focus single block + children)
- [x] Block fold / collapse (Phase 3K)
- [x] Subtree-aware indent (>>, << bring children with parent)
- [x] Leader Y → OS clipboard (Phase 3K)

#### Polish
- [x] Auto-focus first block on page mount (Phase 3L)
- [x] Esc-in-Normal preserves focused block + cm-editor (Phase 3L)
- [x] 3-region splits with `Ctrl+w h/j/k/l` (left sidebar / outliner / right panel) (Phase 3L)
- [x] Modal focus restore: ⌘K / leader-menu / slash-menu close returns focus to last block (Phase 3L)
- [→] Right sidebar items obsolete — right sidebar replaced by bottom drawer in Phase 9.0
- [ ] Bottom drawer: inline keyboard property editing for Properties tab (j/k navigates, currently only mouse-clickable)
- [ ] Empty/loading/error state audit across all views
- [ ] Graph: drag nodes to reposition

### Phase 4: Distribution

#### (Optional) Tauri Wrap
- [ ] Tauri shell serving `web/out/`
- [ ] Menu bar, global hotkeys, system tray

**Deferred:** Whiteboards, long-form prose, mobile/iOS, multi-device sync (CRDTs), App Store, plugin marketplace, collaborative editing.

---

## Backlog

> Self-contained items any agent can pick up. First agent to start it executes it. Tier hints are advice, not gating.

### Mechanical (Haiku candidates)

- [ ] Replace one-off `regex::Regex::new(r"#[...]")` in `crates/tesela-server/src/routes/notes.rs:179` with cached `INLINE_TAG_RE`
- [ ] Replace `std::env::current_dir().unwrap()` in `crates/tesela-cli/src/main.rs:196` with `?` + `.context()`
- [ ] Replace 2 `plist_file.to_str().unwrap()` calls in `crates/tesela-cli/src/main.rs:666,690` with `.context()`
- [ ] Replace 3 `serde_json::to_string_pretty(&results).unwrap()` calls in `crates/tesela-mcp/src/tools.rs:150,236,260` with `.expect("reason")`
- [ ] Annotate 2 regex-capture unwraps in `crates/tesela-cli/src/import_logseq.rs:202,244` with `.expect("reason")`
- [ ] Annotate `cap.get(0).unwrap()` in `crates/tesela-core/src/link.rs:38` with `.expect("reason")`
- [ ] Extract hardcoded server bind address `"127.0.0.1:7474"` into a named constant
- [ ] Extract hardcoded backup-retention magic numbers into named constants

### Architectural (Sonnet candidates)

- [ ] Split `crates/tesela-core/src/db/sqlite.rs` (1126 lines) into db/migrations.rs, db/search.rs, db/links.rs, db/types.rs
- [ ] Split `crates/tesela-cli/src/main.rs` (826 lines) into `src/commands/` submodule
- [ ] Extract duplicated backup logic into shared `tesela_core::backup` module

### Cross-cutting (needs Opus to scope)

- [ ] API endpoint integration tests (server routes)
- [ ] New server endpoints needed for web client: `GET /notes/:id/blocks`, `POST /notes/:id/blocks` (block-level CRUD)
- [ ] Block merge with property conflict resolution: when both the merged-from and merged-into blocks have properties, show an overlay dialog letting the user choose which properties to keep (rather than naively concatenating duplicate keys)
- [x] **Outliner-level undo / redo stack** (`u` / `Ctrl+R` for structural ops) — Phase 3M. Snapshot stack in `web/src/lib/stores/outliner-history.svelte.ts`, sprinkled into every structural mutation in BlockOutliner; falls through to cm-editor history when stack empty. Cmd+Z outside vim is a follow-up.
- [x] **Vim-faithful unified `u`** — Phase 3M.1. Insert sessions are atomic: cache a snapshot on Insert-mode entry, promote on the first keystroke. `o<text><Esc>u` reverts the typing first, then on next `u` reverts the block creation — matches vim. Adds prop→cm6 sync `$effect` (with `externalSync` annotation) so undo restores propagate into editor doc.
- [ ] Cmd+Z outside vim (document-level keydown that calls the same outliner undo when not inside an editor)
- [x] Cancel in-flight saves on undo (close the residual race window where a debounced PUT from before the undo overwrites the restored state) — Phase 3M.2. AbortController plumbed through `api.updateNote`; `applySnapshot` calls `saveBlocksImmediate` which fires `onCancelAndFlush` to abort the in-flight PUT and immediately PUT the restored body.
- [x] Cm6 history coherence after outliner undo: when `applySnapshot` writes a block's body via the externalSync transaction, that transaction lands in cm6's history — so subsequent `Cmd+Z` may walk through the just-undone state. — Phase 3M.2. Added `Transaction.addToHistory.of(false)` to the prop→cm6 sync dispatch so externalSync transactions are excluded from cm6 history.
- [x] Block remount after Ctrl+R into Insert mode: when redo restores an empty newly-created block, the BlockEditor's `startininsert` heuristic (focused empty block) fires on remount, leaving vim in Insert. — Phase 3M.2. Added `restoredFocus` flag in BlockOutliner set by `applySnapshot` and cleared on user-initiated focus changes (click, navigate, new-block, empty-state click); the `startininsert` heuristic now checks `!restoredFocus`.
- [ ] **`dw` / `d$` / etc. integrate with `p` paste** (text-register fidelity). Phase 3K's `delete` operator override no-ops the register-controller side of non-linewise deletes, so deleted text isn't recoverable via `p`. Two viable approaches: (a) populate vim's default register via `vimGlobalState.registerController.pushText` (requires importing a non-public symbol from `@replit/codemirror-vim`, may break across versions), or (b) maintain our own text register alongside `blockClipboard`, and have the `pasteBlock` action prefer block clipboard, falling back to text register inserted at cursor. Pick the approach during design; option (b) is friendlier to upgrades.

---

## Constraints

- Design quality bar: Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode default
- No business logic in the web client — only in `tesela-core` traits
- Database-first; files are export format
- Everything is a page — types, properties, tags are all note files
- Icons: Tabler Icons in web client
- Command palette is the primary discovery surface for commands

## Non-Goals (for now)

- iOS/iPadOS app
- Multi-device sync (CRDTs)
- App Store distribution
- Plugin marketplace
- Collaborative editing
- Whiteboards / infinite canvas
