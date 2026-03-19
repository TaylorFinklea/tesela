# Tesela Architecture

## 1. Overview

Tesela is a keyboard-first, file-based note-taking system built on the **Mosaic** model with **outliner architecture**. Notes are Markdown files with block-based structure, forming a knowledge graph through bidirectional links and hierarchical inheritance. The architecture prioritizes data ownership, offline-first operation, and extensibility.

**Key Principles:**
- Files are truth, database is cache
- Core is headless, UIs are thin shells
- All communication through async trait APIs
- Plugins sandboxed, no direct file/DB access
- Outliner format with block inheritance

## 2. Workspace Structure

```
tesela/
├── Cargo.toml                  # Workspace root
├── crates/
│   ├── tesela-core/            # Foundation: types, traits, storage, DB, indexer
│   ├── tesela-cli/             # `tesela` binary — thin dispatcher over core
│   ├── tesela-tui/             # `tesela-tui` binary — Elm-style TUI (ratatui)
│   ├── tesela-mcp/             # `tesela-mcp` binary — MCP server (JSON-RPC 2.0)
│   └── tesela-plugins/         # Lua + WASM plugin runtimes
├── .github/workflows/          # CI and release automation
└── docs/                       # Documentation
```

### Crate Dependency Graph

```
tesela-core  (no internal deps)
     ↑              ↑
     |               |
tesela-cli    tesela-tui    tesela-mcp    tesela-plugins
     ↑                          ↑              ↑
     |                          |              |
tesela-plugins            tesela-core    tesela-core
```

- **tesela-core**: depended on by all other crates
- **tesela-plugins**: depended on by tesela-cli and tesela-mcp
- **tesela-tui**: depends only on tesela-core
- **tesela-mcp**: both a library and binary

## 3. crate: tesela-core

The headless engine. Provides:

| Module | Purpose |
|--------|---------|
| `note` | `Note`, `NoteId`, `NoteMetadata`, `SearchHit` types |
| `error` | `TeselaError` enum, `ResultExt` trait |
| `link` | `Link`, `LinkType`, wiki-link extraction |
| `tag` | `Tag` type and parsing |
| `config` | `Config`, `StorageConfig`, `GeneralConfig` |
| `daily` | `DailyNoteConfig` |
| `export` | `ExportFormat`, `export_note()` |
| `db/sqlite` | `SqliteIndex` — FTS5 search, WAL mode, connection pool |
| `storage/filesystem` | `FsNoteStore` — CRUD on Markdown files with frontmatter |
| `storage/markdown` | Frontmatter parsing and generation |
| `indexer` | `Indexer` / `IndexerHandle` — file watcher with debounced reindex |
| `traits/*` | `NoteStore`, `SearchIndex`, `LinkGraph`, `Plugin` traits |

### Key Traits

```rust
// Storage
pub trait NoteStore: Send + Sync {
    async fn get(&self, id: &NoteId) -> Result<Option<Note>>;
    async fn create(&self, title: &str, content: &str, tags: &[Tag]) -> Result<Note>;
    async fn update(&self, id: &NoteId, content: &str) -> Result<Note>;
    async fn delete(&self, id: &NoteId) -> Result<()>;
    async fn list(&self, tag: Option<&str>, limit: u64, offset: u64) -> Result<Vec<Note>>;
    async fn daily_note(&self, date: chrono::NaiveDate) -> Result<Option<Note>>;
    async fn mosaic_root(&self) -> PathBuf;
}

// Search
pub trait SearchIndex: Send + Sync {
    async fn search(&self, query: &str, limit: u64, offset: u64) -> Result<Vec<SearchHit>>;
    async fn upsert_note(&self, note: &Note) -> Result<()>;
    async fn remove_note(&self, id: &NoteId) -> Result<()>;
    async fn rebuild(&self, notes: Vec<Note>) -> Result<()>;
}

// Links
pub trait LinkGraph: Send + Sync {
    async fn get_backlinks(&self, id: &NoteId) -> Result<Vec<Link>>;
    async fn get_forward_links(&self, id: &NoteId) -> Result<Vec<Link>>;
}

// Plugins
pub trait Plugin: Send + Sync {
    fn id(&self) -> &str;
    fn on_note_created(&self, note: &Note) -> Result<()>;
    fn on_note_updated(&self, note: &Note) -> Result<()>;
    fn on_note_deleted(&self, id: &NoteId) -> Result<()>;
    fn on_search(&self, query: &str) -> Result<Option<String>>;
}
```

