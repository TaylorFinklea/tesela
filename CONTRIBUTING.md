# Contributing to Tesela

Tesela is a keyboard-first note-taking system with a Rust backend and a native macOS SwiftUI frontend. Reliability matters more than feature count, so contributions should prefer small, verifiable changes over broad refactors.

## Repository layout

| Path | Purpose |
| --- | --- |
| `crates/tesela-core/` | Core types, traits, storage, indexing, and business logic |
| `crates/tesela-cli/` | CLI entrypoints and macOS LaunchAgent helpers |
| `crates/tesela-tui/` | Ratatui-based terminal UI |
| `crates/tesela-server/` | REST API and WebSocket server on `localhost:7474` |
| `crates/tesela-mcp/` | MCP server for AI tools |
| `crates/tesela-plugins/` | Plugin runtime |
| `app/Tesela/` | Native macOS SwiftUI app and tests |
| `.docs/ai/` | Current roadmap, state, and handoff notes |

## Setup

Prerequisites:

- Rust stable
- Xcode
- `xcodebuild` available on your `PATH`

Optional:

- `xcodegen` if you need to regenerate the Xcode project from `app/Tesela/project.yml`

## Build and verify

Run these before you open a PR or hand off work:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all
xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build
```

CI currently enforces:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

## Development guidelines

- Keep business logic in `tesela-core`, not in CLI, TUI, server glue, or SwiftUI views.
- New trait definitions belong in `crates/tesela-core/src/traits/`.
- TUI state changes should flow through the action system.
- New TUI modes must update `mode.rs`, `handler.rs`, `app.rs`, and `status_bar.rs`.
- Prefer additive, local changes over architectural rewrites.
- If you touch note or type behavior, verify both the Rust workspace and the macOS app build.

## Swift app notes

- The app connects to `tesela-server` on `localhost:7474`.
- If you add or remove Swift files and the Xcode project needs regeneration, run:

```bash
xcodegen generate --project app/Tesela/project.yml
```

- App tests live in `app/Tesela/TeselaTests/`.

## Tests

When adding behavior, add or extend the closest existing tests:

- Rust core behavior: `crates/*/tests` or crate-local unit tests
- Swift parsing and model logic: `app/Tesela/TeselaTests/`
- Server route behavior: integration tests around `tesela-server`

Prefer targeted unit or integration coverage over broad snapshot-style assertions unless the feature is primarily visual.

## Roadmap and handoff

Before starting new work, read:

- `.docs/ai/roadmap.md`
- `.docs/ai/current-state.md`
- `.docs/ai/next-steps.md`

If you finish a meaningful chunk, update the handoff docs when needed so the next contributor can pick up quickly.

## Commit hygiene

- Make focused commits with descriptive messages.
- Keep one logical change per commit when practical.
- Do not mix unrelated cleanup into feature or bug-fix commits.

## Pull requests

A good PR description should include:

- What changed
- Why it changed
- How it was verified
- Any follow-up work or known limitations
