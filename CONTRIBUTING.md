# Contributing to Tesela

Tesela is a keyboard-first note-taking system with a Rust backend and a Next.js + React web frontend. Reliability matters more than feature count, so contributions should prefer small, verifiable changes over broad refactors.

## Repository layout

| Path | Purpose |
| --- | --- |
| `crates/tesela-core/` | Core types, traits, storage, indexing, and business logic |
| `crates/tesela-cli/` | CLI entrypoints and LaunchAgent helpers |
| `crates/tesela-tui/` | Ratatui-based terminal UI |
| `crates/tesela-server/` | REST API and WebSocket server on `localhost:7474` |
| `crates/tesela-mcp/` | MCP server for AI tools |
| `crates/tesela-plugins/` | Plugin runtime |
| `web/` | Next.js 16 App Router web client (CodeMirror 6 + shadcn/ui) |
| `.docs/ai/` | Current roadmap, state, and handoff notes |

## Setup

Prerequisites:

- Rust stable
- Node 20+ and pnpm

## Build and verify

Run these before you open a PR or hand off work:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all

pnpm --dir web install
pnpm --dir web tsc --noEmit
pnpm --dir web lint
```

CI currently enforces:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace -- -D warnings`
- `cargo test --workspace`

## Development guidelines

- Keep business logic in `tesela-core`, not in CLI, TUI, server glue, or web client UI.
- New trait definitions belong in `crates/tesela-core/src/traits/`.
- TUI state changes should flow through the action system.
- New TUI modes must update `mode.rs`, `handler.rs`, `app.rs`, and `status_bar.rs`.
- Prefer additive, local changes over architectural rewrites.
- If you touch note or type behavior, verify the Rust workspace tests AND that `pnpm --dir web tsc --noEmit` still passes.

## Web client notes

- The web client talks to `tesela-server` on `localhost:7474`.
- TypeScript types in `web/src/lib/types/` are generated from Rust via `ts-rs`. Regenerate with:

```bash
cargo test -p tesela-core --lib export_bindings
```

- Commit regenerated type files alongside the Rust changes that triggered them.

## Tests

When adding behavior, add or extend the closest existing tests:

- Rust core behavior: `crates/*/tests` or crate-local unit tests
- Server route behavior: integration tests around `tesela-server`
- Web client behavior: component tests (Vitest/Testing Library) in `web/src/**/*.test.ts{,x}`

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