## 4. crate: tesela-cli

The `tesela` binary. Thin command dispatcher using `clap`. Each subcommand calls into `tesela-core` traits directly (in-process). No business logic lives here.

**Subcommands:** `init`, `new`, `list`, `cat`, `edit`, `search`, `daily`, `links`, `export`, `reindex`, `tui`, `completions`

## 5. crate: tesela-tui

The `tesela-tui` binary. Elm-style architecture: `Event → Action → State → View`, no side effects in the handler.

```
main.rs         Terminal setup, indexer wiring, App::run()
app.rs          Event loop, draw dispatch, action processing
handler.rs      Pure function: (State, Event) → Vec<Action>
action.rs       Action enum
event.rs        Event enum (Key, Tick, Resize)
state/
  mod.rs        AppState
  mode.rs       Mode enum (MainMenu, Listing, Search, NoteView, Help)
  listing.rs    ListingState
  search.rs     SearchState
view/           One render fn per mode
widgets/
  outliner.rs   Block-tree outliner widget
  graph.rs      Backlinks/forward-links graph widget
```

## 6. crate: tesela-mcp

The `tesela-mcp` binary. MCP server over JSON-RPC 2.0 on stdin/stdout.

**Exposed tools:**
1. `search_notes` — full-text search
2. `get_note` — by ID or title
3. `create_note` — create with title/content/tags
4. `list_notes` — with tag filter and pagination
5. `get_backlinks` — backlinks for a note
6. `get_daily_note` — get or create today's daily note

**Protocol methods:** `initialize`, `tools/list`, `tools/call`, `notifications/initialized`, `ping`

## 7. crate: tesela-plugins

Plugin runtimes. The loader dispatches by file extension:

| Extension | Runtime | Status |
|-----------|---------|--------|
| `.lua` | `LuaPlugin` via `mlua` | Fully implemented |
| `.wasm` | `WasmPlugin` via wasmtime | Stub (no-ops) |

Plugins are loaded from `~/.tesela/plugins/` and `<mosaic>/.tesela/plugins/`.

## 8. Data Flow

### Storage
| Layer | Authority |
|-------|-----------|
| `notes/` and `dailies/` directories | Authoritative — plain Markdown with YAML frontmatter |
| SQLite (`tesela.db`) | Derivative cache — FTS5 index, link graph |
| `tesela.toml` | User configuration |

### Write Path
`NoteStore::create/update` → Markdown file → `Indexer` → SQLite upsert

### Read Path
`SearchIndex::search` → SQLite FTS5 → `SearchHit` results

### External Edit
File watcher (notify) → debounced event → `Indexer::reindex_file` → SQLite update

## 9. Outliner Format

All notes use block-based structure:

```markdown
---
title: "Example Note"
created: 2025-01-15T10:30:00Z
tags: ["example"]
---
- Top-level block #important
  - Child inherits #important
  - Another child
- Second top-level block
```

Blocks start with `- ` at varying indentation levels. Child blocks inherit tags from parents. The TUI outliner widget renders this as a collapsible tree.

## 10. Deployment

| Mode | Use Case |
|------|---------|
| CLI (`tesela`) | Script-friendly, shell integration |
| TUI (`tesela tui`) | Interactive daily-driver interface |
| MCP server (`tesela-mcp`) | AI assistant integration (Claude Code, etc.) |
| Slint GUI (planned) | Desktop/mobile native UI |

## 11. Sync

Files-as-truth means any file sync tool works from day one (Syncthing, Dropbox, iCloud Drive). Future: native P2P sync with conflict detection, optional CRDT-based real-time collaboration.

## 12. Open Risks

| Risk | Mitigation |
|------|-----------|
| SQLite lock contention | WAL mode + connection pooling |
| Plugin security | Capability-based permissions, WASM sandbox |
| Schema evolution | Versioned migrations |
| Cross-platform file watching | Polling fallback + checksums |
