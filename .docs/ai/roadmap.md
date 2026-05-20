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

**3E: Code blocks (rendering)**
Fenced ``` ```lang ... ``` ``` spans inside a block today are stored verbatim but rendered as plain paragraph text. Smallest viable change: detect the fence in BlockText (web + iOS) and render the span in a monospaced, themed code surface (no syntax highlighting yet). Tags and wikilinks inside a code fence must not be parsed. A separate **executable code blocks** track lives in `Later` — see below.

**3D: Task Management Depth (Apple Reminders / Todoist parity) — promote sooner**
The user is daily-driving Tesela for tasks; three threads need to ship soon so the system can compete with Apple Reminders / Todoist while preserving the database-first foundation. Detailed scope in **Phase 12** below.
- **Apple Reminders bidirectional sync (priority)** — lets the user lean on iOS location-based reminders, Watch, and Siri while editing in Tesela.
- **Recurring tasks & events** — rrule-subset on `deadline::` / `scheduled::`; auto-roll on completion.
- **Notifications** — desktop + push for deadlines, scheduled times, recurring rolls.
- **Task hierarchy** — subtasks, dependencies, project rollups.

### Later
Rust backlog (parallel work) lives in the Backlog section below — Mechanical and Architectural items are safe for parallel work.

**Bundled desktop app (no CLI required)** — Tesela today requires running `tesela init`, `tesela-server`, and `pnpm --dir web dev` from a terminal. That's a non-starter for actual daily-driver use. Need a single-installer macOS app (Tauri or SwiftUI shell) that bundles the Rust server binary + serves the SvelteKit web client + manages mosaic init/list/switch + import + backup + restore — all reachable from the UI. The CLI keeps existing for power users / scripting. Without this, every "trust" workflow (import a vault, take a backup, restore from a backup) requires terminal literacy. Priority: high once import + backup feel solid via CLI.

**Executable code blocks (org-babel parity)** — fenced ``` ```lang ... ``` ``` blocks become "runnable" units the user can execute in place (a la Emacs org-mode babel / Jupyter cells). Output gets pinned under the block. Languages to support first: shell, Python, JavaScript. Hard parts: sandboxing, output streaming back into the block tree, deciding which interpreter the host vs. user trusts. Design needed before implementation.

**iOS on-device Parakeet inference via FluidAudio** — Tesela's TranscriptionCatalog currently lists Parakeet variants pointed at NVIDIA's raw `.nemo` training-format files (not iOS-runnable). VoiceInk and Handy ship the same model (parakeet-tdt-0.6b-v2) via the FluidAudio Swift package, which bundles a CoreML-converted variant (~450MB on disk). Pull FluidAudio in as a dependency, swap the LocalTranscriptionEngine to dispatch to it for Parakeet IDs, flip `inferenceSupported` to true on those catalog entries.

**Mosaic discovery + server-side multi-mosaic (PRIORITY)** — iOS's "Add mosaic" form currently requires the user to paste a server URL, which is unintuitive: their Mac is already running a `tesela-server` and they expect iOS to see whatever mosaics it has. Two coupled changes needed:
  - **Server-side multi-mosaic**: `tesela-server` today is `--mosaic <one-path>`. Extend to host a list of mosaics with per-mosaic routing (path prefix or header) and a `GET /mosaics` endpoint that lists them. CLI's `init` adds to the list rather than overwriting.
  - **Discovery / pair-handoff**: the existing pair-device flow (6-char short code + QR) is the right place to advertise the host's mosaic list. When iOS finishes pairing, it imports the host's available mosaics into its local `MosaicRegistry` automatically. LAN Bonjour discovery is a separate parallel path for "find devices already on my network."

Without this, users have to spin up a second `tesela-server` on a different port to have a second mosaic — a non-starter for mobile-first daily-driving.

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

#### Phase 9.8 — Fuzzy autocomplete + recency ranking ✓
- [x] **Fuzzy match** in `AutocompleteMenu.svelte`: substring filter replaced with a tiered fuzzy scorer (prefix > word-start > substring > subsequence, with position penalties). Used for both `[[` (wiki-link) and `#` (tag) pickers since both pass through the same component. Typing `[[ph` now returns `Phase3GQA, Phase3GDQA, Phase3IQA, Phase3FQA` (prefix matches first); `[[dud` returns `dude` (substring) then `Scheduled / ScheduledItem` (subsequence).
- [x] **Recency tie-break**: same-score items sort by recency rank from `getRecents()`. When the filter is empty, the full list is sorted by recency so the user's recent notes are at the top.
- [x] **Highlight matching characters**: `highlightRuns(label, positions)` splits the label into `{ ch, match }` runs; matched chars render in `<strong class="text-primary font-semibold">` so the user sees why a result matched.
- [x] Files: new `web/src/lib/fuzzy.ts` (scorer + highlight helper); `web/src/lib/components/AutocompleteMenu.svelte` (fuzzy filter, recency sort, highlight rendering).

