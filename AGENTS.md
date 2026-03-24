# Tesela — Agent Instructions

## Progress Tracking

**Always use todo lists.** When working on any task, create and maintain a todo list using TaskCreate/TaskUpdate so the user can see progress and what you're working on. Mark tasks `in_progress` when starting and `completed` when done. Break work into discrete steps.

## After Any Work Session

**Always commit before stopping.** After completing a working chunk of changes:
1. Stage relevant files with `git add`
2. Create a commit with a descriptive message
3. Do **not** push unless explicitly asked by the user

**Always produce a QA checklist.** After implementing any user-facing TUI feature, output a step-by-step manual test plan covering:
- Exact key sequences to trigger each new feature
- Observable expected outcomes (what the user should see)
- Cancel/Esc paths and edge cases
- A regression section for anything adjacent that could have broken

## Bash Command Rules

**One command per Bash call.** Do not chain with `&&` unless the second command needs the first's stdout via a pipe.

- Wrong: `cd app/Tesela && xcodegen generate`
- Right: two separate calls, or `git -C /path/to/repo commit ...` to avoid a `cd`
- Piping is fine: `xcodebuild ... | grep "error:"`

## Project

Tesela is a keyboard-first, file-based note-taking system in Rust.
5-crate Cargo workspace. See `CLAUDE.md` for full details.

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
