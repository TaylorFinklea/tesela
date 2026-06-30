# Tesela — Agent Instructions

Project-specific guidance for any AI coding agent (Claude Code, Codex, Copilot, etc.). Shared agent behavior (TaskCreate/TaskUpdate, shell discipline, commit/push defaults) lives in `~/AGENTS.md`.

## AI Handoff

Handoff state lives in `.docs/ai/`. Read `.docs/ai/roadmap.md` (Now/Next/Later) and `.docs/ai/current-state.md` at session start; update them at session end.

## Task tracking — beads pilot (2026-06-30)

**The forward backlog / "what to work on next" for this repo is piloted in [beads](https://github.com/steveyegge/beads) (`bd`), not the roadmap's Now/Next list.** Local-only stealth install — `.beads/` is git-excluded via `.git/info/exclude`; nothing is committed, so the pilot leaves no trace if dropped.

Agent loop (harness-agnostic — `bd` is just a CLI):
- `bd ready` — priority-sorted, dependency-aware queue of unblocked work (`--json` for scripting; `bd ready --claim --json` atomically claims the top item).
- `bd show <id>` — full detail before starting.
- `bd update <id> --claim` — set in_progress + assignee atomically (replaces any `[~]`/claim ceremony).
- Run the repo's Verify (build/test) first, then `bd close <id> --reason "…"`.
- `bd create "Title" -t task -p 2 -d "…"` — file work discovered mid-task; `bd dep add <new> <parent> -t discovered-from` records provenance.
- `bd dep add <issue> <blocker>` — `<issue>` is blocked-by `<blocker>` (hidden from `bd ready` until the blocker closes).

Layer split — beads owns ONLY the backlog/ready-queue. Do NOT migrate these into beads:
- **Rationale / ADRs → `.docs/ai/decisions.md`** (prose, unchanged).
- **Multi-session design → `.docs/ai/phases/*`** (prose, unchanged).
- **Loop state → `.docs/ai/current-state.md`** (unchanged).
- `roadmap.md` keeps the durable product-arc narrative; new *actionable* work goes into `bd`.

`user-verify`-labeled issues = human device-test/ops gates, not agent dev work. This is a time-boxed evaluation (does `bd ready` beat scanning roadmap Now?) — see chezmoi-config `.docs/ai/phases/beads-pilot-spec.md`.

## After Any Work Session

**Always produce a QA checklist.** After implementing any user-facing feature (web or TUI), output a step-by-step manual test plan covering:

- Exact key sequences or click paths to trigger each new feature
- Observable expected outcomes (what the user should see)
- Cancel/Esc paths and edge cases
- A regression section for anything adjacent that could have broken

## Project Overview

Tesela is a keyboard-first, local-first note-taking system (org-mode / Logseq-DB successor) with a Rust core and three clients: a SvelteKit web app, a SwiftUI iOS app, and a Tauri desktop shell that wraps the web UI. Taylor's daily-driver tool — reliability matters more than features.

**Core principle:** Loro CRDT is the sync source of truth; Markdown files are a materialized export and the SQLite/FTS5 index is a rebuildable cache. (Pre-2026-05 this was "database-first, files are export"; the Loro cutover made the CRDT authoritative — see `decisions.md` 2026-05-29 / 2026-06-10.)

## Workspace Structure

12 crates in `crates/` plus the Tauri desktop shell in `src-tauri/`:

| Crate | Binary | Purpose |
|-------|--------|---------|
| `tesela-core` | — | Foundation: types, traits, storage, SQLite/FTS5, indexer, query engine (JQL) |
| `tesela-cli` | `tesela` | Thin dispatcher; all subcommands via `clap` |
| `tesela-tui` | `tesela-tui` | Elm-style TUI (ratatui/crossterm) |
| `tesela-mcp` | `tesela-mcp` | MCP server over JSON-RPC 2.0 on stdin/stdout |
| `tesela-server` | `tesela-server` | REST + WebSocket API on localhost:7474 (the web/desktop backend) |
| `tesela-plugins` | — | Lua runtime (working) + WASM stub |
| `tesela-sync` | — | Loro CRDT sync engine — the sole engine post-cutover |
| `tesela-sync-ffi` | — | UniFFI bindings exposing the sync engine to the iOS app |
| `tesela-relay` | `tesela-relay` | Zero-knowledge encrypted sync relay (mailbox; the CF Worker is the prod twin) |
| `tesela-backup` | — | age-encrypted backup / restore + GFS retention |
| `tesela-fixtures` | — | Shared test-fixture mosaics |
| `tesela-fixtures-cli` | `tesela-fixtures-cli` | CLI to generate fixture mosaics |
| `src-tauri` | `tesela-desktop` | Tauri desktop shell — runs `serve()` in-process and serves the static web UI same-origin |

Plus two non-crate clients:

- **Web client** in `web/`: SvelteKit + Svelte 5 (runes) + TypeScript + CodeMirror 6 + TanStack Query + bits-ui + Tailwind v4. The live shell is the Graphite UI at route `/g`. TypeScript types are generated from Rust via `ts-rs` — run `cargo test -p tesela-core --lib export_bindings` to regenerate `web/src/lib/types/`.
- **iOS app** in `app/Tesela-iOS/`: SwiftUI; runs the Loro engine locally via `tesela-sync-ffi` and syncs through the relay.

## Build & Verify

Always run before committing:

```bash
# Rust
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all

# Web
pnpm --dir web check        # svelte-check (types) — blocking in CI
pnpm --dir web test:unit     # node:test unit suites
```

CI runs on every push/PR (`.github/workflows/ci.yml`), four blocking jobs: Rust (fmt + `clippy -D warnings` + `cargo test --workspace`), web (svelte-check + node:test unit suites), security audit (`cargo audit` against RUSTSEC), and relay conformance against the Cloudflare Worker.

## Architecture Notes

- **No business logic in CLI/TUI** — only `tesela-core` traits (`NoteStore`, `SearchIndex`, `LinkGraph`).
- **TUI is Elm-style**: `Event → Action → State → View`. Handler is a pure function; no side effects.
- **External editor**: TUI suspends raw mode, spawns `$EDITOR`, restores raw mode. Handled in `app.rs::spawn_editor`.
- **Plugin hooks**: `on_note_created`, `on_note_updated`, `on_note_deleted`, `on_search`.
- **MCP tools**: `search_notes`, `get_note`, `create_note`, `list_notes`, `get_backlinks`, `get_daily_note`.

## Conventions

- All trait definitions live in `crates/tesela-core/src/traits/`.
- TUI state changes go through the action system (`action.rs` → `app.rs::process_action`).
- Do not add business logic to `tesela-cli` or `tesela-tui` — keep it in `tesela-core`.
- New TUI modes need updates in: `mode.rs`, `handler.rs`, `app.rs` (draw + process_action), `status_bar.rs`.

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

## Releases

Versioning is date-based: `v0.YYYYMMDD.N` (auto-generated by CI).

- **Auto-release**: pushing to `main` triggers `.github/workflows/release.yml` (builds Linux/macOS/Windows, creates GitHub Release).
- **Manual release**: `scripts/release.sh --manual` triggers the manual workflow via `gh`.
- **Dry run**: `scripts/release.sh --dry-run` verifies the build without releasing.
- **AUTO_RELEASE hook**: when a commit message matches `feat(mN):` (milestone feature), the hook emits a signal. Dispatch the `release` agent in background to push and trigger CI.
- **Release agent**: `.claude/agents/release.md` (Haiku model) — runs the script, reports version or error.
