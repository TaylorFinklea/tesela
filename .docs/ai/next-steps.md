# Next Steps

*Last updated: 2026-04-14*

## What's Done

The web client is feature-complete through Phase 2 (Navigation & Discovery). All core outliner, Vim, slash commands, leader menu, sidebar, command palette, graph, timeline, tag tables, settings, themes, favorites, search highlighting, tag table filtering, right sidebar properties, and graph filters are implemented and working.

## Next Phase Candidates

Pick from these based on what feels most needed for daily-driver use:

### Phase 3A: Type System Depth (Anytype vision)
- [ ] Kanban view on tag pages (group blocks by a select property like Status)
- [ ] Queries / Sets — saved filters by type + property values, displayed as table/list/kanban
- [ ] Collections — manual groupings of pages
- [ ] Node references — property value links to another page (bidirectional)
- [ ] Tag inheritance — `extends` chain (Task → Root Tag), child inherits parent properties
- [ ] Global property registry — search existing property pages when adding to a tag

### Phase 3B: Editor Power Features
- [ ] Visual mode in Vim (character + line selection)
- [ ] Block merge on Backspace at start of non-empty block
- [ ] Multi-block selection and operations
- [ ] `/template` slash command — insert from template pages
- [ ] `/date` slash command — date picker UI
- [ ] Block drill-in — focus on a single block and its children

### Phase 3C: Polish & Edge Cases
- [ ] Empty/loading/error states for every view (audit)
- [ ] Keyboard shortcuts for favorites (e.g., `f` to toggle)
- [ ] Graph: click node → navigate, drag to reposition
- [ ] Right sidebar: inline property editing (not just display)
- [ ] Breadcrumb improvements — clickable path segments
- [ ] Mobile/responsive layout considerations

### Rust Backlog (parallel)
See `roadmap.md` Backlog section — Haiku and Sonnet tier items are safe for parallel work.

## When Picking Up Work

1. Read `.docs/ai/current-state.md` and this file
2. Check `git log --oneline -10` to see recent changes
3. Start `tesela-server`: `cargo run -p tesela-server`
4. Start web dev server: `pnpm --dir web dev`
5. Pick a phase or ask Taylor what to prioritize
