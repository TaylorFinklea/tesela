# Tesela Roadmap

## What Tesela Is

Keyboard-first note-taking system (org-mode successor). Rust backend, native macOS SwiftUI app. Taylor's daily-driver tool — reliability matters more than features.

**Core principle:** Database-first, files are export. Apple-first, web later.

## Architecture

- **Rust workspace** (`crates/`): tesela-core, tesela-cli, tesela-tui, tesela-mcp, tesela-server, tesela-plugins
- **SwiftUI macOS app** (`app/Tesela/`): connects to tesela-server on localhost:7474 (REST + WebSocket)
- **Type system**: Tags, Properties, and Values are pages with YAML frontmatter (Logseq DB model)
- **React board prototype**: external project for Life OS kanban design — will merge into Tesela when proven

## Completed

- MVP: Block outliner, sidebar, search, WebSocket sync, tiles, Vim mode, graph view
- Type system: Tag/Property pages, inheritance chains, property configuration UI
- Vim engine: Visual mode, dot-repeat, /search with highlighting + n/N
- Inline autocomplete for `#tags` and `[[page refs]]` with "New tag" creation
- Tag display: all tags as right-aligned plain text (Logseq style), removed from editor
- Block drill-in (Logseq-style zoom) + back/forward navigation
- Tag page views: table + kanban with multi-property filtering, drag-and-drop, sortable columns
- Right sidebar: page info, grouped backlinks with context, unlinked references, focused block properties
- Custom bullet icons (SF Symbols) with color picker per type
- Baseline-aligned layout system for all inline elements
- Backup: CLI `tesela backup` + auto-daily on server startup
- Life OS data model types: Domain, LifeProject, Issue, Ritual, ScheduledItem
- Node references (property type "node" linking to pages)
- Search match count display

---

## Phase 1: Polish & Reliability (current)

Make what exists beautiful and trustworthy. This is the gate to daily-driver status.

- [x] **UI overhaul** — theme system, baseline alignment, right-aligned badges/tags
  - [x] Consistent spacing, padding, and margins across all views
  - [x] Theme system (dark/light + accent color customization)
  - [x] Bullet threading from baseline
  - [ ] Icon/status alignment pixel-perfection (in progress)
- [x] **Server lifecycle** — embed server in SwiftUI app as child process + keep LaunchAgent as CLI fallback
- [x] **Data integrity** — backup CLI + auto-daily + restore command
- [x] **Empty block UX** — ghost bullet on hover

## Phase 2: LogSeq Importer ✅

- [x] **CLI command**: `tesela import-logseq --source ~/logseq [--dry-run]`
- [x] **Format conversion**: journals → daily notes, pages → notes
- [x] **Syntax mapping**: `DEADLINE:`, `SCHEDULED:`, `[#A]` priorities, `TODO/DOING/DONE`
- [x] **LogSeq-specific cleanup**: strip `collapsed::`, `id::`, `#+BEGIN_QUERY` blocks
- [x] **Dry-run mode**: preview what would be imported without writing

## Phase 3: First-Class Types (Anytype-style) ⚠️ NEEDS DISCOVERY

Types as classes, pages as instances. Requires product discovery session before coding.

- [ ] **Discovery**: design type creation UI, @person syntax, type templates, layout options
- [ ] **Type creation page**: name, plural name, icon, format (page/list), layout, properties
- [ ] **Instance creation**: new page automatically gets type's property schema
- [ ] **@person syntax**: `@taylor-finklea` → renders as mention, creates Person page
- [ ] **Type-specific views**: per-type default layouts, table/kanban/list per type

## Phase 4: Long-Form Writing Mode ⚠️ NEEDS DISCOVERY

Outliner-only is limiting. Need Notion-like prose alongside block structure.

- [ ] **Discovery**: design mixed outliner+prose pages, paragraph-level backlinking
- [ ] **Prose blocks**: paragraphs rendered as flowing text, still individually referenceable
- [ ] **Mixed pages**: switch between outline and prose sections on the same page
- [ ] **Block-level backlinking in prose** (like Capacities)

## Phase 5: Power Menu ⚠️ NEEDS DISCOVERY

Alfred/Raycast-style universal command bar replacing Cmd+K.

- [ ] **Discovery**: design grammar for natural language input, task shortcuts, inline properties
- [ ] **Natural language tasks**: `t Get Milk tom at 4` → Task scheduled tomorrow 4 PM
- [ ] **Universal navigation**: type page name to jump
- [ ] **Quick capture**: bare text adds to today's daily note
- [ ] **Inline properties**: `t Get Milk #shopping p:high d:friday`

## Phase 6: Query Language ⚠️ NEEDS DISCOVERY

Advanced filtering beyond the current property filters.

- [ ] **Discovery**: syntax design for `status NOT "Done" AND Tags in ("cool")`
- [ ] **Query builder UI** (visual) + raw query input (power users)
- [ ] **Saved queries**: persist as named views on tag pages
- [ ] **NOT / OR operators**: complement existing AND-only filtering

