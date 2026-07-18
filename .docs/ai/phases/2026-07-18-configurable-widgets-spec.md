# Configurable in-app widgets — tesela-tko

## Goal

- reconnect Query notes + saved views to live web rail and iOS Dashboard
- add/remove/reorder on both clients; device-local persistence
- preserve current web rail projections; perfect Agenda, Inbox, Sync Health first
- no general widget SDK; no OS/home-screen widgets

## Contract

- stable placement id: `builtin:<id>` / `query:<note-id>` / `view:<view-id>`
- persisted v1 layout: ordered placements with source kind/id + fallback title
- missing/deleted source remains visible as unavailable until removed/replaced
- Query-note dependency revision: canonical note checksum/modified revision
- saved-view dependency revision: deterministic canonical record payload
- result invalidation: web canonical note-list revision + WS query invalidation; iOS `refreshTick` + source revision
- explicit states: loading, refreshing/stale, error, empty, unavailable
- command ids: add, add-by-id, remove, move-up, move-down, refresh/open

## Web

- pure layout/candidate/revision helpers + unit coverage
- `GrRail`: hydrate notes + views, resolve ordered placements, picker, command-event bridge
- built-ins: Quick Capture, Favorites, Pinned, Recent, Agenda, Sync Health
- default also mounts canonical saved-view Inbox; arbitrary Query notes/views use compact query results
- every control remains inside rail keyboard traversal and carries a command id

## iOS

- shared Codable placement/layout helpers persisted in `UserDefaults`
- expose all Query-note descriptors from `MockMosaicService`; reuse `fetchViews()`
- Dashboard loads catalog + ordered layout, renders query/view result cards, Agenda, Sync Health
- add sheet + move/remove controls; result refresh keyed to source revision + `refreshTick`
- wire the shipping Graphite Library Dashboard and keep legacy Workspace Dashboard compatible

## Out of scope

- syncing layout between devices
- user-authored projection/plugin SDK
- WidgetKit/home-screen widgets
- changing Query/View storage or Rust query semantics
- table/kanban layout inside the narrow web rail or iOS Dashboard

## Acceptance

- add any Query note or saved view on web + iOS
- remove and reorder; reload/relaunch preserves order independently per device
- Agenda, Inbox, Sync Health show useful compact content and honest state
- edits/remote changes refresh without retaining stale query results
- adjacent rail keyboard/Escape behavior remains green

## Verify

- `pnpm --dir web test:unit`
- `pnpm --dir web check`
- `pnpm --dir web test:e2e -- rail-keyboard` (or focused runner equivalent)
- `xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 16'`
- installed desktop manual QA + iOS Simulator manual QA checklist
