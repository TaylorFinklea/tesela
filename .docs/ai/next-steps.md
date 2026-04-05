# Next Steps

*Last updated: 2026-04-04*

## Phase 1: Complete ✅

All Phase 1 items shipped.

## Phase 2: LogSeq Importer (next)

- [ ] CLI command: `tesela import-logseq --source ~/logseq --target ~/mosaic`
- [ ] Format conversion: journals → daily notes, pages → notes
- [ ] Syntax mapping: DEADLINE, SCHEDULED, [#A] priorities, TODO/DOING/DONE
- [ ] LogSeq-specific cleanup: strip collapsed::, id::, #+BEGIN_QUERY
- [ ] Dry-run mode: preview what would be imported

## Discovery Sessions Needed

- [ ] Phase 3: First-class types (Anytype-style) — design session
- [ ] Phase 5: Power menu grammar — design session

## Backlog (remaining)

See `.docs/ai/roadmap.md` → Backlog section. Remaining items:
- Layout: pixel alignment, tag alignment, spacing, sidebar polish
- Bugs: autocomplete positioning, cursor bugs, WebSocket reliability
- Tests: API integration tests, SwiftUI snapshots
- Docs: (most done — SwiftUI view snapshots still open)

## When picking up work

1. Read `.docs/ai/roadmap.md`, `current-state.md`, and this file
2. Run `cargo test --workspace`
3. Build: `xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build`
4. Pick from Phase 2 items or ask Taylor