#### Phase 9.9 — Daily-driver keyboard ergonomics ✓
- [x] **Cross-block j/k auto-scrolls** — `BlockOutliner.handleNavigate` now does `scrollIntoView({ block: "nearest" })` on the new block after focus advances. Cursor stays in viewport during multi-block navigation.
- [x] **Ctrl+U / Ctrl+D as outliner page-jump** — vim chord registered in BlockEditor + new `pageJump` callback in `vimCtx`; jumps 10 blocks per press through the same handleNavigate path so scroll-into-view follows.
- [x] **`^w h` flips active side when split open** — first `^w h` swaps right→left, second `^w h` collapses (full-screen left). Prior behavior collapsed unconditionally. Mirrors `^w l`'s flip-to-right.
- [x] **`^w j` opens drawer if closed** — previously dropped to drawer only when it was already open. Now ensures the drawer is open and focused. Kanban-split path now requires drawer to be already open AND a kanban split AND no column-split.
- [x] **`/p/dailies` auto-focuses today** — JournalView's anchor effect, after `scrollIntoView`, also `.focus()`s the cm-content of the anchored daily so the user can type immediately.
- [x] **`gd` follows wiki-link at cursor (NORMAL mode)** — new vim action in BlockEditor scans for `[[...]]` containing the cursor position and calls `gotoNote(target)`. No-op if cursor isn't inside a wiki-link span.
- [x] **⌘K hardened** — capture-phase + `stopImmediatePropagation` so cm-editor focus on /p/dailies (or any future surface that wires Cmd+K into its keymap) can't swallow the toggle.
- [x] **⌘1-9 quick-pick + action-label hint** — palette items 1-9 get a `⌘N` badge prefix; pressing the chord runs the Nth item's action without arrow nav. Footer shows the selected item's `actionHint` ("Open page", "Run search", "Create note", etc.).
- [x] **Inline properties hidden by default with per-block toggle** — `hiddenKeysFor` now adds every `block.properties` key to the hide set unconditionally. The per-block chevron toggle (already wired) reveals via `.show-props` ancestor. New `gp` vim chord toggles the focused block's expansion state.
- [x] **Query-result row actions** — `QueryWidgetView` auto-focuses on mount so j/k just works. Block-kind rows (Task / typed) show a status glyph button; clicking the glyph (or pressing `s`) cycles status (todo → doing → done) without leaving the list. Reuses `setBlockProperty` from `triage.svelte.ts`.
- [x] Files: `web/src/lib/components/BlockOutliner.svelte` (handleNavigate scroll + handlePageJump + ontoggleprops wiring + hiddenKeysFor); `web/src/lib/components/BlockEditor.svelte` (vim Ctrl+U/D, gd, gp; vimCtx pageJump/toggleProps; module-import gotoNote); `web/src/routes/+layout.svelte` (`^w h` flip + `^w j` drawer-opens path); `web/src/lib/components/JournalView.svelte` (today auto-focus); `web/src/lib/components/CommandPalette.svelte` (capture-phase ⌘K, ⌘1-9, action hint badges, footer); `web/src/lib/components/QueryWidgetView.svelte` (auto-focus, status glyph, `s` cycle).
- [x] Deferred to Phase 10.x: query-row `/`-command picker; in-place row edit; tasks kanban + filter views rebuild; spacemacs-style space menu redesign; per-page vs. global History scope refactor.

#### Phase 9.9 follow-up — daily-driver fixes (round 2) ✓
After 9.9 dogfooding, the user surfaced 8 follow-on bugs. All fixed; #8 (gp) and #9 (⌘K + ⌘1-9) already worked.
- [x] **#6 — `gd` then Esc no longer corrupts parent display.** Drilling via `gd` and going back used to leave the right pane showing the drilled note's content under the parent's title. Root cause: BlockOutliner's body-sync `$effect` had a "preserve focus by id" early-return that always fired on noteId change (the focused block id from the previous note can never be in the new note's reparsed list). Fix: detect noteId change separately from body change and force-reset blocks/focusedIndex/history when the note changes.
- [x] **#10 — `/p/tasks` returns block-tagged results.** SQL pre-filter for `kind:block tag:Task` queries was matching only `body LIKE '%#Task%'` (legacy inline syntax). Blocks using `tags:: Task` continuation-line syntax (e.g. `notes/projects.md`) were excluded before `block_matches` even ran. Fix: relax the LIKE pattern to `%Task%` — over-inclusive at the SQL stage, refined by `block_matches` in-memory.
- [x] **#4 — Ctrl+U / Ctrl+D page-jump in NORMAL mode.** Earlier 9.9 used `Vim.mapCommand("<C-d>", "action", ..., {context:"normal"})`, which never fired on macOS — cm6's `standardKeymap` binds `Ctrl-d`→`deleteCharForward` at a precedence that wins over cm-vim's domEventHandlers. Fix: register Ctrl+U/D in `blockKeymap` (cm6 keymap level), check `getCM(view)?.state?.vim?.insertMode` to yield in INSERT mode (so cm-vim's insert defaults still apply).
- [x] **#1 — `/p/dailies` auto-focus reliable.** Already covered by the BlockOutliner noteId-change reset (#6 fix); JournalView's two-RAF focus path now consistently lands.
- [x] **#2 — ⌘K input keyboard-focused; Esc closes.** HTML `autofocus` was unreliable on modal mount. Fix: bind input ref + `$effect` calling `inputRef.focus()` two RAFs after `open` becomes true. Esc handler moved up to the document-level capture-phase keydown listener so it works even when focus has wandered inside the modal.
- [x] **#3 — Cmd+K → Create lands in INSERT mode.** New notes now seed body with a single empty block (`- \n`) and the palette appends `?fresh=1` to the navigation URL. BlockOutliner's auto-focus effect reads the param and skips the `autoFocused` gate so the empty seed block enters Insert. BlockEditor's prop-change `$effect` now also calls `Vim.handleKey(cm, "i")` post-mount when `startInInsert` becomes true after focus settles (the original onMount-time path missed this case).
- [x] **#5 — j/k navigates by visual line, not logical line.** The vim j/k action was advancing by `s.doc.lineAt(...)` (logical/`\n`-separated). For wrapped paragraphs that's a paragraph-jump, mismatched with the arrow keys. Fix: probe with `view.moveVertically()` and dispatch only if the y-coord changed; cross-block fall-through fires when at the last/first visual line of the editor.
- [x] **#7 — j/k skips collapsed property lines.** Combined with #5 fix: `visualLineMove` iterates candidate positions and only commits the dispatch when the candidate's `.cm-line` element doesn't carry `.cm-tesela-hidden-prop-line` (or `.cm-tesela-tags-line`). If no visible target exists in the editor, the function returns false and the caller cross-blocks.
- [x] Files: `web/src/lib/components/BlockOutliner.svelte` (noteId-change reset; `?fresh=1` autoFocused gate); `web/src/lib/components/BlockEditor.svelte` (Ctrl+U/D in blockKeymap; visual-line `j`/`k` + hidden-line skipping; ArrowDown/Up updated to match; post-mount `startInInsert` effect); `web/src/lib/components/CommandPalette.svelte` (input bind + focus-on-open effect; doc-level Esc; `?fresh=1` on createNote); `crates/tesela-core/src/db/sqlite.rs` (block-query SQL pre-filter relaxed).

