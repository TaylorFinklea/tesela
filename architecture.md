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

## 2. Component Diagram

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

## 3. Data Flow & Storage Model

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

### Cache Strategy
- LRU cache for frequently accessed notes and queries
- TTL-based expiration for search results
- Invalidation on write operations
- Configurable memory limits

## 4. Core API (Rust traits)

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

## 5. Plugin Architecture

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

## 6. Deployment & Sync Options

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

## 7. Open Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| SQLite lock contention | Performance degradation | WAL mode, read replicas |
| Large file handling | Memory pressure | Streaming parser, chunked indexing |
| Plugin security | Data breach | Capability-based permissions, WASM sandbox |
| Schema evolution | Breaking changes | Versioned migrations, compatibility layer |
| Cross-platform file watching | Missed updates | Polling fallback, checksums |

## 8. Roadmap / Future Enhancements

### Phase 1: Foundation (v0.1-0.3) ✅ COMPLETE
- ✅ Core library with outliner-format file operations
- ✅ Organized folder structure (notes/, dailies/, attachments/) - sync-ready
- ✅ Attachment management with file organization
- ✅ SQLite indexing with cross-directory support
- ✅ Complete CLI with intelligent autocomplete and cross-directory functionality
- ✅ Daily notes separation with proper organization
- ✅ Smart autocomplete with title-to-filename mapping

### Phase 2: Desktop (v0.4)
- Slint desktop UI with native file drag & drop
- Local graph visualization with outliner block relationships
- PDF and image previews inline
- Cross-directory note browsing and editing
- Block-based editing with inheritance visualization

### Phase 3: Plugin System (v0.5)
- Plugin system foundation (Lua/Fennel)
- Security model and sandboxing
- Plugin API with outliner block access
- Cross-directory plugin operations

### Phase 4: Plugins & Whiteboarding (v0.6)
- Example plugins and plugin marketplace
- Excalidraw integration for whiteboarding

### Phase 5: Code Execution (v0.7)
- Code execution in notes (like org-babel)
- Multiple language support
- Sandboxed execution environment

### Phase 6: Mobile Apps (v0.8)
- iOS and Android apps with Slint
- Touch-optimized outliner editing
- Quick capture and offline sync
- Platform-specific integrations
- Cross-directory mobile operations

### Phase 7: Sync (v0.9)
- Native sync protocol for real-time updates
- Platform-specific sync (iCloud, Google Drive)
- Optional self-hosted sync server
- Better conflict resolution than file-based sync

### Phase 8: Intelligence (v1.0)
- AI integration (local LLM, OpenAI bridge)
- Smart linking suggestions with block inheritance
- Content summarization across directories
- Block-aware AI features
- Outliner structure optimization suggestions

### Phase 9: Basic Collaboration (v1.1)
- Shared mosaics
- Comments and annotations
- Version history

### Phase 10: JavaScript Plugins (v1.2)
- JavaScript/TypeScript plugin support
- npm ecosystem compatibility
- React component plugins

### Phase 11: WASM & Advanced (v1.3)
- WASM plugin runtime
- Cross-language plugin support
- Advanced plugin composition

### Phase 12: Real-time Collaboration (v1.4+)
- CRDT-based real-time editing
- Collaborative Excalidraw boards
- Presence awareness
- Session recording

### Future Considerations
- **Performance**: Incremental indexing, parallel query execution
- **Graph Visualization**: Advanced graph layouts and 3D visualization (like Obsidian's graph view)
- **Code Execution**: Babel-like code blocks with sandboxed runtimes
- **Federation**: ActivityPub for public notes
- **Export**: Pandoc integration, static site generation
- **Analytics**: Local knowledge graph metrics
- **Plugin Security**: Move from Lua to WASM for better sandboxing
- **Advanced Caching**: Distributed cache for multi-instance deployments