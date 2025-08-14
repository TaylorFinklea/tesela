# Tesela Development Plan

## Development Strategy

### Core Philosophy
- **Local-first**: Full functionality without internet
- **Cross-platform from day one**: macOS, Linux, Windows
- **Progressive enhancement**: Basic features first, advanced features later
- **Community-driven**: Open source, plugin-friendly

### Phased Rollout
1. **Phase 1**: Local-only, cross-platform foundation
2. **Phase 2**: Platform-native sync (iCloud, then others)
3. **Phase 3**: Custom sync protocol with collaboration
4. **Phase 4**: AI features and advanced plugins

**Note: All versions before 1.0 are considered alpha releases**

## Version 0.1 - Core Foundation

### Phase 1: Project Setup
- [ ] Workspace structure with Cargo.toml
- [ ] Core library crate (`tesela-core`)
- [ ] Error handling with `thiserror` and `anyhow`
- [ ] Logging with `tracing` and `tracing-subscriber`
- [ ] Basic CI with GitHub Actions
- [ ] AGPL-3.0 License and Contributing guidelines

### Phase 2: Storage Layer
- [ ] Note structure definition
- [ ] Markdown parsing with `pulldown-cmark`
- [ ] Frontmatter handling with `matter` or custom parser
- [ ] File operations module
- [ ] Path validation and normalization
- [ ] Attachment storage structure
- [ ] File type detection (mime types)
- [ ] Attachment copy/move operations
- [ ] Unit tests for file operations

### Phase 3: Database Foundation
- [ ] SQLite setup with `sqlx`
- [ ] Schema creation (notes, blocks, links, tags, attachments)
- [ ] FTS5 virtual table setup
- [ ] Basic CRUD operations
- [ ] Attachment metadata storage
- [ ] Connection pool configuration
- [ ] Database tests with fixtures

### Phase 4: Basic CLI
- [x] CLI structure with `clap`
- [x] `tesela init` - Initialize mosaic with clean folder structure
- [x] `tesela new [title]` - Create note
- [x] `tesela list` - List recent notes
- [x] `tesela cat [id]` - Display note
- [ ] `tesela attach [note] [file]` - Attach file to note
- [ ] `tesela export [note] [format]` - Export note (markdown, HTML)
- [x] Configuration file support (TOML)
- [ ] Basic undo/redo system design
- [x] Mosaic structure: single folder with notes/ and attachments/ subdirectories
- [x] All files in one place - ready for any sync tool (Syncthing, Dropbox, etc.)

### Phase 4.5: Testing Foundation
- [x] Unit tests for `init` command
- [x] Unit tests for `new` command  
- [x] Unit tests for `list` command
- [x] Integration tests for CLI workflow (12 tests passing)
- [x] Test helper utilities for file operations
- [x] Test fixtures for mock notes/mosaics
- [x] Fixed macOS linking issues with .cargo/config.toml
- [ ] CI pipeline for automated testing (GitHub Actions workflow created)

### Phase 4.6: Documentation Foundation
- [x] Project structure documentation (STRUCTURE.md)
- [x] Module-level documentation in source files
- [x] Inline function documentation with examples
- [x] Developer documentation section in README
- [x] Architecture documentation maintained
- [x] Development roadmap tracking (this file)

## Version 0.2 - Indexing & Search

### Phase 1: File Watcher & Indexer
- [ ] File watcher with `notify`
- [ ] Incremental indexing logic
- [ ] Link extraction from Markdown
- [ ] Tag parsing from frontmatter
- [ ] Attachment reference parsing
- [ ] Checksum-based change detection
- [ ] Index rebuild command

### Phase 2: Query Engine
- [ ] Full-text search implementation
- [ ] Tag-based filtering
- [ ] Date range queries
- [ ] Link queries (backlinks/forward)
- [ ] Query result ranking
- [ ] Search benchmarks

### Phase 3: Cache Layer
- [ ] LRU cache implementation
- [ ] Cache invalidation logic
- [ ] Configurable memory limits
- [ ] Hot path optimization
- [ ] Cache metrics/debugging

## Version 0.3 - Enhanced CLI & API

