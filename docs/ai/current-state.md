# Current State

*Last updated: 2026-03-27*

## Active Branch

`main` — all work lands directly on main.

## Recent Progress

- **Phase H complete**: PropertyPageView (editable schema for Property pages), PropertyPicker popover on TagPageView, AppState.updatePageContent() for frontmatter mutations
- **Keyboard-navigable select popover**: SelectListView replaces NSButton-based popover — arrow/j/k nav, Enter to confirm, Escape to dismiss
- **Priority & Effort slash commands**: wired up (were TODO stubs)
- **Bug fixes**: BlockStyler crash (text/textStorage length mismatch), focusedBlockIndex not set on becomeFirstResponder (slash commands broken on tiles), slash/space menu overlays missing from TilesView, popover z-ordering (window activation), store.create() double-frontmatter stripping custom fields

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
