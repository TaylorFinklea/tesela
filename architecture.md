# Tesela Architecture

## 1. Overview

Tesela is a keyboard-first, file-based note-taking system built on the **Island Core** pattern with **outliner architecture**. Notes are Markdown files with block-based structure forming a knowledge mosaic through bidirectional links and hierarchical inheritance. The architecture prioritizes data ownership, offline-first operation, and extensibility.

**Key Principles:**
- Files are truth, database is cache
- Core is headless, UIs are thin shells  
- All communication through async trait API
- Plugins sandboxed, no direct file/DB access
- Outliner format with block inheritance
- Organized directory separation (notes/, dailies/, attachments/)

## 2. Project Structure

### Source Code Organization

```
tesela/
├── src/
│   ├── main.rs         # CLI entry point with clap integration
│   ├── lib.rs          # Library API and public exports
│   ├── commands.rs     # All command implementations
│   └── tui/           # Terminal User Interface
│       └── app.rs     # TUI application logic with graph view
├── tests/
│   ├── cli_integration.rs  # End-to-end CLI tests
│   └── fixtures/          # Test data
├── .cargo/config.toml     # Build configuration (macOS fixes)
├── Cargo.toml            # Dependencies and project manifest
├── ARCHITECTURE.md       # This file - system design
└── README.md            # User documentation
```

### Key Components

- **`src/main.rs`**: CLI definition with clap, command routing, error handling
- **`src/lib.rs`**: Public library API, module declarations, unit tests
- **`src/commands.rs`**: Core business logic - note creation, search, cross-directory operations
- **`src/tui/app.rs`**: Interactive TUI with listing, search, preview, and graph modes
- **`tests/`**: Integration tests ensuring CLI functionality works end-to-end

### User Data Structure

```
<mosaic-directory>/
├── tesela.toml         # Mosaic configuration
├── notes/             # Regular topic-based notes
├── dailies/           # Daily notes with date-based naming
└── attachments/       # Binary files (PDFs, images, etc.)
```

## 3. Component Diagram

```mermaid
graph TB
    subgraph "User Space"
        NOTES[notes/ - Regular Notes]
        DAILIES[dailies/ - Daily Notes]
        ATT[attachments/ - File Attachments]
        CFG[tesela.toml - Config]
    end
    
    subgraph "Core Island"
        API[Core API Layer]
        NE[Note Engine]
        IDX[Indexer]
        QRY[Query Engine]
        CACHE[Query Cache]
        GRP[Graph Service]
        SYN[Sync Service]
        AI[AI Bridge]
        ATT[Attachment Service]
        EVT[Event Bus]
        DB[(SQLite Index)]
    end
    
    subgraph "Transport Layer"
        IPC[IPC/gRPC Adapter]
        DIR[Direct Call Adapter]
    end
    
    subgraph "Clients"
        DESK[Slint Desktop]
        MOB[Slint Mobile]
        CLI[CLI/TUI]
        PLG[Plugin Runtime]
    end
    
    NOTES <--> NE
    DAILIES <--> NE
    ATT <--> ATT
    NE <--> DB
    NE --> IDX
    IDX --> DB
    QRY --> CACHE
    CACHE --> DB
    GRP --> DB
    
    API --> NE
    API --> IDX
    API --> QRY
    API --> CACHE
    API --> GRP
    API --> SYN
    API --> AI
    API --> ATT
    API --> EVT
    
    DIR --> API
    IPC --> API
    
    DESK --> DIR
    MOB --> IPC
    CLI --> DIR
    PLG --> API
```

## 4. Data Flow & Storage Model

### Storage Layers

| Layer | Purpose | Authority |
|-------|---------|-----------|
| notes/ directory | Regular notes in outliner format | Authoritative |
| dailies/ directory | Daily notes with date-based naming | Authoritative |
| attachments/ directory | Binary files (PDFs, images, etc.) | Authoritative |
| SQLite index | Cross-directory query acceleration | Derivative |
| tesela.toml | User preferences and configuration | User-controlled |

### SQLite Schema (v1)

```sql
-- Core tables (v0.3.7)
notes (id, path, title, created, modified, checksum, directory)
blocks (id, note_id, content, type, position, parent_id)
links (source_id, target_id, type, context)
tags (id, name, note_id, inherited)
types (id, name, schema_json)
attachments (id, note_id, filename, path, mime_type, size, checksum)

-- FTS5 virtual table with cross-directory support
notes_fts (title, content, directory)
```

