# Tesela 🗿

> A keyboard-first, file-based note-taking system for building lasting knowledge mosaics

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

## 🤔 Why Tesela?

In a world of cloud-based, proprietary note apps that lock in your data, Tesela takes a different approach:

1. **Files as Truth**: Your notes are just Markdown files. Use any editor, sync any way, never lose access
2. **Database as Cache**: SQLite index for fast queries, but always rebuildable from your files
3. **Progressive Enhancement**: Start simple, add features as you need them
4. **Community-Driven**: Open source from day one, designed for extensibility
5. **AI-Native Architecture**: Built from the ground up with AI as a first-class feature - not bolted on as an afterthought. Intelligence is woven into the core design, but always optional and privacy-respecting

Think of it as combining the best of Obsidian, Logseq, and Roam, while staying true to simplicity, keyboard-first interaction, and user control.

## 🚀 Quick Start

> ⚠️ **Early Development**: Tesela is actively being developed! Features are being added as time permits - this is built with love in my free time, so development pace may vary. As I'm learning Rust while building this, the architecture and implementation will evolve as I gain experience.

```bash
# Install from source (Rust required)
git clone https://github.com/yourusername/tesela
cd tesela
cargo install --path .

# Initialize a vault
tesela init ~/notes

# Create your first note
tesela new "My First Note"

# Search your notes
tesela search "knowledge"

# Open daily note
tesela daily
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
   │Markdown │  │   SQLite    │  │Plugins│
   │  Files  │  │   Index     │  │ (Lua) │
   └─────────┘  └─────────────┘  └───────┘
```

## 🗺️ Roadmap

### Phase 1: Foundation (Current)
- [x] Project structure and planning
- [ ] Core file operations
- [ ] SQLite indexing
- [ ] Basic CLI

### Phase 2: Desktop Experience
- [ ] Slint-based GUI
- [ ] Graph visualization
- [ ] Quick switcher
- [ ] Themes

### Phase 3: Intelligence
- [ ] Plugin system (Lua)
- [ ] Smart linking
- [ ] Local AI integration
- [ ] Content suggestions

### Phase 4: Collaboration
- [ ] Sync protocol
- [ ] Mobile apps
- [ ] Shared vaults
- [ ] Real-time collaboration

See [plan.md](plan.md) for the development roadmap. Note that as a passion project, these are goals rather than deadlines - progress happens when life allows!

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
cargo run -- init ~/test-vault
```

### Code Style

- Follow Rust conventions
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix warnings
- Write tests for new features

## 📊 Benchmarks

Performance targets for v1.0:

| Operation | Target | Current |
|-----------|--------|---------|
| Search 10k notes | < 100ms | TBD |
| Open note | < 50ms | TBD |
| Index 10k notes | < 5s | TBD |
| Memory usage | < 50MB | TBD |

## 🌟 Inspiration

Tesela draws inspiration from many great tools:

- **[NeoVim](https://github.com/neovim/neovim)**: Keyboard-first design and extensibility
- **[Logseq](https://github.com/logseq/logseq)**: Block references and daily notes
- **[Emacs Org-mode](https://git.savannah.gnu.org/cgit/emacs/org-mode.git)**: Plain text power and flexibility
- **[Zed](https://github.com/zed-industries/zed)**: Performance-first design and thoughtful AI integration
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

## 🙏 Acknowledgments

Special thanks to the open source projects that make Tesela possible:

- [Rust](https://rust-lang.org) for the language
- [SQLite](https://sqlite.org) for the database
- [Slint](https://slint-ui.com) for the UI framework
- [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark) for Markdown parsing

---

**Note**: Tesela (Spanish/Portuguese for "tessellation piece") represents how individual notes form a complete knowledge mosaic. Each note is a tile in your personal knowledge architecture.
