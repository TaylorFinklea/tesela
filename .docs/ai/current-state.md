# Current State

*Last updated: 2026-05-20*

## Active Branch

`main`

## Architecture at a Glance

- **Rust workspace** (`crates/`): `tesela-core`, `tesela-cli`, `tesela-tui`, `tesela-mcp`, `tesela-server`, `tesela-plugins`. Stable, well-tested.
- **Web client** (`web/`): **SvelteKit 2 + Svelte 5** (runes) + CodeMirror 6 + `@replit/codemirror-vim` + Tailwind v4 + TanStack Query (@tanstack/svelte-query v6) + Tabler Icons. SSR disabled (`export const ssr = false` in `+layout.ts`).
- TypeScript types generated from Rust via `ts-rs` — run `cargo test -p tesela-core --lib export_bindings`.
- WebSocket client with exponential backoff reconnect, wired to TanStack Query cache invalidation.

**Design quality bar:** Linear × Logseq × Zed — craft, restraint, keyboard-first, dark-mode-first.

**Theme system:** "Warm Study" — Newsreader serif display + Source Sans 3 body. Day (warm cream) and Evening (warm charcoal) themes, plus 4 alternate themes (Woven, Tile Grid, Depth Layers, Neon Glow). CSS custom properties applied via inline styles on `<html>`.

## Web Client Feature State

### Core (all working)
- Block outliner with always-editable CM6 instances
- Vim mode via `@replit/codemirror-vim` with custom block operators (dd, yy, p, o, O, >>, <<)
- Cross-block j/k navigation
- Slash commands (/task, /todo, /doing, /done, /heading, /property, /link, /date)
- Space leader menu (hierarchical, Neovim which-key style)
- Inline autocomplete for #tags and [[wiki-links]]
- Debounced auto-save (500ms PUT)

### Navigation & Discovery
- **Sidebar**: Today, Timeline, Graph, Pages nav links + Favorites section + Recents section + Settings footer
- **Command palette** (⌘K): Raycast-style with sections (Recent, Actions, Create, Notes, Search), context-aware commands on note pages, keyboard shortcuts as kbd badges, Ctrl+j/k navigation, search highlighting with bold matches
- **Favorites**: localStorage-persisted, star toggle on note pages, sidebar section, command palette "Toggle Favorite" action
- **Right sidebar**: Properties panel (tags, type, custom properties) + Backlinks + Forward links

### Views
- **Note page** (Focus Mode): Large Newsreader title, tag pills, breadcrumb nav, flat block styling (no cards), star/delete buttons
- **All Notes** (/): Paginated list with timestamps and tag badges
- **Timeline** (/timeline): Logseq-style journal with inline editable BlockOutliner per daily note
- **Graph** (/graph): Canvas-based force-directed graph with tag filter dropdown, depth slider, theme-aware colors
- **Tag pages**: Table view with sortable columns, per-column text filters (AND logic), inline property editing
- **Settings**: Theme picker, font size, Vim toggle, server URL, keyboard shortcuts reference

### Layout
- h-screen viewport-pinned layout — sidebar + main content + status bar all fixed, content scrolls internally
- Status bar showing Vim mode, current note, connection status

## Build Status

- Rust: `cargo test --workspace` green on 2026-05-20 (one flaky test — `tesela-server::sigterm_triggers_validated_backup` — passes on retry; not a real failure).
- Web: `pnpm --dir web exec tsc --noEmit` green on 2026-05-20.
- iOS: `xcodebuild -scheme Tesela` green for both `Tesela-Test` simulator and `Roshar` device (iPhone 15 Pro, id `A885F93A-60DD-59DA-9049-289C35EACE23`). Deploy: `xcodebuild ... -destination 'platform=iOS,name=Roshar' -allowProvisioningUpdates build` then `xcrun devicectl device install app --device <id> <Tesela.app>`. Bundle id `app.tesela.ios`.
- Dev server: `pnpm --dir web dev` (Vite, port 5173)

## Running Services (this session — may be stale next session)

- `tesela-server` on `:7474` pointed at `~/teselas/personal` (the imported real Logseq mosaic).
- Vite dev on `:5173`.

## Recent Session Notes

- Phase 14.2 frontend perf smoke suite is in place under `web/tests/perf/`, with a runner that creates a medium fixture mosaic, starts `tesela-server` and Vite on dynamic localhost ports, runs Playwright, and records JSONL timings.
- `tesela-fixtures` now seeds built-in Task/Status/Priority/Deadline/Scheduled pages so generated mosaics have task board property metadata before the server's initial index.
- Phase 14.3 perf workflow is in `.github/workflows/perf.yml`: nightly/main uploads Criterion baselines, PRs diff with `critcmp`, and comments only when a benchmark regression exceeds 10%.
- **2026-05-19 — iOS bottom chrome rewrite**: `AppShell.swift` uses iOS 26 native `TabView` with `Tab(role: .search)` for the search slot, replacing the hand-rolled `BottomChrome` HStack.

