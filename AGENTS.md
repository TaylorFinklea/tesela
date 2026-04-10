# Tesela — Agent Instructions

## AI Handoff

Handoff state lives in `.docs/ai/` (follows global workflow from `~/CLAUDE.md`).

## Progress Tracking

**Always use todo lists.** When working on any task, create and maintain a todo list using TaskCreate/TaskUpdate so the user can see progress and what you're working on. Mark tasks `in_progress` when starting and `completed` when done. Break work into discrete steps.

## After Any Work Session

**Always commit before stopping.** After completing a working chunk of changes:
1. Stage relevant files with `git add`
2. Create a commit with a descriptive message
3. Do **not** push unless explicitly asked by the user

**Always produce a QA checklist.** After implementing any user-facing feature (web or TUI), output a step-by-step manual test plan covering:
- Exact key sequences or click paths to trigger each new feature
- Observable expected outcomes (what the user should see)
- Cancel/Esc paths and edge cases
- A regression section for anything adjacent that could have broken

## Bash Command Rules

**One command per Bash call.** Do not chain with `&&` unless the second command needs the first's stdout via a pipe.

- Wrong: `cd crates/tesela-core && cargo test`
- Right: two separate calls, or `cargo test -p tesela-core`, or `git -C /path/to/repo commit ...` to avoid a `cd`
- Piping is fine: `cargo test 2>&1 | grep "error\["`

## Project

Tesela is a keyboard-first note-taking system with a Rust backend and a Next.js web frontend.
6-crate Cargo workspace plus `web/`. See `CLAUDE.md` for full details.

## Build & Verify

Always run before committing:

```bash
cargo build --workspace          # must succeed
cargo test --workspace           # all tests must pass
cargo clippy --workspace -- -D warnings  # zero warnings
cargo fmt --all                  # auto-format
```

## Conventions

- All trait definitions live in `crates/tesela-core/src/traits/`
- TUI state changes go through the action system (`action.rs` → `app.rs::process_action`)
- Do not add business logic to `tesela-cli` or `tesela-tui` — keep it in `tesela-core`
- New TUI modes need updates in: `mode.rs`, `handler.rs`, `app.rs` (draw + process_action), `status_bar.rs`
