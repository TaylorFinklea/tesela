# Tesela Roadmap

## What Tesela Is

Keyboard-first, file-based note-taking system (org-mode successor). Rust backend, native macOS SwiftUI app. Taylor's daily-driver tool — reliability matters more than features.

**Core principle:** Files are truth, SQLite is cache.

## Architecture

- **Rust workspace** (`crates/`): tesela-core, tesela-cli, tesela-tui, tesela-mcp, tesela-server, tesela-plugins
- **SwiftUI macOS app** (`app/Tesela/`): connects to tesela-server on localhost:7474 (REST + WebSocket)
- **Property system**: Tags, Properties, and Values are all pages with YAML frontmatter (Logseq DB model)

## Milestones

### Done

- **Phase 11 (MVP)**: Block outliner, sidebar, search, WebSocket sync, tiles timeline, Vim mode, right sidebar, wiki-link pills, graph view, properties
- **Phase 12 (v1 Polish)**: Inline tile editing, UX polish, graph polish
- **Phase 13.1–13.6 (Types & Properties)**: Task MVP, typed properties, page types, Tag/Property pages, property inheritance, shared properties
- **Phases A–H (Property System Migration)**: Auto-create tag pages, property pages as entities, tag_properties frontmatter, extends chain, table views on tag pages, block property indexing, block drill-in UI, property configuration UI, keyboard-navigable select popover

### Current Focus

- Bug fixes and polish on the property/type system
- Keyboard-first UX for all property interactions

### Upcoming

- **Phase 9**: Slint desktop GUI (TUI side — deferred; SwiftUI app is primary focus)
- **13.7 Node references**: Properties linking to other nodes, bidirectional
- **13.8 Queries**: Filter by type + properties, table/list/kanban results
- **Tag display rework**: Only type tags (#Task, #Project) become pills; casual tags (#meeting) stay inline
- **Visual mode, dot-repeat, /search** in Vim engine
- **Whiteboards, sync, App Store** — deferred post-v1

## Constraints

- macOS 26 minimum, Swift 6 strict concurrency
- One Rust server process, one SwiftUI client
- No business logic in CLI/TUI/GUI — only in tesela-core traits
- Files are the canonical source; DB is always rebuildable from files

## Non-Goals (for now)

- iOS/iPadOS app
- Multi-device sync (CRDTs)
- App Store distribution
- Plugin marketplace
- Collaborative editing