### Data Flow
1. **Write**: API → Note Engine → Markdown file (notes/ or dailies/) → Indexer → SQLite → Cache invalidation
2. **Read**: API → Query Engine → Cache (hot path) → SQLite (warm path) → Cross-directory scan (fallback)
3. **External edit**: File watcher → Indexer → SQLite update → Cache invalidation → Event broadcast
4. **Cross-directory operations**: API → Scan both notes/ and dailies/ → Merge results → Return unified view

## 5. Core API (Rust traits)

```rust
// Primary service traits
#[async_trait]
pub trait NoteService {
    async fn create(&self, content: &str, metadata: NoteMeta) -> Result<NoteId>;
    async fn update(&self, id: NoteId, content: &str) -> Result<()>;
    async fn delete(&self, id: NoteId) -> Result<()>;
    async fn get(&self, id: NoteId) -> Result<Note>;
    async fn link(&self, from: NoteId, to: NoteId, link_type: LinkType) -> Result<()>;
    async fn attach(&self, note_id: NoteId, file_data: Vec<u8>, filename: &str) -> Result<AttachmentId>;
    async fn get_attachment(&self, id: AttachmentId) -> Result<Attachment>;
}

#[async_trait]
pub trait QueryService {
    async fn search(&self, query: &str, filters: SearchFilters) -> Result<Vec<NoteRef>>;
    async fn graph_neighbors(&self, id: NoteId, depth: u8) -> Result<Graph>;
    async fn daily_note(&self, date: NaiveDate) -> Result<Option<Note>>;
}

#[async_trait]
pub trait PluginHost {
    async fn register(&self, manifest: PluginManifest) -> Result<PluginId>;
    async fn call(&self, plugin_id: PluginId, method: &str, args: Value) -> Result<Value>;
    async fn check_rate_limit(&self, plugin_id: PluginId) -> Result<()>;
}

// Plugin rate limiting
pub struct RateLimiter {
    calls_per_minute: u32,
    burst_size: u32,
}

// Event system
pub trait EventSubscriber {
    fn on_note_changed(&self, event: NoteEvent);
    fn on_index_rebuilt(&self, stats: IndexStats);
}
```

## 6. Plugin Architecture

### Security Model

Plugins run in sandboxed environments with capability-based permissions:

```rust
pub struct PluginPermissions {
    read_notes: bool,
    write_notes: bool,
    network_access: bool,
    file_system: FileSystemAccess,
    rate_limits: RateLimiter,
}

pub enum FileSystemAccess {
    None,
    PluginDataOnly,  // Only plugin's data directory
    ReadOnly(Vec<PathBuf>),  // Specific allowed paths
}
```

### Language Support Progression

#### Phase 1: Lua & Fennel (v1.0)
- **Target**: Power users, Neovim community, Lisp enthusiasts
- **Runtime**: `mlua` with custom sandbox (Fennel compiles to Lua)
- **API**: Synchronous, event-driven

```lua
-- Example Lua plugin: Auto-tagger
local tesela = require("tesela")

tesela.on_note_saved(function(note)
    local content = note:content()
    
    -- Auto-detect programming languages
    if content:match("```rust") then
        note:add_tag("rust")
    end
    
    -- Extract TODOs
    for todo in content:gmatch("TODO:%s*([^\n]+)") do
        tesela.create_task(todo, note.id)
    end
end)
```

```fennel
;; Example Fennel plugin: Auto-tagger
(local tesela (require :tesela))

(tesela.on-note-saved
  (fn [note]
    (let [content (note:content)]
      ;; Auto-detect programming languages
      (when (content:match "```rust")
        (note:add-tag "rust"))
      
      ;; Extract TODOs
      (each [todo (content:gmatch "TODO:%s*([^\n]+)")]
        (tesela.create-task todo note.id)))))
```

#### Phase 2: JavaScript/TypeScript (v1.5)
- **Target**: Web developers
- **Runtime**: QuickJS or embedded Deno
- **API**: Async-first, Promise-based

```typescript
import { Plugin, Note } from "@tesela/plugin-api";

