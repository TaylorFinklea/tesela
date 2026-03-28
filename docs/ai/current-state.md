# Current State

*Last updated: 2026-03-28*

## Active Branch

`main` — all work lands directly on main.

## Recent Progress

- **Visual mode**: Character-level v selection with h/l/w/b/e/0/$ extend, d/c/y operate on selection
- **/search**: / opens search bar, pattern matches across all blocks, n/N navigates matches
- **Dot-repeat**: Count override works (5. after dw), tracks more edit commands
- **Tag page filters**: Server-side filter_property/filter_value params, FilterChip UI, sortable columns
- **Right sidebar polish**: Page Info section (type, tags, dates), backlinks grouped by source with context
- **Inline autocomplete**: # and [[ show filtered page list as you type, arrow/Enter/Escape nav
- **Tag display rework**: Type tags as pills, casual tags inline with styling
- **Bug fixes**: BlockStyler crash, focusedBlockIndex, cursorRect bounds, store.create() frontmatter

## Changed Files (recent)

- `app/Tesela/Tesela/Editor/SelectListView.swift` — new keyboard-navigable list
- `app/Tesela/Tesela/Editor/OutlinerView.swift` — priority/effort commands, focus tracking, popover activation
- `app/Tesela/Tesela/Editor/BlockView.swift` — onFocused callback
- `app/Tesela/Tesela/Editor/BlockStyler.swift` — crash fix
- `app/Tesela/Tesela/Views/PropertyPageView.swift` — new Property page editor
- `app/Tesela/Tesela/Views/TagPageView.swift` — add/remove properties, clickable names, inherited badge
- `app/Tesela/Tesela/Views/TilesView.swift` — slash/space menu overlays
- `app/Tesela/Tesela/Views/ContentArea.swift` — Property page routing
- `app/Tesela/Tesela/App/AppState.swift` — updatePageContent()
- `crates/tesela-core/src/storage/filesystem.rs` — preserve caller frontmatter in create()

## Blockers

None currently.

## Open Questions

- Tag display rework: should casual #tags stay inline while type tags become pills? (Noted in vision doc, not started)
- Property page creation from TagPageView: works but newly created property needs manual type configuration

## Validation Status

- `cargo test --workspace` — all 134 tests pass
- `xcodebuild -scheme Tesela -configuration Debug build` — builds clean
- Manual QA: slash commands work on tiles and pages, priority popover keyboard-navigable, property pages editable
