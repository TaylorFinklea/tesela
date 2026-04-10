# Tesela

A keyboard-first note-taking system built in Rust with a web frontend. Tesela combines a block outliner, Vim-style editing, a page-based type system, and a local REST/WebSocket server so the UI and backend can evolve independently.

<!-- Screenshot goes here -->

> **Work in progress** — this is Taylor's daily-driver tool. Reliability matters more than shipping a wide feature surface.

## Architecture

Tesela is a Cargo workspace plus a Next.js web client in `web/`.

| Crate | Binary | Purpose |
|-------|--------|---------|
| `tesela-core` | — | Foundation: types, traits, storage, SQLite/FTS5, indexer |
| `tesela-cli` | `tesela` | Thin dispatcher; all subcommands via `clap` |
| `tesela-tui` | `tesela-tui` | Elm-style TUI (ratatui/crossterm) |
| `tesela-mcp` | `tesela-mcp` | MCP server over JSON-RPC 2.0 on stdin/stdout |
| `tesela-server` | `tesela-server` | REST API + WebSocket on localhost:7474 |
| `tesela-plugins` | — | Lua runtime (working) + WASM stub |

The web client talks to `tesela-server` at `localhost:7474` over REST and WebSocket. UI stays thin: note storage, search, links, indexing, and type resolution live in `tesela-core` and are exposed through traits such as `NoteStore`, `SearchIndex`, and `LinkGraph`.

**Core principle:** database-first, files are export format.

## Type System

Tesela models schema as content:

- **Tag pages** are notes with frontmatter like `type: "Tag"`, `extends`, and `tag_properties`.
- **Property pages** are notes with frontmatter like `type: "Property"`, `value_type`, `choices`, and `default`.
- **Blocks** inherit schema from their tags and store concrete values with inline `key:: value` properties.

Built-in tags include `Task`, `Project`, `Person`, `Domain`, `LifeProject`, `Issue`, `Ritual`, and `ScheduledItem`.

## Build

Requires Rust (stable toolchain), Node 20+, and pnpm.

```bash
# Build the full Rust workspace
cargo build --workspace

# Run the Rust test suite
cargo test --workspace

# Lint with warnings as errors
cargo clippy --workspace -- -D warnings

# Format Rust sources
cargo fmt --all

# Install web client dependencies
pnpm --dir web install
```

## Run

```bash
# Install the server binary if you want to run it manually
cargo install --path crates/tesela-server

# Start the local API server from a Tesela mosaic
tesela-server

# In another terminal, start the web client
pnpm --dir web dev
```

Then open `http://localhost:3000`.

## Development

```bash
# Rust workspace
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all

# Web client
pnpm --dir web dev        # dev server on localhost:3000
pnpm --dir web tsc --noEmit
pnpm --dir web lint

# Regenerate TypeScript types from Rust (writes web/src/lib/types/)
cargo test -p tesela-core --lib export_bindings
```

CI runs from `.github/workflows/ci.yml` and covers formatting, linting, and tests for the Rust workspace.

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
