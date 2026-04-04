# Tesela

A keyboard-first note-taking system built in Rust with a native macOS SwiftUI app. Tesela combines a block outliner, Vim-style editing, a page-based type system, and a local REST/WebSocket server so the native UI and Rust backend can evolve independently.

<!-- Screenshot goes here -->

> **Work in progress** — this is Taylor's daily-driver tool. Reliability matters more than shipping a wide feature surface.

## Current Features

- **Block outliner with Vim keybindings** — Normal/Insert/Visual mode, motions, operators, dot-repeat
- **Inline autocomplete** for `#tags` and `[[page refs]]`, including "New tag" creation
- **Type system** — tags and properties are pages, with inheritance chains and typed block filtering
- **Custom bullet icons** — SF Symbols with per-type color picker
- **Tag page views** — table and kanban with multi-property filtering, drag-and-drop, sortable columns
- **Back/forward navigation** and block drill-in (Logseq-style zoom)
- **Search** with highlighting, match counts, and `n`/`N` navigation
- **Daily tiles timeline** with inline editing
- **Right sidebar** — page info, grouped backlinks with context, unlinked references, focused block properties
- **Graph view** for note-link relationships
- **Dark/light/auto theme** and accent color customization
- **Embedded server management** in the macOS app, with CLI LaunchAgent support as a fallback
- **Backup system** — `tesela backup` CLI + auto-daily on server startup
- **MCP server** — AI integration via `search_notes`, `get_note`, `create_note`, `list_notes`, `get_backlinks`, `get_daily_note`

## Architecture

Tesela is a Cargo workspace plus a native SwiftUI macOS app in `app/Tesela/`.

| Crate | Binary | Purpose |
|-------|--------|---------|
| `tesela-core` | — | Foundation: types, traits, storage, SQLite/FTS5, indexer |
| `tesela-cli` | `tesela` | Thin dispatcher; all subcommands via `clap` |
| `tesela-tui` | `tesela-tui` | Elm-style TUI (ratatui/crossterm) |
| `tesela-mcp` | `tesela-mcp` | MCP server over JSON-RPC 2.0 on stdin/stdout |
| `tesela-server` | `tesela-server` | REST API + WebSocket on localhost:7474 |
| `tesela-plugins` | — | Lua runtime (working) + WASM stub |

The SwiftUI app talks to `tesela-server` at `localhost:7474` over REST and WebSocket. UI layers stay thin: note storage, search, links, indexing, and type resolution live in `tesela-core` and are exposed through traits such as `NoteStore`, `SearchIndex`, and `LinkGraph`.

**Core principle:** database-first, files are export format.

## Type System

Tesela models schema as content:

- **Tag pages** are notes with frontmatter like `type: "Tag"`, `extends`, and `tag_properties`.
- **Property pages** are notes with frontmatter like `type: "Property"`, `value_type`, `choices`, and `default`.
- **Blocks** inherit schema from their tags and store concrete values with inline `key:: value` properties.

Built-in tags include `Task`, `Project`, `Person`, `Domain`, `LifeProject`, `Issue`, `Ritual`, and `ScheduledItem`.

## Build

Requires Rust (stable toolchain) and Xcode.

```bash
# Build the full Rust workspace
cargo build --workspace

# Run the Rust test suite
cargo test --workspace

# Lint with warnings as errors
cargo clippy --workspace -- -D warnings

# Format Rust sources
cargo fmt --all

# Build the macOS app
xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build
```

## Run

```bash
# Install the server binary if you want to run it manually
cargo install --path crates/tesela-server

# Start the local API server from a Tesela mosaic
tesela-server
```

You can also launch the macOS app from Xcode by opening `app/Tesela/Tesela.xcodeproj`. The app will try to connect to an existing server and can start `tesela-server` itself when the binary is available locally.

## Development

```bash
# Rust workspace
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all

# Swift app
xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build
```

CI runs from `.github/workflows/ci.yml` and covers formatting, linting, and tests.

## Note Format

Markdown with YAML frontmatter and block-based outliner structure:

```markdown
---
title: "My Note"
created: 2025-01-15T10:30:00Z
tags: ["Project"]
---
- Ship the docs refresh #Task
  status:: doing
  priority:: high
  deadline:: [[2026-04-10]]
  - Child block
```

## License

AGPL-3.0 — see [LICENSE](LICENSE) for details.