### Phase 1: Advanced CLI Features
- [ ] `tesela search [query]` - Full-text search
- [ ] `tesela link [from] [to]` - Create links
- [ ] `tesela graph [note]` - Show connections
- [ ] `tesela daily` - Daily note
- [ ] `tesela backup` - Create mosaic backup
- [ ] `tesela import [file/directory]` - Import from other formats
- [ ] Interactive mode with `dialoguer`
- [ ] Shell completions
- [ ] Basic performance benchmarks

### Phase 2: Core API Layer
- [ ] Async trait definitions
- [ ] In-process adapter implementation
- [ ] Event bus with `tokio::sync`
- [ ] Request/response types
- [ ] API documentation
- [ ] Integration tests

### Phase 3: Performance & Polish
- [ ] Benchmark suite with `criterion`
- [ ] Memory profiling
- [ ] Query optimization
- [ ] Error message improvements
- [ ] Progress indicators
- [ ] First alpha release

## Version 0.4 - Desktop UI

### Phase 1: Slint Foundation
- [ ] Slint project setup
- [ ] Main window layout
- [ ] Note list component
- [ ] Editor component with syntax highlighting
- [ ] Search interface
- [ ] Drag & drop attachment support
- [ ] Inline image preview
- [ ] PDF viewer component
- [ ] Keyboard navigation
- [ ] Accessibility foundations (screen reader support)
- [ ] Undo/redo implementation
- [ ] Find and replace

### Phase 2: Desktop Features
- [ ] Graph visualization
- [ ] Tag sidebar
- [ ] Quick switcher (Cmd+P style)
- [ ] Settings panel
- [ ] Theme support (light/dark)
- [ ] Auto-save and conflict detection

## Version 0.5 - Plugin System Foundation

### Phase 1: Lua & Fennel Runtime
- [ ] Security model design document
- [ ] Capability-based permissions system
- [ ] Lua integration with `mlua`
- [ ] Fennel compiler integration
- [ ] Plugin manifest format (TOML)
- [ ] Sandboxed environment setup
- [ ] Resource limits (CPU, memory, disk)
- [ ] Core API bindings for Lua and Fennel
- [ ] Plugin discovery and loading
- [ ] Error handling and recovery

### Phase 2: Plugin API
- [ ] Event hooks implementation
- [ ] Permission system
- [ ] Rate limiting
- [ ] Plugin storage API
- [ ] UI extension points
- [ ] Hot reload support

### Phase 3: Basic Plugin Infrastructure
- [ ] Plugin manager UI
- [ ] Plugin installation/removal
- [ ] Plugin settings interface
- [ ] Plugin debugging tools
- [ ] Plugin documentation system

## Version 0.6 - Example Plugins & Excalidraw

### Phase 1: Core Example Plugins
- [ ] Auto-tagger plugin (Lua)
- [ ] Daily summary plugin (Fennel)
- [ ] TODO extractor
- [ ] Word count stats
- [ ] Plugin documentation
- [ ] Plugin template repository (Lua & Fennel examples)

### Phase 2: Excalidraw Integration
- [ ] Embed Excalidraw as web component
- [ ] Save drawings as .excalidraw files
- [ ] Inline rendering in notes
- [ ] Whiteboard features
- [ ] Create drawing from note
- [ ] Link drawings to notes
- [ ] Export to PNG/SVG

### Phase 3: Advanced Plugin Examples
- [ ] Citation manager plugin
- [ ] Calendar integration plugin
- [ ] Pomodoro timer plugin
- [ ] Note templates plugin

## Version 0.7 - Code Execution in Notes

### Phase 1: Code Block Infrastructure
- [ ] Code block detection and parsing
- [ ] Sandboxed execution environment
- [ ] Basic language support:
  - [ ] Shell/Bash (using `std::process::Command`)
  - [ ] Lua (reuse plugin runtime)
  - [ ] SQL (for note queries)

### Phase 2: Execution Features
- [ ] Execution controls:
  - [ ] Run button in UI
  - [ ] Keyboard shortcuts (Ctrl+Enter)
  - [ ] Safety confirmations
- [ ] Output handling:
  - [ ] Capture stdout/stderr
  - [ ] Inline results display
  - [ ] Error formatting
- [ ] Variables and state:
  - [ ] Pass values between code blocks
  - [ ] Session persistence
  - [ ] Export results as note content

### Phase 3: Advanced Languages & Security
- [ ] Additional language support:
  - [ ] Python (via `pyo3` or subprocess)
  - [ ] JavaScript (via QuickJS)
  - [ ] R (for data analysis)
