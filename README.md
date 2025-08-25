# Tesela 🗿

> A keyboard-first, file-based note-taking system for building lasting knowledge mosaics. Think Emacs org-mode meets modern UI/UX, with first-class backlinks and multimodal support.

Tesela transforms your notes into an interconnected knowledge graph while keeping you in control of your data. Built in Rust for performance, designed for longevity.

## 🚧 Work in Progress 🚧

This is my passion project - the note-taking application I've always dreamed of building. After years of jumping between different tools and always finding something missing, I decided to create the perfect blend of my favorite features from NeoVim, Emacs, Logseq, and others.

I'm using this project to learn Rust, a language I'm new to, so expect some rookie mistakes along the way! Contributions, suggestions, and patience are all greatly appreciated as I learn and improve the codebase.

## ✨ Key Features

- **Keyboard-First**: Navigate, search, and edit without touching the mouse
- **Your Files, Your Control**: Plain Markdown files, no proprietary formats
- **Offline-First**: Full functionality without internet, sync when you want
- **Fast**: Sub-100ms search across thousands of notes
- **Extensible**: Plugin system for custom workflows
- **Privacy-Respecting**: Your notes never leave your device unless you want them to
- **Multimodal**: Native attachments - drag & drop PDFs, images, and files just like modern apps
- **Modern Yet Efficient**: Zed-like balance of contemporary features with keyboard-first speed
- **Outliner-Based**: Block-structured notes with hierarchical inheritance for powerful organization

## 🤔 Why Tesela?

In a world of cloud-based, proprietary note apps that lock in your data, Tesela takes a different approach:

1. **Files as Truth**: Your notes are just Markdown files. Use any editor, sync any way, never lose access
2. **Database as Cache**: SQLite index for fast queries, but always rebuildable from your files
3. **Progressive Enhancement**: Start simple, add features as you need them
4. **Community-Driven**: Open source from day one, designed for extensibility
5. **AI-Native Architecture**: Built from the ground up with AI as a first-class feature - not bolted on as an afterthought. Intelligence is woven into the core design, but always optional and privacy-respecting
6. **Truly Multimodal**: Unlike traditional text editors, attachments are first-class citizens. Drag in PDFs, images, or any file - they're organized automatically and render inline

Think of it as Emacs org-mode reimagined for the modern era - keeping the power and extensibility while adding native backlinks, multimodal support, and a UI that doesn't require a PhD to navigate. Like how Zed modernized code editing, Tesela modernizes note-taking.

## 📦 Installation

### Release Strategy

Tesela uses a date-based versioning strategy to facilitate frequent and small releases:
- Format: `{major-version}.{YYYY}{MM}{DD}.{build_number}`
- Example: `0.20250825.0` (first release of the day) or `0.20250825.1` (second release of the day)
- Releases are automatically created on every commit to the main branch
- Each release includes pre-built binaries for Linux, macOS, and Windows

### Latest Release

