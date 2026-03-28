# Next Steps

*Last updated: 2026-03-28*

## Done (this sprint)

- [x] Tag display rework, date picker Enter fix, status icon layout
- [x] Inline autocomplete for # and [[ (text-change-driven NSPopover)
- [x] Visual mode (character-level: v + hjklwbe0$, d/c/y on selection)
- [x] Dot-repeat count fix (5. after dw deletes 5 words)
- [x] /search within page (/ opens search bar, n/N navigates matches)
- [x] Tag page filters (filter by property value, sort by column)
- [x] Right sidebar polish (page info, grouped backlinks, context lines)

## Immediate

- [ ] 13.7 Node references — properties that link to other nodes (bidirectional)
- [ ] Kanban view on tag pages (in addition to table view)
- [ ] Search highlighting (yellow background on matches in blocks)

## When picking up work

1. Read `docs/ai/roadmap.md`, `current-state.md`, and this file
2. Run `cargo test --workspace` to confirm clean baseline
3. Build the app: `xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build`
4. Pick the top unchecked item or ask Taylor what to work on