- [ ] Security hardening:
  - [ ] Execution timeout limits
  - [ ] Memory limits
  - [ ] Network access control
  - [ ] File system sandboxing
  - [ ] Per-language permission model

## Version 0.8 - Mobile Apps

### Phase 1: Core Mobile Experience
- [ ] Slint mobile shell (iOS/Android)
- [ ] Touch-optimized UI
- [ ] Note editor with mobile keyboard support
- [ ] Quick capture
- [ ] Search and navigation
- [ ] Attachment support (camera, gallery)
- [ ] Offline-first architecture
- [ ] Note: Mosaic folder can be synced with Syncthing/Dropbox from day one

### Phase 2: Platform Integration
- [ ] iOS specific features
  - [ ] Share extension
  - [ ] Widgets
  - [ ] Handoff support
- [ ] Android specific features
  - [ ] Quick settings tile
  - [ ] Intent filters
  - [ ] Material You theming

### Phase 3: Mobile Optimization
- [ ] Performance tuning
- [ ] Battery optimization
- [ ] Storage management
- [ ] Background sync
- [ ] Push notifications for reminders

## Version 0.9 - Platform Sync

### Phase 1: P2P Sync
- [ ] Native sync protocol for real-time updates
- [ ] Device discovery and pairing
- [ ] End-to-end encryption by default
- [ ] Selective folder sync
- [ ] Conflict detection
- [ ] Merge strategies
- [ ] Better than file-based sync (instant updates, less conflicts)

### Phase 2: Platform-Specific Sync
- [ ] iCloud Integration (macOS/iOS)
  - [ ] CloudKit setup
  - [ ] iOS app shell (Slint)
  - [ ] Handoff support
- [ ] Cross-platform options
  - [ ] Dropbox integration
  - [ ] OneDrive support
  - [ ] Google Drive support

### Phase 3: Optional Server Sync
- [ ] Self-hosted sync server
- [ ] S3-compatible backend support
- [ ] WebDAV implementation
- [ ] Multi-device sync orchestration
- [ ] Sync status UI
- [ ] Conflict resolution UI

## Version 1.0 - Intelligence

**Note: Plugin API becomes stable at v1.0 - backwards compatibility guaranteed from here**

### Phase 1: Local AI
- [ ] Ollama integration
- [ ] Embedding generation
- [ ] Semantic search
- [ ] Similar note suggestions
- [ ] Smart tag recommendations
- [ ] Privacy-preserving design

### Phase 2: External AI
- [ ] OpenAI integration (optional)
- [ ] Anthropic support
- [ ] Content summarization
- [ ] Writing assistance
- [ ] API key management
- [ ] Cost tracking

### Phase 3: AI Features
- [ ] Smart linking suggestions
- [ ] Duplicate detection
- [ ] Knowledge gaps identification
- [ ] Daily digest generation
- [ ] Q&A over notes
- [ ] AI feature toggles
- [ ] Excalidraw AI features
  - [ ] Sketch to diagram conversion
  - [ ] Handwriting recognition
  - [ ] Smart shape suggestions

## Version 1.1 - Basic Collaboration

### Phase 1: Shared Mosaics
- [ ] Mosaic sharing mechanism
- [ ] Permission model
- [ ] Basic conflict resolution
- [ ] Change tracking
- [ ] User management

### Phase 2: Collaboration Features
- [ ] Comments and annotations
- [ ] Version history
- [ ] Public note sharing
- [ ] Collaboration UI
- [ ] Basic presence indicators

## Version 1.2 - JavaScript Plugins

### Phase 1: JavaScript Runtime
- [ ] QuickJS integration
- [ ] TypeScript support
- [ ] npm package compatibility
- [ ] Async/await support
- [ ] Plugin bundling
- [ ] Dev tools

### Phase 2: Plugin Ecosystem
- [ ] Plugin marketplace UI
- [ ] GitHub-based registry
- [ ] Plugin reviews/ratings
- [ ] Automatic updates
- [ ] Plugin analytics
- [ ] Revenue sharing setup

### Phase 3: Advanced JavaScript Features
- [ ] React component plugins
- [ ] Custom UI panels
- [ ] Advanced templates
- [ ] External API integrations
- [ ] TypeScript type definitions for plugin API
- [ ] Plugin debugging tools

## Version 1.3 - WASM & Advanced Extensibility

