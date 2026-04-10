# Tesela Roadmap

## What Tesela Is

Keyboard-first note-taking system (org-mode successor). Rust backend + Next.js web frontend. Taylor's daily-driver tool — reliability matters more than features.

**Core principle:** Database-first, files are export format.

**Design quality bar:** Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode default.

## Architecture

- **Rust workspace** (`crates/`): tesela-core, tesela-cli, tesela-tui, tesela-mcp, tesela-server, tesela-plugins
- **Web app** (`web/`): Next.js 16 App Router + React + TypeScript + CodeMirror 6 + `@replit/codemirror-vim` + shadcn/ui + Tailwind + TanStack Query + Zustand + cmdk + Lucide; connects to `tesela-server` on `localhost:7474`
- **Type system**: Tags, Properties, and Values are pages with YAML frontmatter (Logseq DB model)

## Rust backend — already done

This is the stable surface the web client builds on. No immediate feature work planned here beyond the backlog below.

- Block outliner data model, wiki-link + tag + property parsing
- SQLite/FTS5 indexer with incremental reindex
- REST + WebSocket server (`tesela-server`) with ~95% coverage for UI needs
- MCP server for AI integration (`search_notes`, `get_note`, `create_note`, `list_notes`, `get_backlinks`, `get_daily_note`)
- CLI with `init`, `new`, `list`, `search`, `cat`, `edit`, `daily`, `links`, `export`, `backup`, `restore`, `reindex`, `install`, `uninstall`
- Daily-backup system + restore command
- LogSeq importer (`tesela import-logseq --source ~/logseq`)
- TUI (`tesela-tui`) — Elm-style, kept for headless/SSH use
- Type registry with Tag/Property/Value pages and inheritance

---

## Web Client — active work

Replaces the earlier native UI. See `/Users/tfinklea/.claude/plans/async-giggling-moth.md` for the full plan.

**Shipping strategy:** Browser-first in dev (`pnpm --dir web dev` → `localhost:3000`), Tauri wrap later.

### M0 — Scaffold & Connect ✓ (2026-04-09)

- [x] Added `ts-rs` v12 to `tesela-core` dev-deps; derived on `Note`/`NoteId`/`NoteMetadata`/`Attachment`/`SearchHit`/`Link`/`LinkType`/`GraphEdge`/`TypeDefinition`/`PropertyDef`/`ParsedBlock`. `cargo test -p tesela-core --lib export_bindings` writes 11 TS files to `web/src/lib/types/`.
- [x] Scaffolded `web/` with Next.js 16.2.3, React 19, Tailwind v4, TypeScript, App Router, `src/` layout. shadcn/ui initialized with the base-nova preset on `@base-ui/react` (neutral base color, full dark-mode tokens).
- [x] Installed CM6 core + lang-markdown + search, `@replit/codemirror-vim`, TanStack Query v5, Zustand v5, cmdk, Lucide.
- [x] `web/src/lib/api-client.ts` — typed `ApiClient` with `health()` + `listNotes()`, extensible to the full route table.
- [x] `web/src/lib/ws-client.ts` — exponential backoff reconnect (1s → 30s), `intentionallyStopped` latch, connection-id guard against stale receive loops.
- [x] Boot screen at `/`: header with live/loading/offline status pill, notes list, error/empty/loading states.
- [x] Verified via Chrome DevTools MCP — page loads, dark theme applies, api-client fires, WS reconnect loop backs off correctly.

### M1 — Read-only outliner

- [ ] Port or reuse `BlockParser` (decision: port to TS for zero round-trip, or call `tesela-core::block::parse_blocks` via a new `/notes/:id/blocks` API endpoint)
- [ ] `/p/[id]` route that fetches a note and renders its blocks in an indented outliner layout
- [ ] `BlockEditor` with one CM6 instance per block (read-only), wiki-link + tag-pill + property-line decorations, arrow-key navigation between blocks

### M2 — Editing + save-back

- [ ] CM6 editable, 500ms debounced `PUT /notes/{id}`
- [ ] Enter/Tab/Shift-Tab block ops, WS reconcile without clobbering in-flight edits, undo/redo

### M3 — Vim engine

- [ ] Write a new TS Vim engine (`web/src/editor/vim-engine/`) with state machine, motions, operators, dot-repeat, visual mode
- [ ] Cross-block motion routing layered over `@replit/codemirror-vim`
- [ ] Vitest coverage for motions, operators, visual mode, dot-repeat
- [ ] Command palette (cmdk); global shortcuts `⌘K`, `⌘J`, `⌘[`, `⌘]`