You can always get the latest release from the [releases page](https://github.com/your-username/tesela/releases/latest).

### Quick Install (Linux/macOS)

```bash
# Download latest release (replace VERSION with actual version)
curl -L https://github.com/your-username/tesela/releases/latest/download/tesela-$(uname -s | tr '[:upper:]' '[:lower:]')-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

### From Source

```bash
# Clone and install from source (Rust required)
git clone https://github.com/your-username/tesela
cd tesela
cargo install --path .
```

## 🚀 Quick Start

> ⚠️ **Early Development**: Tesela is actively being developed! Features are being added as time permits - this is built with love in my free time, so development pace may vary. As I'm learning Rust while building this, the architecture and implementation will evolve as I gain experience.

```bash
# Initialize a mosaic
tesela init ~/my-knowledge-base
cd ~/my-knowledge-base

# Create your first note
tesela -n "My First Note"

# Create a daily note
tesela -d

# Edit any note (works across all directories)
tesela -e "First"

# Search your notes
tesela -s "knowledge"

# TUI mode for full interactive functionality
tesela tui
```

## 🏗️ Architecture

Tesela follows the **Island Core** pattern - a headless core with multiple UI shells:

```
┌─────────────┐  ┌─────────────┐  ┌─────────────┐
│   CLI/TUI   │  │   Desktop   │  │   Mobile    │
│   (Rust)    │  │   (Slint)   │  │   (Slint)   │
└──────┬──────┘  └──────┬──────┘  └──────┬──────┘
       │                │                │
       └────────────────┴────────────────┘
                       │
              ┌────────┴────────┐
              │   Core Engine   │
              │     (Rust)      │
              └────────┬────────┘
                       │
        ┌──────────────┼──────────────┐
        │              │              │
   ┌────┴────┐  ┌──────┴──────┐  ┌───┴───┐
   │Notes &  │  │   SQLite    │  │Plugins│
   │Dailies  │  │   Index     │  │ (Lua) │
   └─────────┘  └─────────────┘  └───────┘
```

## 🗺️ Development Roadmap

> **Note**: Tesela follows an incremental development approach with continuous delivery. Features are released as they're completed, using date-based versioning (e.g., `v2024.01.15`).

### ✅ Phase 1: Foundation (v0.1-0.3) - COMPLETE
**Core Infrastructure & CLI**
- [x] Project structure with outliner-based notes
- [x] Organized directories (`notes/`, `dailies/`, `attachments/`)
- [x] SQLite indexing with full-text search
- [x] Complete CLI with intelligent autocomplete
- [x] Cross-directory operations (edit, search, list)
- [x] Daily notes with date-based organization
- [x] TUI with preview, search, and graph/backlinks view
- [x] Smart note discovery with title-to-filename mapping
- [x] Comprehensive test suite with 39+ tests

### 🚧 Phase 2: Desktop Experience (v0.4) - IN PROGRESS
**Native GUI Application**
- [ ] Slint-based desktop UI with native performance
- [ ] Drag & drop file attachments
- [ ] Visual graph view with block relationships
- [ ] Inline PDF and image previews
- [ ] Quick switcher across all directories
- [ ] Themes and customization options
- [ ] Block-based editing with inheritance visualization

### 📋 Phase 3: Plugin System (v0.5)
**Extensibility Foundation**
- [ ] Lua/Fennel plugin runtime with sandboxing
- [ ] Plugin API with security permissions
- [ ] Core example plugins (auto-tagger, daily summary)
- [ ] Plugin marketplace and distribution
- [ ] Block-aware plugin operations
- [ ] Cross-directory plugin support

### 📋 Phase 4: Enhanced Features (v0.6)
**Productivity & Integration**
- [ ] Excalidraw integration for whiteboarding
- [ ] Code execution in notes (like org-babel)
- [ ] Advanced plugin examples
- [ ] Smart templates with outliner inheritance
- [ ] Enhanced graph visualization options

### 📋 Phase 5: Mobile Apps (v0.7)
**Cross-Platform Access**
- [ ] iOS app with touch-optimized outliner editing
- [ ] Android app with platform integrations
- [ ] Quick capture and offline sync
- [ ] Cross-device file synchronization
- [ ] Mobile-specific UI patterns

### 📋 Phase 6: Sync & Collaboration (v0.8)
**Multi-Device & Team Features**
- [ ] Native sync protocol with conflict resolution
- [ ] Platform-specific sync (iCloud, Google Drive, Syncthing)
- [ ] Optional self-hosted sync server
- [ ] Shared mosaics and collaborative editing
- [ ] Real-time collaboration with CRDT
- [ ] Version history and change tracking

### 📋 Phase 7: Intelligence (v0.9)
**AI-Powered Features**
- [ ] Local AI integration (Ollama, local LLMs)
- [ ] Smart linking suggestions with block inheritance
- [ ] Content summarization across directories
- [ ] AI-powered search and discovery
- [ ] Context-aware content suggestions
- [ ] Block-level AI assistance

### 📋 Phase 8: Advanced Extensibility (v1.0+)
**Enterprise & Power User Features**
- [ ] JavaScript/TypeScript plugin support
- [ ] WASM plugin runtime for any language
- [ ] Advanced collaboration features
- [ ] Enterprise deployment options
- [ ] Advanced analytics and insights
- [ ] Federation and public knowledge sharing

### 🎯 Current Focus
**Immediate Next Steps:**
1. **Desktop GUI**: Slint-based application with file management
2. **Graph Enhancement**: Visual graph view in desktop app
3. **Plugin Foundation**: Lua runtime with basic API

### 📊 Success Metrics
**Performance Targets:**
- Search 10k notes: < 100ms (current: ~50ms) ✅
- Open note: < 50ms (current: ~10ms) ✅
- Index 10k notes: < 5s (current: ~2s) ✅
- Memory usage: < 50MB (current: ~15MB) ✅

**Community Goals (Year 1):**
- 1,000+ GitHub stars
- 50+ active contributors
- 100+ plugins created
- 10,000+ daily active users

> **Development Philosophy**: Each phase builds incrementally on the previous one, with working software delivered continuously. As a passion project, timelines are estimates - quality and sustainability take priority over speed.

See [ARCHITECTURE.md](ARCHITECTURE.md) for technical details and [plan.md](plan.md) for detailed task tracking.

## 🧩 Plugin Ideas

Tesela's plugin system will enable:

- **Auto-tagging** based on content
- **Daily summaries** of your notes
- **Citation management** for researchers
- **TODO extraction** across all notes
- **Custom note types** (recipes, contacts, etc.)
- **External integrations** (calendar, email, etc.)

## 🤝 Contributing

We'd love your help making Tesela better!

### Ways to Contribute

- **Code**: Pick an issue labeled `good-first-issue`
- **Plugins**: Build and share your workflows
- **Documentation**: Help others get started
- **Testing**: Break things and tell us how
- **Ideas**: Share your use cases and needs

### Development Setup

```bash
# Clone the repo
git clone https://github.com/yourusername/tesela
cd tesela

# Install Rust (if needed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Build and test
cargo build
cargo test

# Run locally
cargo run -- init ~/test-mosaic
```

### Developer Documentation

- **[STRUCTURE.md](STRUCTURE.md)**: Complete guide to the project file structure
- **[architecture.md](architecture.md)**: System design and architectural decisions
- **[plan.md](plan.md)**: Development roadmap with task tracking

For new contributors, start by reading `STRUCTURE.md` to understand how the codebase is organized and what each file does.

### Code Style

- Follow Rust conventions
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Write tests for new features

## 📊 Benchmarks

Performance targets for production release:

| Operation | Target | Current |
|-----------|--------|---------|
| Search 10k notes | < 100ms | ~50ms |
| Open note | < 50ms | ~10ms |
| Index 10k notes | < 5s | ~2s |
| Memory usage | < 50MB | ~15MB |
| Cross-directory edit | < 100ms | ~25ms |

## 🌟 Inspiration

Tesela draws inspiration from many great tools:

- **[NeoVim](https://github.com/neovim/neovim)**: Keyboard-first design and extensibility
- **[Logseq](https://github.com/logseq/logseq)**: Block references and daily notes
- **[Emacs Org-mode](https://git.savannah.gnu.org/cgit/emacs/org-mode.git)**: Plain text power and flexibility (but with modern UX)
- **[Zed](https://github.com/zed-industries/zed)**: Performance-first design, thoughtful AI integration, and perfect balance of modern features with efficiency
- **[Git](https://github.com/git/git)**: Version control concepts
- **[SQLite](https://github.com/sqlite/sqlite)**: Reliability and performance
- **[Obsidian](https://obsidian.md)**: The power of local-first files and a powerful plugin ecosystem

## 🔐 Privacy

- **No telemetry**: We don't track you
- **No account required**: It's your computer
- **No cloud**: Unless you add it yourself
- **Encrypted sync**: When we add sync, it's E2E encrypted

## 📜 License

AGPL-3.0 License - see [LICENSE](LICENSE) for details. This ensures that any derivatives remain open source and benefit the community.

## 💬 Community

- **Discord**: [Join our server](https://discord.gg/tesela) (coming soon)
- **Matrix**: #tesela:matrix.org (coming soon)
- **GitHub Discussions**: Share ideas and questions
- **Twitter**: [@tesela_app](https://twitter.com/tesela_app) (coming soon)

## 🏗️ Directory Structure

Tesela organizes your knowledge into a clean, sync-friendly structure:

```
my-knowledge-base/
├── tesela.toml        # Configuration
├── notes/             # Regular notes
│   └── *.md          # Topic-based notes
├── dailies/           # Daily notes
│   └── daily-*.md    # Time-based notes
└── attachments/       # File attachments
    └── files/        # PDFs, images, etc.
```

### Note Format (Outliner Structure)

Every note uses a minimal outliner format with block inheritance:

```markdown
---
title: "Project Planning"
created: 2025-01-15 10:30:00
last_opened: 2025-01-15 12:45:00
tags: ["work"]
---
- Project Alpha #urgent
  - Research phase
    - Literature review (inherits #urgent + "work")
    - Competitor analysis
  - Implementation
- Project Beta
  - Different approach (inherits "work" only)
```

## 🙏 Acknowledgments

Special thanks to the open source projects that make Tesela possible:

- [Rust](https://rust-lang.org) for the language
- [SQLite](https://sqlite.org) for the database
- [Slint](https://slint-ui.com) for the UI framework
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) for Markdown parsing

---

---

## ✨ Current Features

- **Complete CLI**: All core commands with intelligent autocomplete
- **Cross-Directory Operations**: Seamless editing/searching across `notes/` and `dailies/`
- **Outliner Format**: Block-based notes with hierarchical inheritance
- **Smart Search**: Full-text search with ranking and context
- **Daily Notes**: Separate organization with date-based structure
- **File Attachments**: Organized file management with drag & drop support
- **TUI Mode**: Beautiful Terminal User Interface for all operations
- **Shell Integration**: Tab completion for all major shells

**Note**: Tesela (Spanish/Portuguese for "tessellation piece") represents how individual notes form a complete knowledge mosaic. Each note is a tile in your personal knowledge architecture.
