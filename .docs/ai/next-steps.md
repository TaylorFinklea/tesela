# Next Steps

*Last updated: 2026-04-09*

## Web Client — M1 Next

Plan: `/Users/tfinklea/.claude/plans/async-giggling-moth.md`

### M0 — Scaffold & Connect ✓

All M0 items done. See `current-state.md` for the full summary.

### M1 — Read-only outliner (immediate)

Open questions to resolve before starting:
- **BlockParser strategy.** Port `tesela-core/src/block.rs` to TypeScript (zero round-trip, duplicated logic) or expose it via a new `GET /notes/:id/blocks` endpoint (single source of truth, one extra fetch per note open). Recommended: port to TS — block parsing needs to be synchronous inside the editor.
- **Route shape.** `/p/[id]` for a single note, or stick with `/notes/[id]`? Pick one and stay consistent.

Implementation:
- Add `api.getNote(id)` to `web/src/lib/api-client.ts`
- Create `web/src/lib/block-parser.ts` (if porting) or the new API endpoint (if not)
- Create `web/src/app/p/[id]/page.tsx` — fetches the note, parses blocks, renders them in an indented layout
- Create `web/src/components/BlockEditor.tsx` — one CM6 instance per block in read-only mode, with decorations for `[[wiki-links]]`, `#tags`, and `key:: value` property lines
- Arrow-key navigation between blocks (leaves editor focus on blur, restores on focus)
- Click a wiki-link → route to that page

### Post-M1

- **M2** — CM6 editable + 500ms debounced `PUT /notes/{id}` + Enter/Tab/Shift-Tab block ops + WS reconcile without clobbering in-flight edits
- **M3** — Vim engine (new TS implementation, Vitest coverage, cross-block motions, command palette)
- **M4** — Sidebar & tag pages
- **M5** — Tiles & drill-in
- **M6** — Graph & search UI
- **M7** — Theme/polish to Linear/Logseq/Zed bar
- **M8** — (Optional) Tauri wrap

### Rust-side backlog (still active)

See `.docs/ai/roadmap.md` Backlog section — Haiku and Sonnet items are fair game.

## When picking up work

1. Read `.docs/ai/roadmap.md`, `current-state.md`, and this file
2. Read the plan file at `/Users/tfinklea/.claude/plans/async-giggling-moth.md`
3. Start `tesela-server` for testing: `cargo run -p tesela-server`
4. Start the web dev server: `pnpm --dir web dev`
5. Pick from the current milestone's checklist
