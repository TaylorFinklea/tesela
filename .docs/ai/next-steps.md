# Next Steps

*Last updated: 2026-04-09*

## Web Frontend Pivot — M0 Complete, M1 Next

Plan: `/Users/tfinklea/.claude/plans/async-giggling-moth.md`

### M0 — Scaffold & Connect ✓

All M0 items done. See `current-state.md` for the full summary. Committed as `465c6a8` (ts-rs wiring) plus the pending commit for the `web/` scaffold and handoff doc updates.

**Outstanding after M0:**
- Verify full happy path by starting `tesela-server` and confirming notes list renders + WS status flips to "live"
- Decide whether to add a `.docs/ai/` mention of how to regenerate TS types (`cargo test -p tesela-core --lib export_bindings`)

### Next — M1 Read-only Outliner

- Port `BlockParser.swift` logic to `web/src/lib/block-parser.ts` (already have a Rust version in `tesela-core/src/block.rs` — could call that via an API endpoint instead of re-implementing in TS; decide)
- One CM6 instance per block, read-only, with decorations for wiki-links (`[[target]]`), tags (`#tag`), and property lines (`key:: value`)
- `/p/[id]` route that renders a note's blocks in an indented outliner layout
- Use existing `api.getNote(id)` (add this endpoint to `api-client.ts`)

### Next up

- **M1** — Read-only outliner (BlockParser port, one CM6 per block, wiki-link/tag decorations)
- **M2** — Editing + save-back
- **M3** — Vim engine port
- **M4** — Sidebar & tag pages
- **M5** — Tiles & drill-in
- **M6** — Graph & search UI
- **M7** — Theme, settings, polish to Linear/Logseq/Zed bar
- **M8** — (Optional) Tauri wrap

### SwiftUI-side work

**All frozen.** The SwiftUI app stays in the repo but no new feature work. The broken OutlinerView split in the working tree is left alone per Taylor's call.

### Rust-side backlog (still active — benefits both clients)

- See `.docs/ai/roadmap.md` Backlog section — Rust Haiku/Sonnet items are still fair game. Swift items are frozen.

## When picking up work

1. Read `.docs/ai/roadmap.md`, `current-state.md`, and this file
2. Read the plan file at `/Users/tfinklea/.claude/plans/async-giggling-moth.md`
3. Start `tesela-server` for testing: `cargo run -p tesela-server`
4. Start the web dev server: `pnpm -C web dev`
5. Pick from the current milestone's checklist