- **2026-05-20 — iOS persistent capture bar + keyboard toolbar**: `CaptureSheet` deleted; replaced by a persistent `CaptureBar` in `tabViewBottomAccessory` (`Components/CaptureBar.swift`) — Slack-style composer (target chip + `+` attach stub + text field + mic/send), always visible, floats above keyboard. `CaptureTarget` enum (today/inbox/page/childOf) + `CaptureDefault` AppStorage setting. Block editing got a keyboard accessory toolbar (`.toolbar(placement: .keyboard)` in `BlockRow.swift`) — user-configurable item list (`KeyboardToolbarItem`, Settings → Capture → Keyboard toolbar; horizontally scrollable, Hide-keyboard pinned right). Voice recorder (`StreamingVoiceRecorder`) lifted to AppShell `@StateObject` to fix a Fence Hang from repeated AVAudioEngine init. Enter-on-empty-line splits to a new block. Inline `#tag` editing round-trips via `MockMosaicService.splitInlineTags`.

- **2026-05-20 — iOS multi-mosaic**: `MosaicProfile` + `MosaicRegistry` (device-local list persisted to UserDefaults, seeds first profile from legacy `backend.serverURL`). `MosaicChromeButton` replaces the old sync dot in all three TopBars — icon = active mosaic's symbol, color = reachability. `MosaicSwitcherSheet` + `MosaicEditView` for add/switch/edit. **Known limitation**: `tesela-server` is one-server-per-mosaic, so "Add mosaic" requires a separate server URL — see roadmap "Mosaic discovery + server-side multi-mosaic (PRIORITY)".

- **2026-05-20 — Logseq importer fidelity + backup trust**: `import_logseq.rs` `convert_content` now preserves block refs `((uuid))` literally, wraps `#+BEGIN_QUERY` blocks in ` ```query ` fences, rewrites `../assets/` → `../attachments/` URLs, respects code fences. `feature_coverage_audit` test covers every construct. **The real `~/logseq` vault was imported** into `~/teselas/personal` (462 pages, 268 assets, 7 whiteboards hard-skipped — clean). Trust artifacts: `tesela-cli` integration test `logseq_import_backup_restore_byte_exact_round_trip` (CLI path) + `tesela-server` test `http_backup_round_trip` (web-UI path) — both do import→backup→restore→byte-exact-diff. Backup/restore confirmed working from the web UI (`BackupSettings.svelte`).

- **2026-05-20 — web daily journal gap fix**: `JournalView.svelte` `visibleDailies` now builds a gap-free descending calendar from today back to the oldest real daily, filling missed days with synthetic empty placeholders.

- **2026-05-20 — Apple Reminders sync EventKit fix**: `reminders/darwin.rs` now routes all EventKit access through one process-wide `EKEventStore` (`shared_event_store()`, `OnceLock`-backed) instead of constructing a fresh store in `request_access`, `fetch_reminders`, and `push_all`. A single `sync_all` previously built four stores (the access request runs inside both `pull_all` and `push_all`); auto-sync every 5 min exhausted EventKit's per-process instance cap within ~an hour, after which every sync failed with "too many EKEventStore instances". Regression test `shared_event_store_is_a_process_singleton` (9/9 darwin reminders tests green); `tesela-server` release binary rebuilt and restarted; manual sync verified clean (no errors).

- **2026-05-20 — iOS option 1 (finish the HTTP client)**: Correction to prior docs — `MockMosaicService` is the iOS app's *real* dual-mode (mock + HTTP) service, not a mock; the app has shipped ~31 internal phases (real page load/edit/writeback, search, voice) tracked only in git, not in this roadmap. This slice wired the still-mock derived data: real per-page backlinks (`GET /notes/{id}/backlinks` → `loadedBacklinks` dict, fetched in `loadPage`), per-page outline derived from loaded blocks (`OutlineEntry.derive`), and a local UserDefaults-backed pinned store with a star toggle on `PageView`. Global `backlinks`/`outline` dropped from the `MosaicService` protocol. iOS app builds for the simulator and launches clean; backlinks endpoint contract curl-verified. **Not yet done:** interactive Peek QA against a live server (needs device/simulator tap-through). Next iOS steps, in order: server-side multi-mosaic, then write a proper iOS roadmap doc — see memory `project_ios_next_roadmap`.

- **2026-05-20 — iOS option 2 (server-side multi-mosaic, approach B)**: iOS can now discover, add, and switch between the mosaics a single `tesela-server` hosts — no more one-server-per-mosaic. Approach B: the server stays single-mosaic (it already had `/mosaics/discovered|current|switch` + `/server/restart` + `POST /mosaics` — **no server changes needed**); iOS drives switch-via-restart and the existing auto-reconnect rides out the ~2-3s reboot. iOS changes (12 files): `MosaicProfile.mosaicPath`; `MosaicServerClient` (endpoint wrapper, in `MosaicRegistry.swift`); `MosaicRegistry.importDiscovered`; `MockMosaicService.ensureServerMosaic` + a `.switching` connection state; `AddMosaicView` (discover-and-pick + create-new, in `MosaicSwitcherSheet.swift`); `AppShell.activateMosaic`; pairing handoff (`PairScanView`/`PairWithShortCodeView` `adopt` → `importDiscovered`). Verified end-to-end in the simulator via idb: discovery listed all 3 real mosaics, add → switcher, and switch+restart (app reconnected onto `logseq`, chrome read "logseq, connected"). QA caught and fixed a dead-tap-gap (`.contentShape`) on the switcher/discovery rows. iOS app builds for the simulator.

- **2026-05-20 — iOS roadmap doc**: `.docs/ai/roadmap.md` gained an "iOS App — Phases" section reconstructing the iOS app's Phases 0–31+ from git history — the iOS app (35 commits) was never tracked in the roadmap. Refreshed stale iOS claims in the `project_v4_redesign` (iOS work long since resumed) and `project_mobile_strategy` (app shipped HTTP-client, not the UniFFI-embedded core) memories.

- **2026-05-20 — iOS Peek lenses finished**: `PageView`'s Peek surface now shows real data on every lens. `PropsLens` shows real `created`/`edited` (`Page` gained a `created` field from frontmatter); `TasksLensView` shows the page's own task blocks; `GraphLensView` shows an outgoing-links list (`GET /notes/{id}/links`). Backlinks + graph rows are tap-to-navigate via the `tesela://page/` scheme (`Backlink` gained a `pageId`). A real in-app graph *render* is a wanted later item — the list is the interim. iOS app builds for the simulator.