### M4 — Sidebar & tag pages (table only)

- [ ] Left sidebar (pages, recents, favorites, search)
- [ ] Tag page table view (TanStack Table) with filter/sort/property columns
- [ ] Property editor
- [ ] Right sidebar: backlinks, forward links, focused-block properties

### M5 — Tiles & drill-in

- [ ] Daily notes timeline (virtualized)
- [ ] Block zoom route `/p/[id]/zoom/[block]`

### M6 — Graph & search UI

- [ ] Cytoscape.js graph view
- [ ] Search results page against `/search`

### M7 — Theme, settings, polish

- [ ] Settings page (theme, accent color, server URL)
- [ ] Empty/loading/error states for every view
- [ ] **Linear/Logseq/Zed craft bar** — every screen held to this design standard before it ships

### M8 — (Optional) Tauri wrap

- [ ] Tauri shell serving `web/out/`
- [ ] Menu bar, global hotkeys, `tesela web` CLI subcommand

**Deferred past M8:** kanban, long-form writing mode, power menu grammar, query language, mobile/iOS, attachment upload, bulk ops.

---

## Backlog (parallel, tiered by model capability)

<!-- tier3_owner: claude -->

Items that can be done alongside milestone work. Each is self-contained and well-scoped. Tiered by required model capability — see `~/CLAUDE.md` for the claim protocol.

### Haiku (mechanical, no judgment)

- [ ] Replace one-off `regex::Regex::new(r"#[...]")` in `crates/tesela-server/src/routes/notes.rs:179` with the cached `INLINE_TAG_RE` from `crates/tesela-core/src/regex_cache.rs:21` (already the identical pattern)
- [ ] Replace `std::env::current_dir().unwrap()` in `crates/tesela-cli/src/main.rs:196` with `?` + `.context("Failed to resolve current directory")` so `tesela init` surfaces a real error instead of panicking
- [ ] Replace 2 `plist_file.to_str().unwrap()` calls in `crates/tesela-cli/src/main.rs:666,690` with `.context("plist path is not valid UTF-8")` — currently panics on non-UTF-8 HOME paths
- [ ] Replace 3 `serde_json::to_string_pretty(&results).unwrap()` calls in `crates/tesela-mcp/src/tools.rs:150,236,260` with `.expect("tool response is always serializable")` so the reason for the unwrap is documented
- [ ] Annotate 2 regex-capture unwraps in `crates/tesela-cli/src/import_logseq.rs:202,244` with `.expect("regex group 1 exists after successful match")`
- [ ] Annotate `cap.get(0).unwrap()` in `crates/tesela-core/src/link.rs:38` with `.expect("capture group 0 always exists on match")`
- [ ] Extract hardcoded server bind address `"127.0.0.1:7474"` in `crates/tesela-server/src/main.rs:154` into a `const DEFAULT_BIND_ADDR` at the top of the file
- [ ] Extract hardcoded backup-retention magic numbers into named constants: `MAX_MANUAL_BACKUPS = 10` in `crates/tesela-cli/src/main.rs:421` and `MAX_DAILY_BACKUPS = 5` in `crates/tesela-server/src/main.rs:216`

### Sonnet (some architectural judgment)

- [ ] Split `crates/tesela-core/src/db/sqlite.rs` (1126 lines) into `db/migrations.rs`, `db/search.rs`, `db/links.rs`, `db/types.rs`
- [ ] Split `crates/tesela-cli/src/main.rs` (826 lines, 14 `cmd_*` functions including the 140-line backup/restore pair at lines 378-575) into a `src/commands/` submodule — one file per command, re-exported from `commands/mod.rs`. Keep `main.rs` as a thin dispatcher.
- [ ] Extract duplicated `copy_dir_recursive` + backup retention logic out of `crates/tesela-cli/src/main.rs:561` and `crates/tesela-server/src/main.rs:224` into a shared `tesela_core::backup` module. Also unify the inconsistent retention counts (10 manual backups in CLI vs 5 daily backups in server) into a single `BackupPolicy` struct so both binaries share the same semantics.

### Opus (design skill, cross-cutting — owned by tier3_owner)

- [ ] API endpoint integration tests (server routes)

---

## Constraints

- Design quality bar: Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode default
- No business logic in the web client — only in `tesela-core` traits (`NoteStore`, `SearchIndex`, `LinkGraph`)
- Database-first; files are export format
- Icons: Lucide everywhere in the web client

## Non-Goals (for now)

- iOS/iPadOS app
- Multi-device sync (CRDTs)
- App Store distribution
- Plugin marketplace
- Collaborative editing
