# Next Steps

*Last updated: 2026-03-27*

## Immediate

- [x] Tag display rework — type tags (#Task, #Project) as right-aligned pills, casual tags (#meeting, #work) stay inline in block text
- [x] Date picker Enter key fix — Enter doesn't always fire in text input mode
- [x] Status icon layout — icon should be next to bullet, not replacing it

## Short-term

- [ ] 13.7 Node references — properties that link to other nodes (bidirectional)
- [ ] 13.8 Queries — filter blocks by type + property values, render as table/list/kanban
- [ ] Visual mode in Vim engine (character and line selection)
- [ ] Dot-repeat (`.`) in Vim engine
- [ ] `/search` in Vim Normal mode

## When picking up work

1. Read `docs/ai/roadmap.md`, `current-state.md`, and this file
2. Run `cargo test --workspace` to confirm clean baseline
3. Build the app: `xcodebuild -project app/Tesela/Tesela.xcodeproj -scheme Tesela -configuration Debug build`
4. Pick the top unchecked item or ask Taylor what to work on