- **2026-05-20 — iOS transcription download crash fixed**: every model download crashed on completion — `didFinishDownloadingTo` runs on URLSession's background delegate queue but called `MainActor.assumeIsolated` (a hard crash off the main actor). Parakeet's catalog URL is a dead 404, so it "finished" instantly and crashed near-on-tap. Fixed: compute the destination via a `nonisolated` static helper, and reject non-2xx responses (a 404 body was otherwise saved as a bogus model). Verified — Whisper-tiny downloads and completes cleanly in the simulator.

- **2026-05-20 — iOS Parakeet via FluidAudio**: wired on-device Parakeet transcription. Added the FluidAudio Swift package; `LocalTranscriptionEngine` dispatches by model family (Whisper → SwiftWhisper, Parakeet → a FluidAudio `AsrManager`); `TranscriptionStore` routes Parakeet downloads to `AsrModels.downloadAndLoad` (per-version cache dir under Application Support, indeterminate progress). Catalog now lists 3 Parakeet entries — v2/v3/110M — all `inferenceSupported: true`, no `downloadURL` (a `parakeetVersion` token maps to `AsrModels.Version`). iOS app builds clean and launches with FluidAudio linked, no crash. **Not runtime-verified:** the actual Parakeet model download (hundreds of MB from HuggingFace) + transcription — best confirmed on device. See decision log.

- **2026-05-20 — iOS QR pairing camera freeze fixed**: the pairing scanner froze the UI for ~10s on open. Root cause: `QRScannerViewController.configureSession()` ran on the main thread in `viewDidLoad` — adding an `AVCaptureDeviceInput` acquires the camera hardware and blocks. `startRunning()` was already on a background queue, but configuration was not, and the two ran on *different* queues. Fix: a single dedicated serial `sessionQueue` now owns all `AVCaptureSession` calls (configure/start/stop), `viewDidLoad` only builds the preview layer on main, and config is batched in `beginConfiguration()`/`commitConfiguration()`. Moving config off-main made the silent `guard`-return failure asynchronous, so `PairScanView` gained an `onSessionError` callback + a "Camera unavailable" card (dismisses back to the pair screen, where typed-code pairing is one tap away). iOS app builds green for the `Tesela-Test` simulator. Not device-verified (camera timing needs hardware).

## Blockers

None.
