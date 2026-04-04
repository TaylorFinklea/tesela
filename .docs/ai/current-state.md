# Current State

*Last updated: 2026-04-04*

## Active Branch

`main` — all work lands directly on main.

## Last Session Summary

**Date**: 2026-04-03/04

- Rebuilt roadmap with phased plan + backlog for smaller AI models
- Migrated handoff docs from `docs/ai/` to `.docs/ai/` (global convention)
- Fixed tag system: tags managed by array (block.tags), never inline in editor text
- Baseline-aligned layout for all inline elements (bullet, status, text, tags)
- Tags rendered as right-aligned plain text (Logseq style, no pills)
- Deferred autocomplete check to fix cursor position lag (off-by-one in query)
- Backup system: `tesela backup` CLI + auto-daily on server startup
- Empty block at bottom of tiles/pages for quick capture
- "New tag: name" option in autocomplete for non-existent tags

## Build Status

- `cargo test --workspace` — all tests pass
- `xcodebuild -scheme Tesela -configuration Debug build` — builds clean

## Blockers

None.

## Known Issues

- Bullet threading visual quality still needs work (position close but not Logseq-quality)
- Tag alignment is plain text now but may need per-tag click handling for multi-tag blocks
- Server must be started manually (LaunchAgent exists but untested, embed-in-app not done)
- No LogSeq importer yet — can't migrate existing data
