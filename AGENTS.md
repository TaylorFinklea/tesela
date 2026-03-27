# Tesela — Agent Instructions

## Session Workflow (AI Handoff)

**Start of every session — read these files first:**
1. `docs/ai/roadmap.md` — durable goals, milestones, constraints
2. `docs/ai/current-state.md` — branch, recent progress, blockers, test status
3. `docs/ai/next-steps.md` — exact next actions checklist

**End of every work session — update shared state:**
1. Update `docs/ai/current-state.md` — branch, what changed, blockers, validation status
2. Update `docs/ai/next-steps.md` — check off completed items, add new ones
3. Update `docs/ai/decisions.md` — if any non-obvious architectural decisions were made
4. Commit the docs/ai/ updates along with your code changes

These docs are the source of truth for cross-session continuity. See `docs/ai/handoff-template.md` for the session-end format.

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
