# Tesela

A keyboard-first note-taking system built in Rust with a native macOS SwiftUI app. Block outliner with Vim-style editing, a type system inspired by Logseq DB mode, and a local REST/WebSocket server that keeps the UI and backend cleanly separated.

<!-- Screenshot goes here -->

> **Work in progress** — this is Taylor's daily-driver tool. Reliability matters more than features.

## Key Features

- **Block outliner with Vim keybindings** — Normal/Insert/Visual mode, motions, operators, dot-repeat
- **Inline autocomplete** for `#tags` and `[[page refs]]`, including "New tag" creation
- **Type system** — Tags, Properties as pages with inheritance chains (Logseq DB-inspired)
- **Custom bullet icons** — SF Symbols with per-type color picker
- **Tag page views** — table and kanban with multi-property filtering, drag-and-drop, sortable columns
- **Back/forward navigation** + block drill-in (Logseq-style zoom)
- **/search** with highlighting and `n`/`N` navigation
- **Daily tiles timeline** with inline editing
- **Right sidebar** — page info, grouped backlinks with context, unlinked references, focused block properties
- **Graph view**
- **Dark/light/auto theme** + accent color customization
- **Auto-start server** on app launch (LaunchAgent fallback for CLI use)
- **Backup system** — `tesela backup` CLI + auto-daily on server startup
- **MCP server** — AI integration via `search_notes`, `get_note`, `create_note`, `list_notes`, `get_backlinks`, `get_daily_note`

## Architecture

6 crates in `crates/` plus a native SwiftUI macOS app in `app/Tesela/`:

| Crate | Binary | Purpose |
|-------|--------|---------|
| `tesela-core` | — | Foundation: types, traits, storage, SQLite/FTS5, indexer |
| `tesela-cli` | `tesela` | Thin dispatcher; all subcommands via `clap` |
| `tesela-tui` | `tesela-tui` | Elm-style TUI (ratatui/crossterm) |
| `tesela-mcp` | `tesela-mcp` | MCP server over JSON-RPC 2.0 on stdin/stdout |
| `tesela-server` | `tesela-server` | REST API + WebSocket on localhost:7474 |
| `tesela-plugins` | — | Lua runtime (working) + WASM stub |

The SwiftUI app connects to `tesela-server` on `localhost:7474`. No business logic lives in the UI layers — only in `tesela-core` traits (`NoteStore`, `SearchIndex`, `LinkGraph`).

**Core principle:** Database-first. Files are export format.

## Build & Run

Requires Rust (stable) and Xcode.

```bash
# Build the full Rust workspace
cargo build --workspace

# Install the server so it's on your PATH
cargo install --path crates/tesela-server

# Open the macOS app in Xcode and run
open app/Tesela/Tesela.xcodeproj
```

The app expects `tesela-server` to be running on `localhost:7474`. Start it manually with `tesela-server` until the embedded-server feature lands.

## Development

```bash
# Run tests
cargo test --workspace

# Lint
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Regenerate Xcode project after adding files
cd app/Tesela && xcodegen generate
```

CI runs on every push/PR (`.github/workflows/ci.yml`): fmt + clippy + tests on Ubuntu and macOS.

## Note Format

Markdown with YAML frontmatter and block-based outliner structure:

```markdown
---
title: "My Note"
created: 2025-01-15T10:30:00Z
tags: ["example"]
---
- Top-level block #tag
  - Child block inherits #tag
  - Another child
```

## License

AGPL-3.0 — see [LICENSE](LICENSE) for details.
