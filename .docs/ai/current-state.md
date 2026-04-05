# Current State

*Last updated: 2026-04-04*

## Active Branch

`main`

## Last Session Summary

**Date**: 2026-04-04

**Phase 1 (Polish & Reliability) completed:**
- Server auto-start: app finds and launches tesela-server as child process
- Theme system: dark/light/auto + 11 accent colors, persisted in UserDefaults
- Backup restore: `tesela restore <dir> [--overwrite] [--dry-run]`
- Ghost bullet on hover: faint dot between blocks, click to insert new block
- Bullet threading: lines from parent baseline to child baseline
- Baseline-aligned layout: all inline elements share baselineY = yOffset + 11
- Tags as right-aligned plain text (no pills)

**Backlog items completed (subagents + external handoff agent):**
- VimEngine unit tests: 90 tests
- BlockParser + Block model tests: 60+ tests
- README rewrite
- API endpoint documentation
- MCP tool documentation
- Type system documentation
- Contributing guide
- Inline code comments (OutlinerView, VimKeyHandler)

**Infrastructure:**
- Claude Code automations: /build-and-test skill, auto-format hook, .env blocker hook
- backlog-worker + test-writer subagent definitions
- context7 MCP server installed
- Handoff docs migrated from docs/ai/ to .docs/ai/

## Build Status

- `cargo test --workspace` — all tests pass
- `xcodebuild -scheme Tesela -configuration Debug build` — builds clean

## Blockers

None.

## Next Phase

Phase 2: LogSeq Importer — see `.docs/ai/roadmap.md`