### Phase 1: WASM Runtime
- [ ] Wasmtime integration
- [ ] WASI support
- [ ] Component model
- [ ] Language bindings (Rust, Go, C++)
- [ ] WASM plugin examples

### Phase 2: Cross-Language Plugins
- [ ] Polyglot plugin support
- [ ] FFI bridge for native extensions
- [ ] Performance-critical plugins
- [ ] Advanced sandboxing

### Phase 3: Power User Features
- [ ] Custom note types via WASM
- [ ] Workflow automation engine
- [ ] External tool integration
- [ ] Plugin composition and chaining

## Version 1.4 - Real-time Collaboration & Production Polish

### Phase 1: CRDT Implementation
- [ ] CRDT-based real-time editing
- [ ] WebSocket server
- [ ] Real-time sync engine
- [ ] Cursor presence
- [ ] Collaborative whiteboards
  - [ ] Real-time drawing sync
  - [ ] Multi-user cursors
  - [ ] Drawing permissions

### Phase 2: Advanced Collaboration
- [ ] Operational transformation fallback
- [ ] Branching and merging
- [ ] Suggested edits
- [ ] Real-time commenting
- [ ] Session recording/playback

### Phase 3: Production Ready

### Phase 1: Polish & Performance
- [ ] Performance audit
- [ ] Memory leak fixes
- [ ] UI responsiveness
- [ ] Complete accessibility audit (WCAG 2.1 AA)
- [ ] Internationalization (i18n)
- [ ] Final benchmarks
- [ ] Security audit
- [ ] Documentation review

### Phase 2: Launch Preparation
- [ ] Documentation website
- [ ] Video tutorials
- [ ] Migration tools (from Obsidian/Logseq)
- [ ] Backup/restore tools
- [ ] Community Discord/Matrix
- [ ] Launch blog post

## Success Metrics

### Performance Targets
- [ ] < 100ms search latency for 10k notes
- [ ] < 50MB memory for 10k notes
- [ ] < 5s cold index rebuild for 10k notes
- [ ] < 200ms app startup time
- [ ] 60fps UI animations

### Quality Targets
- [ ] 80% test coverage
- [ ] Zero data loss in fuzz tests
- [ ] < 1% crash rate
- [ ] A11y compliance (WCAG 2.1 AA)
- [ ] All unwrap() calls removed

### Community Targets (Year 1)
- [ ] 100 GitHub stars
- [ ] 10 contributors
- [ ] 5 third-party plugins
- [ ] 1,000 monthly active users
- [ ] Regular release cycle established

## Development Practices

### Code Quality
- Rust formatting with `rustfmt`
- Linting with `clippy`
- Pre-commit hooks
- Conventional commits
- PR templates
- Code review required

### Testing Strategy
- Unit tests for all modules
- Integration tests for workflows
- Property-based testing for parsers
- Fuzz testing for file operations
- UI testing with Slint tools
- Manual testing checklist

### Release Process
1. Version bump in Cargo.toml
2. Update CHANGELOG.md
3. Run full test suite
4. Build all platforms
5. Create GitHub release
6. Publish to crates.io
7. Update package managers
8. Announce on social media

### Distribution Channels
- GitHub releases (all platforms)
- crates.io (Rust library)
- Homebrew (macOS)
- AUR (Arch Linux)
- Flathub (Linux)
- Microsoft Store (Windows)
- Mac App Store (later)

## Risk Management

### Technical Risks
- **File corruption**: Atomic writes, backups, checksums
- **Performance degradation**: Continuous benchmarking, profiling
- **Platform differences**: Extensive testing, CI matrix
- **Dependency issues**: Minimal deps, vendoring critical ones

### Community Risks
- **Toxic behavior**: Code of Conduct, moderation
- **Burnout**: Sustainable pace, delegate early
- **Fork fragmentation**: Clear governance, inclusive process
- **Corporate capture**: Stay independent, diverse funding

## Long-term Vision

### Year 2+ Goals
- Mobile apps (iOS/Android)
- Web version (WASM)
- Plugin marketplace
- Educational content
- Sustainable funding model (donations/sponsorship)

### Year 3+ Ideas
- Academic features (citations, LaTeX)
- Voice notes with transcription
- Handwriting support
- VR/AR interfaces
- Federation protocol
- Tesela Foundation (governance)