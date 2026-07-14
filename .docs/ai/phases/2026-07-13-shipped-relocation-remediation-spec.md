# Shipped relocation remediation spec

**Bead:** `tesela-8ig` · **Date:** 2026-07-13 · **Owner tier:** Senior

## Goal

Repair the two device-test failures in the shipped block-relocation slice:
Tauri desktop must deliver HTML drag events to Graphite Dailies, and native iOS
`Move to…` must invoke the existing durable Loro subtree-relocation engine.

## Confirmed failures

- Tauri's default native file-drop handler consumes WebKit drag events before
  the Svelte drag handlers receive them. The same frontend gesture reaches the
  relocation route in Chromium.
- iOS renders `Move to…`, but every Graphite and legacy context-menu handler
  explicitly discards `.moveTo`; the UniFFI surface has no relocation method.

## Product contract

- Desktop: dragging the visible handle continues to use the existing web
  request and complete-subtree semantics inside the installed Tauri shell.
- iOS: `Move to…` presents a searchable destination picker, supports daily and
  page destinations, appends the complete subtree at destination root depth,
  then refreshes affected note projections and wakes relay sync.
- Cancel is a no-op. The source destination is unavailable. Engine validation
  remains authoritative for missing roots, duplicate ownership, and unsafe
  requests; the UI never composes a copy/delete.
- Wire every exposed iOS `Move to…` callsite or remove the action; no visible
  menu item may remain a no-op.

## Implementation boundaries

- Disable only Tauri's native drag/drop handler; no native file-drop behavior
  exists to preserve.
- Add a typed UniFFI request/result surface that maps into
  `SyncEngine::relocate_subtree`; keep relocation logic in `tesela-sync`.
- Reuse the iOS mosaic service and existing sheet/search patterns. Do not add a
  second relocation algorithm in Swift.
- Regenerate checked-in Swift/C headers with the repository binding script.

## Verification

- Regression tests fail on both shipped no-op boundaries before production
  changes and pass afterward.
- `cargo test -p tesela-desktop`
- `cargo test -p tesela-sync-ffi`
- focused iOS tests, then the full Tesela simulator suite
- workspace Rust and web quality gates, run serially where required
- install the corrected desktop app and perform a real shell drag gesture
- upload the next TestFlight build and verify App Store Connect processing
