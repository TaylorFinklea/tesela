# Configurable in-app widgets — tesela-tko report

## Shipped

- web/desktop Graphite rail now uses a versioned device-local ordered layout
- stable ids: `builtin:*`, `query:<note-id>`, `view:<view-id>`
- add/remove/move/collapse controls + picker + shared command ids; manifest regenerated for web + iOS
- live arbitrary Query-note and saved-view projections; canonical definition revisions + note/view/WS invalidation
- explicit loading/refreshing/error/empty/unavailable states; compact Agenda, Inbox, Sync Health
- iOS Graphite + legacy Dashboard share one configurable collection; default Agenda/Inbox/Sync Health
- iOS local Query execution now supports page results as well as blocks
- missing sources remain removable unavailable placements instead of silently disappearing

## Tests

- web layout/revision/group flattening: 5 focused tests
- iOS layout/stable-id/revision behavior: 5 focused tests
- browser rail: 4/4 — keyboard/Escape + add/remove/reorder/collapse persistence across reload
- full web unit: 1,012/1,012
- `pnpm --dir web check`: 0 errors, 48 pre-existing warnings
- command manifest freshness + web production build: pass
- iOS app target build + focused `DashboardWidgetsTests`: pass on iPhone 17 simulator
- direct simulator install/launch: pass; live Graphite shell rendered
- iOS Dashboard click-through on iPhone 17 / iOS 26.5: add/remove/reorder/collapse, picker cancel, all-added picker state, and relaunch persistence pass
- unavailable-source fixture: explicit recovery state rendered and remained removable; native accessibility removal pass
- Sync Health refresh: no crash; simulator had no live relay tick, so live-tick content was not claimed

## Verification boundary

- repo-wide `cargo fmt --all -- --check` fails on existing untouched Rust drift; tracked by `tesela-bz5`
- no Rust changed; `cargo build -p tesela-server` passed for the browser harness
- build skill stopped the remaining repo-wide Rust matrix at the formatter failure
- full unit initially exposed release tests pinned to desktop 0.1.2 while catalog/config were already 0.1.3; expectations aligned to shipped source

## Product QA exercised

- web/desktop: add Query-note + saved-view widgets; reorder, collapse, reload, and remove
- iOS: Library → Dashboard; add, remove, reorder, collapse, relaunch, picker cancel/all-added, and Sync Health refresh
- iOS unavailable source: render fallback title/recovery guidance, then remove without crashing