#### Phase 10.1 — Query-row in-place edit + slash menu ✓
Brought row-level affordances to query widgets (Tasks, Inbox, etc.) so the user doesn't have to drill into the source page to triage.
- [x] **`e` opens in-place edit on highlighted row.** Inline `<input>` swaps in for the row text; Enter saves via the new `setBlockText(content, blockId, newText)` helper + `api.updateNote`; Esc bails. Saves invalidate both the widget query and the underlying `note` query so the outliner reflects the edit immediately.
- [x] **`/` opens slash menu anchored to highlighted row.** Reuses the existing `SlashMenu.svelte`. Block-kind rows get six commands: Edit text, Open in split, Mark todo / doing / done, Delete block. Page-kind rows get just Open in split. Filtering: typing alphanumerics narrows; arrow keys + Enter / Esc forwarded from QWV's onkeydown to the menu.
- [x] **`setBlockText` + `deleteBlock` helpers in `triage.svelte.ts`.** Same line-number addressing as `setBlockProperty` so the three compose. `setBlockText` preserves indent + bullet + continuation lines; `deleteBlock` walks until the next bullet at `<= indent` and splices the slice.
- [x] **JournalView lands in INSERT.** Calling `.focus()` on cm-content alone wasn't enough — cm6's internal `.cm-focused` lagged and vim stayed NORMAL. Now we dispatch a synthetic `i` keydown on cm-content after focus, which both syncs cm6's focus state and enters INSERT so the user can type immediately on /p/dailies. (Caught during dogfooding the 9.9 follow-up bundle.)
- [x] Files: `web/src/lib/triage.svelte.ts` (setBlockText + deleteBlock); `web/src/lib/components/QueryWidgetView.svelte` (in-place edit, slash menu, key forwarding, edit-input CSS); `web/src/lib/components/JournalView.svelte` (synthetic 'i' for vim INSERT).
- [x] Verified end-to-end via Chrome DevTools MCP: `e` rename → disk update; `/` → menu opens with 6 commands; ArrowDown ×4 + Enter on Mark done → status flips to done on disk and row drops out of `-status:done` filter.
- [ ] Deferred: keyboard chord for "drill in split" (e.g. `^d` on row); status-cycle key in slash menu (currently the `s` shortcut still works on the row, but slash menu has fixed Mark todo/doing/done).

#### Phase 10.1 follow-up — daily-driver task creation ✓
After 10.1 dogfooding, the user surfaced two issues with the Cmd+Enter task-creation flow on dailies:
- [x] **Enter no longer drags continuation lines onto the new block.** Cmd+Enter cycles status (appends `status:: <next>` continuation), then pressing Enter at end of the bullet line was using `doc.sliceString(cursor)` which returned `\nstatus:: <value>` — the whole continuation became the new block's `raw_text`, leaving the original block bare. Fix in `BlockEditor.svelte`'s Enter handler: when the cursor is on the FIRST line of a multi-line block (cursor ≤ first `\n`), keep the continuation tail with the current block; only split the first line. Multi-line content edits past the first line keep the old split-at-cursor behavior.
- [x] **Cmd+Enter auto-tags as `Task`.** User's mental model is "Cmd+Enter creates a task". Previously it only cycled status, so new daily blocks never appeared in `/p/tasks` (which filters on `tag:Task`). Now `handleStatusCycle` also calls `toggleBlockTag(raw, "Task", autoFillNamesForTag("Task"))` if the block has no tag yet AND we're cycling into a non-empty status — so the user gets `status:: <next>`, `tags:: Task`, plus the Task tag-properties auto-filled (Priority/Deadline/Scheduled blanks). Cycling back to empty status leaves tags intact.
- [ ] **Edit-revert in QueryWidgetView** — user reports `e`+Enter edit "times out and reverts" intermittently. Couldn't reproduce in MCP test (typed "dudbar"+Enter saved cleanly to disk and UI). Parked until user re-reports with a reproducer; the fix probably involves an optimistic `setQueryData` on the widget cache so any refetch race can't show stale data.
- [x] Files: `web/src/lib/components/BlockEditor.svelte` (Enter handler keeps continuation on first-line split); `web/src/lib/components/BlockOutliner.svelte` (handleStatusCycle auto-tags Task).

#### Phase 10.1 follow-up #2 — dailies trailing-empty focus ✓
- [x] **Dailies lands on a trailing empty block, not the front of the first block.** JournalView's `ensureTrailingEmpty(noteId)` checks today's body — if the last non-blank line isn't already a bare `- ` bullet, it PUTs a new content with `- \n` appended. The anchor-scroll/focus effect then targets the LAST `.cm-editor .cm-content` in today's section (instead of the first), so the cursor lands on the empty bullet ready to type. After the PUT, the effect's "scrolled-for-anchor" flag resets so the focus re-fires once the new block lands in the DOM.
- [x] Files: `web/src/lib/components/JournalView.svelte` (ensureTrailingEmpty helper + focus effect targets last cm-editor + post-PUT flag reset).

#### Phase 10.1 follow-up #3 — slash + edit conflicts ✓
Two friction items the user surfaced after testing the previous bundle:
- [x] **`e` no longer reverts the title mid-edit.** Pressing `e` while the rename input was focused was bubbling up to QWV's `onkeydown`, which re-ran `startEditRow(row)` → `editingValue = row.label` → wiped what the user had typed. Fix: `handleKeydown` returns immediately when `editingRowId !== null` so the input owns its own keys.
- [x] **`/` on /p/tasks opens the slash menu, not the command palette.** The global `panelHandler` in `+layout.svelte` mapped `/` → dispatch `Cmd+K` to open the palette as a "search" shortcut. It checked target.tagName for INPUT/TEXTAREA/cm-editor but treated the QWV root as a plain element, so the palette stole the keystroke. Fix: extend the panelHandler's "is editing" guard to also bail when target is inside `.qwv` — QueryWidgetView owns its own keyboard scope (`j/k`, `/`, `e`, `s`).
- [x] Files: `web/src/lib/components/QueryWidgetView.svelte` (edit-mode key gate); `web/src/routes/+layout.svelte` (panelHandler `.qwv` opt-out).

