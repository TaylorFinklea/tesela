# Current State

*Last updated: 2026-03-27 (session 2)*

## Active Branch

`main` — all work lands directly on main.

## Recent Progress

- **Tag display rework**: Only type tags (#Task, #Project) become right-aligned pills; casual tags (#meeting, #work) stay inline with secondary label color styling
- **Status icon layout**: Checkbox now sits right next to bullet (was detached 12px to the right)
- **Date picker Enter fix**: Removed conflicting global event monitor; Enter now reliably fires in text input mode
- **Phase H complete**: PropertyPageView, PropertyPicker popover, keyboard-navigable SelectListView
- **Bug fixes**: BlockStyler crash, focusedBlockIndex tracking, TilesView slash/space overlays, popover z-ordering, store.create() frontmatter preservation

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