## Phase 7: Board View (from React prototype)

Life OS kanban board, designed in React, implemented natively in Tesela.

- [ ] Import proven board design from React prototype
- [ ] Native SwiftUI implementation with domain swimlanes
- [ ] Sandbox mode (draft changes before applying)
- [ ] AI integration via MCP (board state tools, domain insights)

---

## Backlog (parallel, tiered by model capability)

<!-- tier3_owner: claude -->

Items that can be done alongside phases. Each is self-contained and well-scoped. Tiered by required model capability — see `~/CLAUDE.md` for the claim protocol.

### Haiku (mechanical, no judgment)

- [ ] Pixel-perfect bullet/icon/text alignment across all block types
<!-- build-failed: 2026-04-06 cargo clippy failed at crates/tesela-core/src/db/sqlite.rs:311 (needless-borrows-for-generic-args) -->
- [x] Bullet threading visual quality (line positioning, thickness, opacity)
- [x] Tag text alignment consistency across blocks
- [ ] Consistent spacing between blocks, sections, headers
- [x] Status icon vertical centering with different font sizes
- [x] Date badge alignment with text baseline
- [ ] Sidebar visual polish (spacing, section headers, icons)
- [x] Replace 10 debug `print()` calls with os.log or remove (ServerManager.swift:22-71, AppState.swift:130, TagPageView.swift:418)
- [x] Replace 22 silent `try?` suppressions with logged error handling (AppState.swift, TagPageView.swift, ServerManager.swift)
- [x] Extract hardcoded timeout constants: ServerManager 5s health poll, APIClient 10s/30s request timeouts (ServerManager.swift:54, APIClient.swift:133,156)
- [x] Replace force-unwrap URL constructions with safe initializers (APIClient.swift:12,173,175)
- [x] Add `.expect("reason")` messages to 5 mutex lock unwraps in lua.rs (crates/tesela-plugins/src/lua.rs:86,119,129,257,275)
- [x] Add `.expect("reason")` messages to 3 regex unwraps in import_logseq.rs (crates/tesela-cli/src/import_logseq.rs:142-144)
- [x] Extract hardcoded magic numbers: SQLite max_connections, TUI tick_timeout, debounce durations (sqlite.rs:44,62, app.rs:81)

### Sonnet (some architectural judgment)

- [x] Tag extraction edge cases (tags at end of line, tags with special chars)
- [x] Autocomplete popover positioning near screen edges
- [x] Cursor position bugs after block operations (Enter, delete, indent)
- [x] BlockStyler crash guards (text/textStorage length mismatches)
- [x] Search highlighting persistence across block rebuilds (verified working)
- [x] WebSocket reconnection reliability (exponential backoff)
- [x] Block zoom save-back correctness for deeply nested blocks (verified correct)
- [ ] Split OutlinerView.swift (2155 lines) into focused modules: OutlinerLayout, OutlinerCompletion, OutlinerSearch, OutlinerProperties
- [ ] Split sqlite.rs (1126 lines) into db/migrations.rs, db/search.rs, db/links.rs, db/types.rs
- [ ] Split TagPageView.swift (841 lines) into TagPageHeader, TagBlockTable, TagKanbanBoard, TagPropertyEditor
- [ ] Replace 4 hardcoded DispatchQueue.asyncAfter delays with proper state machine or animation callbacks (ContentArea.swift:265, TilesView.swift:27,40,79)
- [x] Create shared RegexCache for duplicate regex patterns across import_logseq.rs, notes.rs, BlockStyler.swift
- [x] Add structured error handling to AppState.loadInitialData — partial failures should show user-facing indicators, not silent defaults

### Opus (design skill, cross-cutting — owned by tier3_owner)

- [ ] API endpoint integration tests (server routes)
- [ ] SwiftUI view snapshot tests (if feasible)

### Completed

- [x] VimEngine unit tests: all motions, operators, visual mode, dot-repeat
- [x] BlockParser unit tests: tag extraction, property extraction, serialization round-trips
- [x] Block.displayText unit tests: tag stripping with various inputs
- [x] Block.updateDisplayText unit tests: tag preservation, property lines
- [x] README.md update with current features and architecture
- [x] API endpoint documentation (REST routes, parameters, responses)
- [x] MCP tool documentation (what each tool does, example usage)
- [x] Contributing guide (build steps, test commands, code conventions)
- [x] Type system documentation (how tags, properties, inheritance work)
- [x] Inline code comments for complex methods (OutlinerView.rebuildBlockViews, VimKeyHandler)

---

## Constraints

- macOS 26 minimum, Swift 6 strict concurrency
- Apple-first, web later (Tauri/web shares Rust backend API)
- No business logic in CLI/TUI/GUI — only in tesela-core traits
- Database-first; files are export format
- SF Symbols for icons (web app uses Tabler/Lucide with name mapping)

## Non-Goals (for now)

- iOS/iPadOS app
- Multi-device sync (CRDTs)
- App Store distribution
- Plugin marketplace
- Collaborative editing
