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
- [ ] Frontmatter handling with `gray_matter`
- [ ] File operations module
- [ ] Path validation and normalization
- [ ] Unit tests for file operations

### Phase 3: Database Foundation
- [ ] SQLite setup with `sqlx`
- [ ] Schema creation (notes, blocks, links, tags)
- [ ] FTS5 virtual table setup
- [ ] Basic CRUD operations
- [ ] Connection pool configuration
- [ ] Database tests with fixtures

### Phase 4: Basic CLI
- [ ] CLI structure with `clap`
- [ ] `tesela init` - Initialize vault
- [ ] `tesela new [title]` - Create note
- [ ] `tesela list` - List recent notes
- [ ] `tesela cat [id]` - Display note
- [ ] Configuration file support (TOML)

## Version 0.2 - Indexing & Search

### Phase 1: File Watcher & Indexer
- [ ] File watcher with `notify`
- [ ] Incremental indexing logic
- [ ] Link extraction from Markdown
- [ ] Tag parsing from frontmatter
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
- [ ] Interactive mode with `dialoguer`
- [ ] Shell completions

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
- [ ] First beta release

## Version 0.4 - Desktop UI

### Phase 1: Slint Foundation
- [ ] Slint project setup
- [ ] Main window layout
- [ ] Note list component
- [ ] Editor component with syntax highlighting
- [ ] Search interface
- [ ] Keyboard navigation

### Phase 2: Desktop Features
- [ ] Graph visualization
- [ ] Tag sidebar
- [ ] Quick switcher (Cmd+P style)
- [ ] Settings panel
- [ ] Theme support (light/dark)
- [ ] Auto-save and conflict detection

## Version 0.5 - Plugin System

### Phase 1: Lua Runtime
- [ ] Lua integration with `mlua`
- [ ] Plugin manifest format (TOML)
- [ ] Sandboxed environment setup
- [ ] Core API bindings
- [ ] Plugin discovery and loading
- [ ] Error handling and recovery

### Phase 2: Plugin API
- [ ] Event hooks implementation
- [ ] Permission system
- [ ] Rate limiting
- [ ] Plugin storage API
- [ ] UI extension points
- [ ] Hot reload support

### Phase 3: Example Plugins
- [ ] Auto-tagger plugin
- [ ] Daily summary plugin
- [ ] TODO extractor
- [ ] Word count stats
- [ ] Plugin documentation
- [ ] Plugin template repository

## Version 0.6 - Platform Sync

### Phase 1: iCloud Integration (macOS/iOS)
- [ ] CloudKit setup
- [ ] Sync engine design
- [ ] Conflict detection
- [ ] Merge strategies
- [ ] iOS app shell (Slint)
- [ ] Handoff support

### Phase 2: Cross-Platform Sync
- [ ] File-based sync (Dropbox/Syncthing)
- [ ] S3-compatible backend support
- [ ] WebDAV implementation
- [ ] End-to-end encryption
- [ ] Sync status UI
- [ ] Conflict resolution UI

## Version 0.7 - Intelligence

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

## Version 0.8 - Collaboration

### Phase 1: Sync Protocol
- [ ] CRDT implementation
- [ ] WebSocket server
- [ ] Real-time sync engine
- [ ] Presence awareness
- [ ] Permission model
- [ ] Encryption layer

### Phase 2: Collaboration Features
- [ ] Shared vaults
- [ ] Comments and annotations
- [ ] Change tracking
- [ ] Version history
- [ ] Public note sharing
- [ ] Collaboration UI

## Version 0.9 - Advanced Plugins

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

### Phase 3: Power Features
- [ ] WASM plugin support
- [ ] Custom note types
- [ ] Advanced templates
- [ ] Workflow automation
- [ ] External tool integration
- [ ] Plugin composition

## Version 1.0 - Production Ready

### Phase 1: Polish & Performance
- [ ] Performance audit
- [ ] Memory leak fixes
- [ ] UI responsiveness
- [ ] Accessibility (a11y)
- [ ] Internationalization (i18n)
- [ ] Final benchmarks

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
- [ ] 1,000 GitHub stars
- [ ] 50 contributors
- [ ] 25 third-party plugins
- [ ] 10,000 monthly active users
- [ ] 5 core team members

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

### Year 2 Goals
- Mobile apps (iOS/Android)
- Web version (WASM)
- Enterprise features
- Paid cloud sync
- Plugin marketplace
- Educational content

### Year 3+ Ideas
- Academic features (citations, LaTeX)
- Voice notes with transcription
- Handwriting support
- VR/AR interfaces
- Federation protocol
- Tesela Foundation (governance)