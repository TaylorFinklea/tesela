# iOS home-screen widgets — tesela-a4m.1

## Goal

- ship distinct Agenda and Inbox WidgetKit widgets
- glanceable, read-only, small + medium families
- tap through to the matching Graphite tab

## Data contract

- main app owns all Loro/query/relay work
- versioned compact snapshot in App Group `group.app.tesela.shared`
- Agenda rows: stable id, text, date/time, kind, overdue
- Inbox rows: stable id, text, source title
- extension only decodes the latest snapshot; no network, Keychain, FFI, or mosaic access
- missing snapshot: explicit “Open Tesela to load” state
- app publishes after initial backend activation and query-revision changes, then asks WidgetKit to reload

## Navigation contract

- `tesela://agenda` → Agenda tab
- `tesela://views` → Views tab
- same route parser in Graphite + legacy shells
- cold-launch URL may set the tab before onboarding/shell presentation

## Widget contract

- separate gallery entries: Tesela Agenda, Tesela Inbox
- `StaticConfiguration`; no configuration intent in this first slice
- whole-widget `widgetURL`; no inline mutations
- `systemSmall` and `systemMedium`
- privacy-sensitive row text; honest empty and stale timestamp treatment

## Out of scope

- task completion, triage, or capture from the widget
- user-selected saved views
- Lock Screen/accessory families, Live Activities, controls
- extension-side sync/query execution
- cross-device widget layout sync

## Verify

- focused XCTest: snapshot codec/store, projection, route parsing
- regenerated xcodegen project lists app, tests, widget extension
- simulator build + focused tests
- iPhone Simulator: gallery add/render for both widgets; warm/cold deep-link routing
- device-signing capability check where local credentials permit