#### Phase 10.1 follow-up #4 — leader-chord row menu ✓
The flat slash menu (arrow + Enter or filter-typing) felt sluggish for daily-driver use. User asked for spacemacs/which-key style: `/d` directly marks done, no nav.
- [x] **Replace QWV slash menu with leader-chord menu.** Each row chord is a single keystroke that runs immediately. For block-kind rows: `e` Edit text, `o` Open in split, `t` Mark todo, `i` Mark doing, `d` Mark done, `b` Mark backlog, `x` Delete block. Page-kind rows: `o` Open. The popover anchors under the highlighted row so the user always sees which row will be acted on. Esc or click-outside closes. Unknown letters are swallowed (so they don't bubble back into qwv nav).
- [x] The existing `SlashMenu.svelte` component is untouched — still used inside `BlockEditor.svelte` for inline `/` commands (template / date / tag / etc.).
- [x] Files: `web/src/lib/components/QueryWidgetView.svelte` (chordOpen state, buildChords tree, inline popover render + CSS).

### Phase 10.2 — Unified spacemacs-style leader chord menu ✓
After 10.1's row chord menu, the user asked to apply the chord-leader treatment app-wide and combine it with the existing Space leader (which had limited commands and only worked from NORMAL mode).

- [x] **Generic `ChordMenu.svelte` component** — accepts a tree of `ChordNode { key, label, action?, children? }`. Single-key chords descend or run; Esc/Backspace ascends; click-outside closes. Capture-phase keydown listener so cm-vim doesn't consume the keys. Replaces the deleted `LeaderMenu.svelte`.
- [x] **Unified leader tree.** Top-level groups: `f` File (new/daily/favorite/delete), `b` Block (drill/fold/props/cycle-status/delete/yank), `p` Page (favorite/doc-mode/delete), `s` Search (palette), `g` Go to (home/daily/tasks/inbox/calendar/pages), `w` Window (h/l/j/k/q for pane-nav from any mode), plus `T` Toggle drawer and `y` Yank to clipboard at root.
- [x] **`Ctrl+,` alt-trigger from INSERT mode.** New capture-phase keydown handler in `+layout.svelte` opens the same chord tree from anywhere — including inside cm-editor INSERT mode where `Space` would just type a space. User explicitly asked for this so they don't have to Esc out before reaching for the menu.
- [x] **Block-action dispatch.** "Block" submenu commands dispatch `tesela:block-action` events with `{ kind }`. `BlockOutliner` listens; only the outliner whose `rootEl.contains(document.activeElement)` runs (mirrors the existing `tesela:outliner-undo` filter). Handles drillIn / foldToggle / propsToggle / statusCycle / delete / yank.
- [x] **Page-action dispatch.** `tesela:page-action` mirrors header icon-button actions (favorite / doc-mode / delete) so they're reachable from any keyboard mode. Handler lives in the note `+page.svelte`.
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (new); `web/src/routes/+layout.svelte` (leaderTree, altLeaderHandler, ChordMenu render); `web/src/lib/components/BlockOutliner.svelte` (tesela:block-action listener); `web/src/routes/p/[id]/+page.svelte` (tesela:page-action listener); deleted `web/src/lib/components/LeaderMenu.svelte`.

#### Phase 10.2 follow-up — alt-path hint chips in chord menu ✓
- [x] **`hint?: string` on `ChordNode`** (renamed from initial `vimChord` to keep the field general). Renders as a faint right-aligned kbd chip. Used to advertise an alternative path to the same action.
- [x] **Vim-chord hints** on rows that have a NORMAL-mode equivalent: `b d` ⏎, `b f` za, `b p` gp, `b s` ⌘⏎, `b D` dd, `b y` yy, `f n` ⌘K, `s s` ⌘K, `w h/l/j/k/q` ⌃w h/l/j/k/q, `T` b, `y` "leader Y".
- [x] **URL-path hints** on Go to entries (user requested parity with vim chips on the rest of the menu): `g h` `/`, `g d` `/p/<today>`, `g t` `/p/tasks`, `g i` `/p/inbox`, `g c` `/p/calendar`, `g p` `/p/pages`. Same chip on `f d` (File → Daily) for consistency.
- [x] CSS: `.chord-hint` truncates with ellipsis at `max-width: 14ch` so longer paths don't push the layout.
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (hint field + chip render + .chord-hint CSS); `web/src/routes/+layout.svelte` (annotations).

#### Phase 10.2 follow-up #2 — `g` as Go-to prefix (vim+spacemacs pattern) ✓
- [x] **`g` in vim NORMAL opens the leader chord menu pre-descended into "Go to".** cm6 keymap binding (registered before cm-vim's keymap by extension order) captures `g`, dispatches `tesela:open-leader-at` with `path: ["Go to"]`. The menu lands on the submenu with breadcrumb visible. INSERT/VISUAL modes yield so `g`-prefixed visual operators and inserting the literal letter `g` still work.
- [x] **`gd` → Daily, `gt` → Tasks, `gi` → Inbox, `gc` → Calendar, `gh` → Home, `gp` → Pages** as a natural consequence: each is `g` + the matching first-letter chord in the popup. URL hint chips visible on each row.
- [x] **Wiki-follow rebound to `g f`.** Previous `gd` (Phase 9.9 wiki-follow chord) folded into the popup as `f` Follow wiki link with hint `[[ at ▌`. The action body lives in BlockEditor under `tesela:block-action` listener with kind=`followWiki`; only the editor whose view is currently focused responds.
- [x] **Toggle props (was `gp`) moves to `Space b p` / `Ctrl+, b p`.** No longer a `g`-prefixed chord (since `g p` is now Pages). Still reachable via the unified leader menu's Block submenu.
- [x] `ChordMenu` component gains an `initialPath?: string[]` prop so any caller can open the menu pre-descended into a sub-tree (used by `g` here, but also available for future `t` / `b` etc. prefixes).
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (initialPath prop); `web/src/routes/+layout.svelte` (leaderInitialPath state, openLeaderAtHandler, `g f` Go-to entry); `web/src/lib/components/BlockEditor.svelte` (cm6 `g` keymap binding, removed Vim.mapCommand for gd/gp, new tesela:block-action `followWiki` listener).

#### Phase 10.2 deferred (saved to memory `project_leader_menu_vision.md`):
- [ ] **User-configurable leader tree.** A `~/.tesela/leader.config.{json,toml,ts}` (TBD format) that the user edits to add / remove / rename / re-key entries. Hardcoded tree becomes the merged-in default. Implies an action registry mapping stable IDs (`block.cycleStatus`, `page.toggleFavorite`, etc.) → handlers, so configs reference IDs instead of inline functions.

### Phase 10.3 — In-block `/` slash menu → chord-leader style ✓
The last bounded action surface still using filter+arrow-nav joins the chord pattern.

- [x] **`ChordMenu` extended** with optional `position?: { x, y }` (cursor-anchored mode — overrides centered modal) and `headLabel?: string` (defaults `SPC`; slash menu sets it to `/`). Capture-phase keydown handler now swallows ALL keys when the menu is open (modal behavior) so arrows / vim chords / Cmd+letter combos can't leak through to the cm-editor behind the popover.
- [x] **In-block `/` opens chord popover anchored to caret.** Single-letter chords run actions immediately — `/t` Task, `/T` Tag picker, `/h` Heading, `/p` Property, `/l` Link, `/d` Date, `/q` Query, `/w` Widget, `/c` Collection, `/m` Template. `/s` descends into a Status submenu.
- [x] **Status submenu auto-resolves key collisions.** `assignStatusKeys()` walks each choice's letters and picks the first unclaimed one (with `doing` → `i` and `in-review` → `r` aliases pre-mapped). Falls back to digits 1-9 if all letters are taken. Handles arbitrary user-configured choices like `notes/status.md`'s `["backlog", "todo", "doing", "in-review", "done", "canceled", "on-hold", "dude"]` without dup-key crashes.
- [x] **Hint chips** on Task (`tags:: Task`), Tag picker (`#`), Property (`key:: value`), Link (`[[ ]]`), and each status row (`status:: <choice>`).
- [x] **`SlashMenu.svelte` deleted** — both callers (QWV row chord at 10.1, BlockEditor `/` at 10.3) now use ChordMenu. `AutocompleteMenu.svelte` stays for unbounded surfaces (`#` tag autocomplete, `[[` wiki-link autocomplete).
- [x] Files: `web/src/lib/components/ChordMenu.svelte` (position + headLabel props, modal key swallowing); `web/src/lib/components/BlockEditor.svelte` (getSlashTree, assignStatusKeys, ChordMenu render at slashPosition, removed slashFilter/slashMenuRef/SlashCommand and the keymap branches that forwarded keys to slashMenuRef); deleted `web/src/lib/components/SlashMenu.svelte`.

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

### Phase 12 — Task Management Depth (promoted: Apple Reminders sync, recurring, notifications)

Added 2026-05-07. Tesela is the user's daily-driver task manager; this phase rounds out the surface so it stands on its own against Apple Reminders / Todoist. Apple Reminders sync is the unlock that brings iOS automations (geofencing, Siri, Watch) without giving up file-first editing.

#### 12.1 — Apple Reminders bidirectional sync (HIGH PRIORITY)

Bridge Tesela Task blocks ↔ Apple Reminders items so the user can:
- Add a Task in Tesela → it appears in Reminders → triggers iOS location-based reminders, Watch surface, Siri.
- Tick a Reminder on iPhone → status flips to `done` in Tesela on next sync.
- Use Reminders' geofencing automations ("remind me at home") on Tesela tasks, while editing/searching/linking remains in Tesela.

Architecture (shipped):
- **macOS bridge**: `tesela-server/src/reminders/darwin.rs` uses `objc2-event-kit` directly. Web client never talks to EventKit; all interaction goes through `/sync/reminders/{push,pull}` and the combined `/sync/reminders` route.
- **Identity**: `apple_reminder_id::` on the block stores `EKCalendarItem.calendarItemIdentifier`. Missing identifier → recreate on next push.
- **Conflict gate**: `apple_reminder_synced_at::` (RFC 3339 UTC) is written on every successful push and pull. On pull, we only overwrite Tesela when `EKReminder.lastModifiedDate > synced_at` — i.e. the user actually touched it in Reminders.app since our last sync. Otherwise Tesela keeps its value.
- **Combined sync ordering**: pull-then-push. If the user only edited Tesela, pull no-ops and push writes Tesela → EK. If the user only edited in Reminders.app, pull writes EK → Tesela first, then push reaffirms. Concurrent edits: EK wins per field (documented limitation).
- **Calendar setup**: "Tesela" list auto-created on the Source that owns `defaultCalendarForNewReminders` — the only reliable way to find a writable Source for reminder-type calendars (CalDAV servers may host events but not reminders).

##### 12.1 slice 1 — push (✅ shipped, commit `7bf1560`)

Tesela → Reminders only.
- Eligible: any Task block with a parseable `deadline::` (date or `[[YYYY-MM-DD]]`, optionally with trailing time).
- Property mapping: block text → `title`; `status:: done` → `completed`; `deadline::` → `dueDateComponents` (date-only); `priority:: critical|high|medium|low` → `1|1|5|9`; no priority → `0`.
- Idempotent: first push returns `created`, subsequent pushes return `updated` for the same blocks via the stored identifier.
- POST `/sync/reminders/push` returns `{ created, updated, synced, errors }`.

##### 12.1 slice 2 — pull + combined sync (✅ shipped, commit `0ed74b5`)

Reminders → Tesela, plus a "Sync now" UI surface.
- POST `/sync/reminders/pull` walks the Tesela calendar, diffs each reminder against its matching block, gates on the `lastModifiedDate > synced_at` rule, writes back.
- Pulled fields: title (preserves inline `#tags` by re-appending them after the new text), status (`completed` ↔ `done`/`todo`), deadline (date only), priority (1-4 → high, 5 → medium, 6-9 → low).
- POST `/sync/reminders` does pull-then-push and returns `{ pull, push }`.
- UI: `Cmd+K → "Sync Apple Reminders"` and **Settings → Apple Reminders → Sync now** both call the combined endpoint and show a toast outcome.
- Orphans (reminders with no matching Tesela block) are reported but not imported — slice 3 territory.

##### 12.1 slice 3 — round-trip fidelity & ergonomics

Closing the gaps that slice 2 left open. Status as of 2026-05-09:

1. ✅ **Time-of-day round-trip** (commit `8928131`). `deadline::` now carries an optional `HH:MM` (24h or 12h with AM/PM); push writes `dueDateComponents.{hour,minute}` and pull rebuilds the full timestamp. New `Deadline` struct + 7 unit tests.
2. ✅ **Geofencing (`reminder_location::`)** — push side, title-only. Block property `reminder_location:: <name>` (e.g. `Trader Joes`) attaches via `EKCalendarItem.location`; Reminders.app shows the string and offers a long-press to upgrade it to a real geofence. CLLocation + arrive/leave proximity (full RFC 5545 / EKAlarm.structuredLocation surface) deferred — title-only covers the common "remind me at the grocery store" case.
3. ✅ **Per-list mapping** — push side. `apple_reminder_list:: <name>` on a block routes its push into the named Reminders list (creating it on the user's default source if missing). Untagged blocks still go to the auto-managed `Tesela` calendar. Pull still walks Tesela only — cross-list pull (rebuilding Tesela blocks from arbitrary Reminders lists) is a v2 feature.
4. ✅ **Auto-sync** (commit `797a8a0`). New `reminders::auto` module owning a single `Mutex` so triggers serialize cleanly. Three triggers fire `sync_all`: (a) **startup** — 10s after server boot; (b) **interval** — every 5 minutes; (c) **edit-driven** — debounced 30s after the indexer's `NoteEvent` stream goes quiet. Each call records into a shared `LastSync` exposed at `GET /sync/reminders/status`; the manual `Sync now` button also routes through `AutoSync` so the Settings UI shows a single unified "last synced N minutes ago via <trigger>" line. EKEventStore change-notification observer (true push from EK side) deferred — needs a CFRunLoop and the 5-minute interval covers the user-visible gap.
5. ✅ **Orphan handling** — push side. When `apple_reminder_id::` no longer resolves in EventKit (the Reminder was deleted in Reminders.app), sync stamps `apple_reminder_orphan:: true` on the block and skips it on subsequent pushes until the user clears the flag. Avoids the duplicate-create that would otherwise happen each sync. Pull still ignores reminders that have no matching Tesela block — "import as Task" is a v2 affordance.
6. ✅ **Recurring round-trip** (commit `01a5a63`). `recurring::` ↔ `EKRecurrenceRule` both directions. `Daily / EveryNDays / Weekly / Monthly / Yearly / Weekdays` all round-trip. Diff compares parsed values (so `every 1 week` doesn't flap with `weekly`). BYDAY sets beyond `Weekdays` and end-conditions (`until` / `count`) still deferred — same constraint as Phase 12.2's recurrence engine.
7. ✅ **UI affordances** (commit `797a8a0` together with auto-sync). Settings → Apple Reminders shows last-sync time, trigger, and outcome counts (e.g. "Synced 3m ago via interval · 2 pushed"). Surfacing `apple_reminder_id::` / `apple_reminder_synced_at::` in a "system properties" drawer group is still TODO — they currently render alongside user properties.

Out of scope still: shared lists, attachments, sub-reminders (12.4 handles Tesela-side hierarchy first), multi-account, Reminders categories outside Tasks.

#### 12.2 — Recurring tasks & events ✅ shipped

`recurring::` block property storing an rrule-subset string. On `status:: done`, the engine bumps `deadline::` to the next occurrence in place (Apple-style; one block ↔ one item for life), flips `status::` back to `todo`, and stamps `last_completed::` with the prior date. Shipped forms:
- `recurring:: daily` / `every day`
- `recurring:: weekly` / `every week`, `recurring:: every 2 weeks`
- `recurring:: monthly` / `every month`, `recurring:: every 3 months`
- `recurring:: yearly` / `annually` / `every year`, `recurring:: every 2 years`
- `recurring:: weekdays` (Mon–Fri; from Fri/Sat/Sun → next Mon)
- `recurring:: every N days`

Backend: pure `tesela_core::recurrence` (`parse` + `next_after`) with day-of-month clamping (Jan 31 + monthly → Feb 28/29) and Feb 29 leap handling. `apply_post_save_bumps` in `update_note` auto-detects status flips to `done` and rewrites the block transparently — no client-side trigger needed. Explicit `POST /api/blocks/recur-bump` exists for debugging. Frontend: `parseRecurrenceInput` mirrors the Rust parser; DatePicker has a "repeat" sub-row (none / daily / weekly / monthly / yearly / weekdays / custom); BottomDrawer commit writes `recurring::` alongside `deadline::`; `recurring.md` Property page renders the chip via the existing display-chips system. Mirrors Apple Reminders' recurrence shape so 12.1 sync round-trips correctly. Live-tested 2026-05-09 via PUT to `/notes/{id}` — flipping `status:: done` on `recurring:: monthly` produced the expected bumped deadline + `last_completed::` stamp.

Deferred to 12.2.x: BYDAY sets like `every monday, wednesday, friday`; `until` / `count` end conditions; "skip this occurrence"; recurring on `scheduled::` instead of `deadline::`; `weekends` keyword.

#### 12.3 — Notifications ✅ shipped

Desktop notifications + always-on toast fallback for three event kinds:
- **Deadline approaching** — fires once per (block, deadline) when within the configured lead time (default 1h before deadline). Open Task blocks only.
- **Scheduled time fires** — fires when `scheduled::` time-of-day is reached.
- **Recurring task rolled to next** — fires when `apply_post_save_bumps` advances a recurring task on a `status:: done` flip.

Backend: new `notifications` module on `tesela-server` runs a 60-second tokio interval that walks all notes, parses task blocks, and emits `WsEvent::DeadlineApproaching` / `ScheduledFires` / `RecurringRolled`. Dedupe by `(block_id, kind, deadline_iso)` so the same window doesn't re-trigger every minute (an in-memory `HashSet`; restarts reset, which we accept). Bare-date deadlines anchor to 9 AM local; explicit `HH:MM` honored.

Frontend: `web/src/lib/notifications.ts` owns the dispatch — toast always fires, browser `Notification` only when permission granted. Permission is requested lazily on the first Settings toggle so we don't pop a prompt at boot. Per-kind mute checkboxes in Settings (deadline / scheduled / recurring), each persisted to localStorage. Notification clicks navigate to the source note.

Live-verified 2026-05-09 by setting a deadline 30 minutes ahead and watching the WS event arrive on the next scanner tick.

Out of v1: configurable lead time (currently fixed 1h / 0m), per-property overrides via tag config, web push via service worker (works only when tab open today), in-app linked-block change events.

#### 12.4 — Task hierarchy ✅ shipped (subtasks + same-note deps)

- ✅ **Subtask rollup chip**: `BlockOutliner` walks each block's direct children (one indent deeper) and renders a small `X/Y` pill ahead of the property chips when any child has `status::`. Green when complete, neutral otherwise. Purely visual — no markdown footprint. `Cmd+Enter` quick-add for a child task is still TODO.
- ✅ **Dependencies — `blocked_by::`**: same-note refs only. When `update_note` detects a flipped-to-done block, `apply_dependency_cycles` walks the same note for any `status:: backlog` block whose `blocked_by::` references include the just-done block, then auto-flips it to `todo` if all blockers are now done. Cross-note dependency walking deferred — requires a reverse-index pass per save. Lock pill renders on chips of blocked tasks (amber, with the `lock` icon).
- **Project rollup row** — deferred. Per-tag summary chip ("5/12 done · earliest May 18") would live on tag pages. Today the kanban already gives per-status counts, so this is a nice-to-have rather than a daily-driver gap.

Live-verified 2026-05-09: backlog task auto-flipped to `todo` the moment its blocker's PUT carried `status:: done`.

#### Sequencing

1. **12.2 Recurring** ships first — it's purely local, exercises the property + chip system, and is a prerequisite for 12.1 round-tripping recurring Reminders.
2. **12.3 Notifications** ships second — wires the WS event stream + Notification API; small surface, big perceived value.
3. **12.1 Apple Reminders sync** ships third — biggest scope, needs Swift FFI work; depends on stable `recurring::` semantics from 12.2.
4. **12.4 Hierarchy** rolls in alongside 12.1 as polish.

### Phase 4: Distribution

#### (Optional) Tauri Wrap
- [ ] Tauri shell serving `web/out/`
- [ ] Menu bar, global hotkeys, system tray

**Deferred:** Whiteboards, long-form prose, App Store, plugin marketplace, collaborative editing.

---

## iOS App — Phases

Native SwiftUI iPhone app at `app/Tesela-iOS/` — touch-first, no vim (see memory `mobile-strategy-ios-native`). Phases 0–31 below were not previously tracked in this roadmap; reconstructed from git history on 2026-05-20.

The app talks to a `tesela-server` over HTTP — the `MockMosaicService` class is the *real* client despite its name. It is **not** yet a sync peer (`SyncState` is a debug toggle). Real iOS sync stays gated behind the sync work order (memory `sync-work-order`): iOS sync is last, after block-level sync → LAN transport → desktop sync UX.

### Phases 0–16 — UI shell ✓

- [x] Cross-compile Rust core + Xcode scaffold (`40f7250`)
- [x] Design system: 17 themes, density tiers, type scales (Phases 0–1)
- [x] Component primitives, mock data, Daily front door (Phases 2–4)
- [x] Library tab, PageView with tag chips + collapsible Peek (Phases 5–6)
- [x] Search, context menu, properties sheet, settings, pair flow (Phases 7–10)
- [x] Onboarding, ambient grid, page stack, FFI stub, skeletons (Phases 11–16)

All built against `MockMosaicService`'s in-memory seed — no real data yet.

### Phase 17 — HTTP backend ✓

- [x] `MockMosaicService` gains an `.http` mode against a local `tesela-server`; the app now reads and writes a real mosaic
- [x] 17.1 ATS config; 17.2 property-based task format aligned with the web client

### Phase 18 — Inbox + bottom chrome ✓

- [x] Inbox/triage tab, Mail-style search bar
- [x] 18.1–18.8 iterated the Liquid Glass bottom chrome, landing on iOS 26 native `TabView` with `Tab(role: .search)`

### Phase 19 — Real page content ✓

- [x] Page load, render, and writeback against the server

### Phases 20–31 — editing & voice ✓

- [x] 20–23 block editing, navigation, search, triage
- [x] 24–25 transcription model management
- [x] 26–31 end-to-end + on-device voice (whisper.cpp, streaming, Parakeet gating)

### Recent — pairing, multi-mosaic ✓

- [x] Camera QR + 6-character short-code device pairing
- [x] Auto-reconnect with backoff, disconnect, real pair card
- [x] Native TabView chrome, persistent capture bar, keyboard toolbar
- [x] Device-local multi-mosaic registry (`MosaicProfile` profiles)
- [x] Real per-page backlinks, client-derived outline, local pinned store (`d548cc3`)
- [x] Server-side multi-mosaic — discover / add / switch a server's mosaics (`b8124c3`)

### iOS — Now / Next

- The Peek surface is now real-data on every lens — backlinks, outline, props, tasks, and graph (graph = an outgoing-links list, tap-to-navigate). A real in-app **graph render** is a wanted later item; the list is the interim.
- **Parakeet transcription** — the download-completion crash is fixed (it affected *all* model downloads). Parakeet itself still isn't usable: the catalog URLs 404 and on-device inference isn't wired (`inferenceSupported: false`). Open: what the Parakeet rows should do until then.
- The Settings → Sync page has some mocked elements; sync itself works (per Taylor, 2026-05-20).
- **Architecture note:** the app is an HTTP client, not the UniFFI-embedded core described in memory `mobile-strategy-ios-native`. `tesela-sync` UniFFI bindings are generated, but the embedded-core path is deferred.

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
- [ ] **Panel-flexibility Playwright smoke (Phase 4 of `.docs/ai/phases/2026-05-11-panel-flexibility-plan.md`)** — three smoke tests in `web/tests/perf/panel-flex.spec.ts` covering (a) BottomTab legacy-string → JSON migration, (b) `railOpen` persisted across reload, (c) `drawerSide=right` persisted across reload. The plan's Task 4.1 has the test bodies. Plumbing decision: either reuse `playwright.perf.config.ts` + spin up the heavy perf-test runner (cost: ~60s setup per CI run), OR run against the existing dev server with a lightweight standalone config. Recommend the latter — these tests need no fixture data; the existing dev mosaic suffices. Estimate: ~80 lines of test code + ~30 lines of standalone config.
- [ ] **Importers add `#Task` tag + one-time backfill** — Both Logseq (`crates/tesela-core/src/import_logseq.rs:613-622`) and org (`crates/tesela-cli/src/import_org.rs:268-282`) importers convert `TODO`/`DOING`/`DONE`/`LATER`/`NOW`/`WAITING`/`CANCELED` markers into `status::` block properties but never tag the block with `#Task`. The `Tasks` system widget (`crates/tesela-core/src/system_widgets.rs:54`) queries `tag:Task` — so imported tasks are invisible on `/p/tasks` until tagged. Workaround in place: `tasks.md` query temporarily changed to `kind:block has:status -status:done` (commit-pending). Proper fix: (a) append ` #Task` to the converted text line in both importers; (b) write a one-time backfill script that walks every `.md` in `notes/`, finds blocks with a `status::` property and no `#Task` in `tags`/inline, and appends ` #Task` to the parent line; (c) revert `tasks.md` query back to `kind:block tag:Task -status:done`. Idempotency check: skip blocks where `Task` already appears in tags. Test fixture: an imported workspace with `- TODO buy milk` → after backfill, block text is `- buy milk #Task`. Estimate: ~30 lines per importer + ~120 lines for the backfill script (in `crates/tesela-cli/src/main.rs` as a `tesela tag-tasks` subcommand) + tests.

### Unlinked references (Logseq-style)

- [ ] **Surface "unlinked references" in the v5 backlinks-of-page derived buffer** — placeholder already in the UI under the "Unlinked references" section. Backend changes: add `/api/notes/:id/unlinked` returning notes whose body contains the focused page's `title` OR any of its `aliases` as a plain-text substring without `[[...]]` wrapping. Skip the focused page itself, skip blocks already containing a `[[wiki-link]]` to the focused page on the same line. Frontend changes: TanStack-query the endpoint inside `web/src/lib/renderers/derived/backlinks-of-page.svelte`, render rows that mirror the Backlinks section but with a "Link" inline-action button per row that wraps the mention in `[[...]]` (one-click promote to a real link). Open questions: case-sensitivity (probably case-insensitive); minimum match length (avoid matching short common words — maybe require ≥4 chars); whether to also scan code fences (probably skip).

### Cross-cutting (needs Opus to scope)

- [x] **Frontend perf smoke tests (Phase 14.2)** — extends the Phase 14 regression harness (`.docs/perf/README.md`) to cover the web app, where the last two real-world scaling regressions actually landed (Dailies `limit: 500` + 70-CodeMirror-mount). Pick **Playwright** (battle-tested, used by SvelteKit's own examples) over chrome-devtools-mcp (developer-only). New layout: `web/tests/perf/`, with `pnpm test:perf` running the suite. Setup: spawn `tesela-server` against a `MosaicBuilder::medium()` fixture (already exists at `crates/tesela-fixtures/`); export the synthetic mosaic path via a small `cargo run -p tesela-fixtures-cli` helper that takes `--out <path> --preset medium`. Test scenarios: (1) `/p/dailies` first-paint < 1.5s (the Phase 14 budget), assert via `page.waitForSelector(".day:nth-child(5)", { timeout: 1500 })`; (2) navigating rail → Tasks page → first kanban card visible < 800ms; (3) command palette ⌘K opens + first result rendered < 300ms; (4) Settings → Mosaic → Plan import on a small synthetic Logseq vault < 5s. Each test reports its timing as a Playwright attachment so we can diff across runs. Baseline diff is best-effort (not gated). No PR-blocking — informational. Estimate: ~400 lines of test code + ~50 lines for the fixtures-cli helper.

- [x] **CI perf workflow (Phase 14.3)** — new `.github/workflows/perf.yml`. Triggers: PR + nightly main. Job 1 runs `cargo bench --workspace --benches -- --save-baseline pr-current`. Job 2 fetches the most-recent `main` baseline artifact (uploaded by the nightly run) and diffs via `critcmp pr-current main` (https://github.com/BurntSushi/critcmp — a single-binary criterion diff tool). Posts a PR comment with the table only when any bench regresses >10%. The same workflow nightly-on-main runs the suite and uploads its results as `bench-baseline-main` artifact (with 30-day retention) so PR jobs have a baseline to diff. Frontend Playwright suite (from previous item) runs in the same workflow on the PR side, with timings posted as comment attachments. Doesn't block merge — informational. Estimate: ~150 lines of workflow YAML + brief README pointer.

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