export default class SmartLinker extends Plugin {
    async onNoteCreated(note: Note) {
        const similar = await this.findSimilarNotes(note);
        
        for (const match of similar) {
            if (match.similarity > 0.8) {
                await note.addLink(match.id, "related");
            }
        }
    }
}
```

#### Phase 3: WebAssembly (v2.0)
- **Target**: Any language, high-performance plugins
- **Runtime**: Wasmtime with WASI
- **API**: Interface types, component model

### Plugin API Surface

```rust
// Core plugin trait
#[async_trait]
pub trait Plugin {
    fn manifest(&self) -> &PluginManifest;
    async fn activate(&mut self, host: PluginHost) -> Result<()>;
    async fn deactivate(&mut self) -> Result<()>;
}

// Available hooks
pub enum PluginHook {
    // Lifecycle
    OnNoteCreated(NoteId),
    OnNoteUpdated(NoteId, ChangeSet),
    OnNoteDeleted(NoteId),
    
    // User actions
    OnSearch(Query),
    OnLinkCreated(LinkId),
    
    // System events  
    OnIndexComplete,
    OnSyncStart,
    
    // UI extension points
    OnEditorAction(Action),
    OnRenderNote(NoteId),
}

// Plugin capabilities
impl PluginApi {
    // Read operations
    async fn get_note(&self, id: NoteId) -> Result<Note>;
    async fn search(&self, query: &str) -> Result<Vec<NoteRef>>;
    async fn get_tags(&self) -> Result<Vec<Tag>>;
    
    // Write operations (permission gated)
    async fn update_note(&self, id: NoteId, content: &str) -> Result<()>;
    async fn add_tag(&self, note: NoteId, tag: &str) -> Result<()>;
    
    // Plugin storage
    async fn get_data(&self, key: &str) -> Result<Option<Value>>;
    async fn set_data(&self, key: &str, value: Value) -> Result<()>;
    
    // UI extensions
    async fn show_notification(&self, msg: &str) -> Result<()>;
    async fn register_command(&self, cmd: Command) -> Result<()>;
}
```

### Example Plugin Use Cases

| Plugin | Language | Permissions | Purpose |
|--------|----------|-------------|---------|
| Auto-tagger | Lua | Read notes | Tag based on content patterns |
| Daily summary | JS/TS | Read notes, Network | Generate AI summaries |
| Citation manager | WASM | Read/write notes, Network | Manage academic references |
| Graph visualizer | JS/TS | Read notes | Custom graph layouts |
| Sync adapter | Rust/WASM | Read/write, Network | Custom sync backends |
| Org-mode bridge | Elisp | Read/write notes | Import/export org files |
| Smart templates | Fennel | Read/write notes | Dynamic note templates |

### Plugin Distribution

- **Registry**: GitHub-based plugin registry (like Obsidian)
- **Format**: `.tplugin` bundle with manifest + code + assets
- **Installation**: Copy to `~/.tesela/plugins/`
- **Updates**: Semantic versioning with compatibility checks

## 7. Deployment & Sync Options

| Mode | Transport | Use Case |
|------|-----------|----------|
| Embedded | Direct calls | Desktop app, CLI |
| IPC | Unix socket/Named pipe | Mobile, sandboxed environments |
| Network | gRPC + TLS | Remote clients, web UI |
| Sync | WebDAV/S3 + E2E encryption | Multi-device |

**Sync Strategy (v0.7):**
- File-based sync with conflict detection
- Last-write-wins with manual conflict resolution
- Merkle trees for efficient diff detection
- **P2P Option**: Any file sync tool (Syncthing, Dropbox, etc.) works from day one
- **Server Option**: Self-hosted sync server for centralized backup

**Future Sync (v1.2+):**
- Conflict-free replicated data types (CRDT) for real-time collaboration
- Operational transformation for concurrent edits
- Presence awareness and live cursors

## 8. Open Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| SQLite lock contention | Performance degradation | WAL mode, read replicas |
| Large file handling | Memory pressure | Streaming parser, chunked indexing |
| Plugin security | Data breach | Capability-based permissions, WASM sandbox |
| Schema evolution | Breaking changes | Versioned migrations, compatibility layer |
| Cross-platform file watching | Missed updates | Polling fallback, checksums |

## 9. Development Roadmap

See the comprehensive [Development Roadmap in README.md](README.md#🗺️-development-roadmap) for current status, upcoming features, and long-term vision.

For detailed task tracking and implementation notes, see [plan.md](plan.md).