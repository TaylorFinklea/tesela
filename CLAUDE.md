# Tesela — Claude Code Instructions

## After Any Work Session

**Always commit before stopping.** After completing a working chunk of changes:
1. `git add` the relevant files
2. `git commit` with a descriptive message
3. Do **not** push unless explicitly asked

**Always produce a QA checklist.** After implementing any user-facing TUI feature, output a step-by-step manual test plan the user can follow to verify the feature works end-to-end. Include:
- Exact key sequences to trigger each new feature
- Observable expected outcomes (what the user should see)
- Edge cases / Esc / cancel paths
- A short regression section covering anything that could have broken

## Project Overview

Tesela is a keyboard-first, file-based note-taking system (org-mode successor) written in Rust.
It's Taylor's daily-driver tool — reliability matters more than features.

**Core principle:** Files are truth, SQLite is cache.

## Workspace Structure

5 crates in `crates/`:

| Crate | Binary | Purpose |
|-------|--------|---------|
| `tesela-core` | — | Foundation: types, traits, storage, SQLite/FTS5, indexer |
| `tesela-cli` | `tesela` | Thin dispatcher; all subcommands via `clap` |
| `tesela-tui` | `tesela-tui` | Elm-style TUI (ratatui/crossterm) |
| `tesela-mcp` | `tesela-mcp` | MCP server over JSON-RPC 2.0 on stdin/stdout |
| `tesela-plugins` | — | Lua runtime (working) + WASM stub |

## Build & Test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all
```

CI runs on every push/PR (`.github/workflows/ci.yml`): fmt + clippy + tests on ubuntu + macOS.

## Architecture Notes

- **No business logic in CLI/TUI** — only `tesela-core` traits (`NoteStore`, `SearchIndex`, `LinkGraph`)
- **TUI is Elm-style**: `Event → Action → State → View`. Handler is a pure function; no side effects.
- **External editor**: TUI suspends raw mode, spawns `$EDITOR`, restores raw mode. Handled in `app.rs::spawn_editor`.
- **Plugin hooks**: `on_note_created`, `on_note_updated`, `on_note_deleted`, `on_search`
- **MCP tools**: `search_notes`, `get_note`, `create_note`, `list_notes`, `get_backlinks`, `get_daily_note`

## Note Format

All notes are Markdown with YAML frontmatter and block-based outliner structure:

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

## Key Files

- `crates/tesela-core/src/traits/` — the trait API surface
- `crates/tesela-tui/src/app.rs` — TUI event loop + action processor
- `crates/tesela-tui/src/handler.rs` — pure key → action mapping (all tests here)
- `crates/tesela-tui/src/state/mod.rs` — AppState struct

## What's Done (Phase 8)

- Legacy `src/` monolith removed
- CI workflow added
- TUI: create (`c`), edit (`e`), daily (`d`), delete (`D`), graph toggle (`g`), fuzzy finder (`Ctrl+P`)
- Outliner widget renders `- ` block trees with `├─`/`└─` chars
- Graph widget shows backlinks + forward links

## What's Next

- Phase 9: Slint desktop GUI
