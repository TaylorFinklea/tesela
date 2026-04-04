# Next Steps

*Last updated: 2026-04-04*

## Phase 1: Polish & Reliability (active)

- [ ] UI overhaul — theme system, consistent spacing, professional visual quality
- [ ] Server lifecycle — embed server in SwiftUI app + polish LaunchAgent
- [ ] Backup restore command + round-trip verification
- [ ] Ghost bullet on hover (Logseq-style) instead of permanent empty block

## Up Next

- [ ] Phase 2: LogSeq importer (CLI command, format conversion, dry-run)
- [ ] Phase 3 discovery: First-class types product design session
- [ ] Phase 5 discovery: Power menu grammar design session

## Backlog (smaller models)

See `.docs/ai/roadmap.md` → Backlog section for full list.
Priority picks for next backlog batch:
- [ ] VimEngine + BlockParser unit test coverage
- [ ] Bullet threading visual quality
- [ ] README.md update with current features

## When picking up work

1. Read `.docs/ai/roadmap.md`, `current-state.md`, and this file
2. Run `cargo test --workspace` to confirm clean baseline
3. Build: `xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build`
4. Pick from Phase 1 items or ask Taylor
