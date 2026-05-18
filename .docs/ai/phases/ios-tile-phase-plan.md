# Tesela iOS (Tile) — phase plan

Companion to:
- [`.docs/designs/2026-05-17-ios-design-brief.md`](../../designs/2026-05-17-ios-design-brief.md) — original brief
- [`.docs/designs/2026-05-18-ios-design-followup.md`](../../designs/2026-05-18-ios-design-followup.md) — 15 locked decisions

This file is the ordered build plan for the native SwiftUI iPhone app
in [`app/Tesela-iOS/`](../../../app/Tesela-iOS/). Each phase ships a
coherent slice that compiles + runs in the simulator.

## Context

The existing scaffold (`Tesela-iOS/Sources/{TeselaApp,ContentView}.swift`)
prints a version string from the Rust core via UniFFI. It builds clean
against `iphonesimulator` (verified 2026-05-18).

The Rust FFI surface (`crates/tesela-sync-ffi`) currently exposes only
sync-pairing primitives (`generateDeviceIdHex`, `encodePairingCode`,
`decodePairingCode`, `generateGroupIdentity`, `syncSchemaVersion`,
`teselaSyncVersion`). **No mosaic / page / block API yet.** That gap is
addressed late in the plan (Phase 13).

Until then, the app runs against a Swift-side mock data store mirroring
the design canvas's `data.jsx`. The mock store is structured the same
way the eventual FFI-backed store will be, so swapping it is mechanical.

Per `feedback_full_vision_first.md`: each phase targets the locked v0.5
vision, not an intermediate stepping stone.

## Phase 0 — Phase plan + Xcode project audit

**Goal:** know the shape of the build before pouring any code.

**Scope:**
- This file.
- Verify `xcodegen generate` + `xcodebuild ... iphonesimulator` builds
  clean against the current scaffold.
- Note the available simulator destinations and pick one as the dev
  driver (`Tesela-Test` simulator already exists).

**Files likely touched:** this file only.

**Exit criteria:**
- `.docs/ai/phases/ios-tile-phase-plan.md` committed.
- `xcodebuild` against `Tesela-Test` returns `BUILD SUCCEEDED`.

## Phase 1 — Design tokens + 16 themes

**Goal:** `Theme.swift` is the single source of truth for color, type,
density. SwiftUI views read tokens through environment injection.

**Scope:**
- `Sources/DesignSystem/Theme.swift` — `Theme` struct with role tokens
  (bg, bg-2, bg-3, bg-4, line, line-soft, fg-default, fg-muted,
  fg-subtle, fg-faint, accent-primary, accent-secondary, type-task,
  type-event, type-note, type-project, type-person, type-query,
  type-template).
- `Sources/DesignSystem/Themes/*.swift` — one file per theme palette
  (Prism indigo default, plus 16 dark variants from
  `web/src/themes.css`).
- `Sources/DesignSystem/TypeScale.swift` — semantic styles
  (pageTitle, sectionTitle, heading, body, bodyCompact, caption, chip,
  statusLine).
- `Sources/DesignSystem/Density.swift` — `DensityTier` enum
  (Comfortable / Compact / Compact+) with size multipliers.
- `Sources/DesignSystem/EnvironmentKeys.swift` — `@Environment(\.theme)`
  and `@Environment(\.density)` keys.
- `Sources/DesignSystem/Color+Hex.swift` — hex initializer helper.

**Default theme:** Prism indigo (`#7b8cff` `--accent-primary`). Matches
web v5 default. Per decision #1.

**Files likely touched:** new files only; existing `ContentView.swift`
untouched until Phase 2.

**Exit criteria:**
- `xcodebuild` clean.
- A throwaway preview swatch view renders all 16 themes side-by-side
  to verify color mappings.

**Dependencies:** Phase 0.

## Phase 2 — Component primitives

**Goal:** the atoms and chrome the rest of the app reuses.

**Scope:**
- `Sources/Components/BlockRow.swift` — task / note / project bullet,
  indent, tag chips, wiki-link styling.
- `Sources/Components/TagChip.swift` — trailing-cluster pill, with
  parent/leaf split.
- `Sources/Components/InlineTagMark.swift` — inline (not chip) tag
  styling for body text.
- `Sources/Components/WikiLink.swift` — `[[Tag system]]` styling.
- `Sources/Components/Pill.swift` — sync status pill (synced/warn/err).
- `Sources/Components/KindBadge.swift` — `note` / `tag` / `query`
  pill with type-color tint.
- `Sources/Components/SectionEyebrow.swift` — mono-uppercase header.
- `Sources/Components/BottomTabBar.swift` — three-tab nav with
  Tabler-shaped icons.
- `Sources/Components/CaptureBar.swift` — composer with mic + send,
  fused palette-mode hook (the verb chip strip lands in Phase 7).
- `Sources/Components/TopBar.swift` — Today / page title chrome.
- `Sources/Components/Icons.swift` — Tabler-style stroked SVG set
  ported as `Path` glyphs. No SF Symbols.

**Exit criteria:**
- A `ComponentGallery.swift` debug preview renders every primitive in
  isolation, against the default theme. Looks like the v0.5 canvas
  atoms.

**Dependencies:** Phase 1.

## Phase 3 — Mock data store

**Goal:** the app has a realistic mosaic to render before any real I/O.

**Scope:**
- `Sources/Data/MockMosaic.swift` — `@Observable` class with
  realistic daily blocks, pages (with `type:` frontmatter), tags
  (parent/child), recent, pinned, search results, palette verbs,
  backlinks, outline. Same shape as `t7-review/data.jsx`.
- `Sources/Data/Models/` — `Page`, `Block`, `Tag`, `PaletteVerb`,
  `Backlink`, `SearchResult` structs. Each carries the fields the
  views need.
- `Sources/Data/MosaicService.swift` — protocol the views call
  through (`listPages`, `getPage`, `getBacklinks`, `search`,
  `toggleTask`, `addBlock`). `MockMosaicService` implements it. The
  eventual `FFIMosaicService` will implement the same protocol.

**Exit criteria:**
- `xcodebuild` clean.
- A throwaway preview view lists pages from the mock store.

**Dependencies:** Phase 1.

## Phase 4 — Daily tab (front door)

**Goal:** app launches into today's daily; capture bar visible; tab
bar visible. End-to-end visible slice.

**Scope:**
- `Sources/Views/DailyView.swift` — top bar (Today + date + sync pill),
  vertical scroll of blocks for today, yesterday section, capture
  bar, bottom tab bar.
- `Sources/Views/Shell/AppShell.swift` — the three-tab top-level
  scaffold; only Daily is wired this phase (Library + Search are
  empty-state placeholders).
- Wire mock task-toggle so tapping a task checkbox flips done state
  and writes back through the mock service.

**Exit criteria:**
- Launching the app in the simulator shows today's daily front door,
  matching the v0.5 canvas Tile-A screen.
- Tapping a task toggles its done state visually.
- Capture bar visible above the tab bar (composer chrome only — palette
  mode is Phase 7).

**Dependencies:** Phases 1–3.

## Phase 5 — Library tab (flat list + type-filter strip)

**Goal:** Library = one flat page list with the type-filter chip strip
across the top. Per decision #2.

**Scope:**
- `Sources/Views/LibraryView.swift` — flat list of all pages with
  type-filter strip (All · Pages · Tags · Daily · Projects · People ·
  Queries · Workspace · Scratch).
- Type-filter strip behavior: horizontal scroll on narrow widths,
  active chip highlighted in `accent-primary`.
- Tag rows render the tag-page style (slug + parent path + count).
- Recent / Pinned: sticky eyebrow rows at the top of "All" filter
  showing the last 5 recents + pinned set.
- Workspace filter chip exists but tapping shows a small "coming in
  Phase 11" placeholder.

**Exit criteria:**
- Library tab loads with the flat list.
- Filter chips switch which subset is visible.
- Workspace chip placeholder reads "Phase 11" not "TODO".

**Dependencies:** Phase 4.

## Phase 6 — Page view (body + tag chips strip + collapsible Peek)

**Goal:** tapping a page row in Library opens the page view with body,
tag chips, and the derived-only collapsible Peek segments below the
body. Per decisions #7 and #8.

**Scope:**
- `Sources/Views/PageView.swift` — top bar (back chevron + pin + ⋯),
  title chrome (kind chip + slug + title + meta), tag chips strip
  (mirroring `PageTagsChips.svelte`), page body (block list), then
  the collapsible derived Peek segments below (backlinks · outline ·
  props · tasks · graph).
- `Sources/Components/PeekSegments.svelte`-equivalent in Swift —
  segmented control, derived-only (`page` is NOT a segment).
- `Sources/Views/Derived/*.swift` — one Swift view per derived
  renderer (BacklinksView, OutlineView, PropsView, TasksView,
  GraphView). All read from mock service.
- Wiki-link tap routes to the linked page (push onto the nav stack).
- `Sources/Views/PageTagsChips.swift` — chips with `×` remove + `+`
  picker that mirrors web's `PageTagsChips.svelte` behavior.

**Exit criteria:**
- Library row tap → PageView.
- Tag chips render under title; tapping a chip's `×` removes it from
  the page's frontmatter (in mock state); `+` opens the picker.
- Peek segments swap derived views; collapse/expand works.

**Dependencies:** Phase 5.

## Phase 7 — Search tab + fused verb palette in capture bar

**Goal:** Search tab works against the mock corpus; capture bar
`:` prefix flips to palette mode with verb chip suggestions. Per
decision #6.

**Scope:**
- `Sources/Views/SearchView.swift` — search field with grouped
  results (Pages · Blocks · Tags). No Verbs section per decision #6.
- `Sources/Components/CaptureBar.swift` — extend with palette mode:
  detect `:` prefix, show verb chip strip above the bar with
  filtered verbs, Send button label becomes "Run".
- Hitting `:scratch` → creates a scratch page (mock service) and
  opens it.
- Hitting `:promote` on a scratch → opens the title/location prompt
  sheet.

**Exit criteria:**
- Search tab returns mock results live as the user types.
- Typing `:scr` in capture bar shows the verb chip strip filtered
  to `:scratch`.
- `:scratch` creates and navigates.

**Dependencies:** Phases 4, 6.

## Phase 8 — Long-press block menu + properties sheet

**Goal:** long-press on a block opens the action sheet (verbs as
mono captions under friendly labels). Properties sheet edits
frontmatter.

**Scope:**
- `Sources/Components/BlockActionSheet.swift` — bottom sheet with
  Edit / Promote to page / Convert to tag / Indent / Move to / Copy
  block link / Archive / Delete.
- `Sources/Views/PagePropertiesSheet.swift` — bottom sheet from `⋯`
  on the page top bar; YAML-frontmatter row editor.

**Exit criteria:**
- Long-press a block on Daily → action sheet appears.
- `⋯ → Properties` from PageView opens the sheet; edits write back
  through the mock service.

**Dependencies:** Phases 4, 6.

## Phase 9 — Settings (main + theme picker + density)

**Goal:** Settings tree works with theme picker (all 16 themes) and
density toggle. Per decisions #10 and #15.

**Scope:**
- `Sources/Views/Settings/SettingsView.swift` — Mosaic / Workspace /
  Appearance / Sync / Bridges / Voice / Advanced groups.
- `Sources/Views/Settings/ThemePickerView.swift` — list of all 16
  dark themes with the role swatch row + checkmark on active.
- `Sources/Views/Settings/DensityView.swift` — Comfortable /
  Compact / Compact+ radio.
- `Sources/Views/Settings/VoiceView.swift` — top-level (not under
  Bridges per decision #12); Parakeet model status + language +
  auto-punctuation + split-on-pauses toggle. Mostly placeholder; no
  actual voice integration yet.
- `Sources/Views/Settings/BridgesView.swift` — Apple Calendar /
  Reminders / Shortcuts / Share / Files / API toggles. All Off by
  default.
- Settings is reachable from a `⋯` in the Library top bar.

**Exit criteria:**
- Selecting a theme repaints the entire app live (verify several).
- Density toggle changes the body text scale across all views.

**Dependencies:** Phases 1, 5.

## Phase 10 — Sync surfaces (status + peer list + pair flow)

**Goal:** Settings → Sync shows the connected state, peer list, and
the pair-device flow. Symmetric P2P language only. Per decision #4.

**Scope:**
- `Sources/Views/Settings/SyncView.swift` — connected banner, peer
  rows (no `host`/`relay` roles), strategy toggles, conflict policy,
  advanced (sync token, reset sync state).
- `Sources/Views/Settings/SyncDisconnectedView.swift` — the
  offline variant from the canvas, with retry + diagnose.
- `Sources/Views/Sync/PairDeviceView.swift` — QR code generated
  from a fresh pairing code (using
  `encodePairingCode` from the Rust FFI), 6-digit short code,
  active pairings list. Symmetric language ("Pair this iPhone with
  another device" — no "source of truth").
- `Sources/Views/Sync/ConflictResolutionView.swift` — the conflict
  sheet from the canvas. "This iPhone" / "Other device" / "Keep
  both". No source labeling.

**Exit criteria:**
- Pair flow generates a real (uniffi-validated) pairing code.
- Conflict sheet renders without role labels.

**Dependencies:** Phase 9.

## Phase 11 — Onboarding (pair-first)

**Goal:** first launch lands on onboarding; primary CTA is "Join
existing mosaic" → pair flow. Per decision #3.

**Scope:**
- `Sources/Views/Onboarding/OnboardingView.swift` — three bullets
  (Local-first, Capture from anywhere, Same mosaic) + primary CTA
  "Join existing mosaic" + secondary CTA "Create a new mosaic".
- `@AppStorage("onboardingComplete")` gates whether OnboardingView
  shows.
- "Join existing mosaic" pushes the Pair Device flow as the
  post-onboarding screen.

**Exit criteria:**
- First launch on a fresh simulator install shows Onboarding.
- Tapping "Join existing mosaic" pushes pair-device.
- "Create new mosaic" jumps to the empty-state Daily tab.

**Dependencies:** Phase 10.

## Phase 12 — Ambient buffers (Workspace filter chip)

**Goal:** the four ambients (Calendar / In Progress / Dashboard / AI
placeholder) live behind the Library Workspace filter chip. Per
decision #5.

**Scope:**
- `Sources/Views/Ambients/WorkspaceGridView.svelte`-equivalent —
  4-card grid revealed when Workspace chip is active.
- `Sources/Views/Ambients/CalendarView.swift` — month view; tap a
  day to open that daily page.
- `Sources/Views/Ambients/InProgressView.swift` — list of
  in-progress tasks across the mock corpus.
- `Sources/Views/Ambients/WorkspaceDashboardView.swift` — pinned
  widgets grid.
- `Sources/Views/Ambients/AIView.swift` — placeholder card.

**Exit criteria:**
- Workspace chip active → 4-card grid.
- Calendar tap-day → daily page navigation.

**Dependencies:** Phase 5.

## Phase 13 — Multi-page swipe-stack navigation

**Goal:** Safari-style page card stack. Per decision #14.

**Scope:**
- `Sources/Navigation/PageStack.swift` — `@Observable` stack of
  currently-open pages.
- Swipe up from above the tab bar reveals a horizontal carousel
  of open page cards.
- Tap a card to jump; swipe a card up to dismiss.
- Stack persists across app launches via `@AppStorage`.

**Exit criteria:**
- Open ≥3 pages by tapping wiki-links; carousel shows all 3.
- Carousel survives app relaunch.

**Dependencies:** Phase 6.

## Phase 14 — Modified marker (offline-pending-edits state)

**Goal:** `●` indicator appears on the page title when sync is offline
AND local edits haven't been seen by peers. Per decision #13.

**Scope:**
- `Sources/Sync/SyncState.swift` — `@Observable` exposing
  `isReachable: Bool` and `hasPendingEdits: Bool` (mocked this phase;
  real values come in Phase 15+).
- Page top bar conditionally renders the `●` to the left of the
  title when both flags are true.

**Exit criteria:**
- Toggle a debug "offline" switch in Settings → Sync; daily page
  title shows the `●` once an edit lands.

**Dependencies:** Phases 6, 10.

## Phase 15 — Real data layer (FFI expansion + filesystem source)

**Goal:** the mock service is swapped for a real one backed by a
filesystem mosaic + the Rust core. The Rust FFI surface expands to
expose page/block/tag operations.

**Scope:**
- New Rust FFI methods in `crates/tesela-sync-ffi/src/lib.rs`:
  `mosaicListPages(rootPath)`, `mosaicGetPage(pageId)`,
  `mosaicUpsertBlock(...)`, `mosaicGetBacklinks(...)`,
  `mosaicSearch(...)`. Each delegates to `tesela-core`.
- Regenerate Swift bindings; rebuild iOS sim library.
- `Sources/Data/FFIMosaicService.swift` — implements the same
  protocol as `MockMosaicService`.
- Toggle in Settings → Advanced ("Use mock data") so we can A/B
  the real vs. mock service.
- Wire `@AppStorage("mosaicRoot")` to choose the mosaic location
  (iCloud-Documents-backed, default).

**Exit criteria:**
- Pointing the app at a real mosaic folder shows real pages.
- Toggling "Use mock data" off + on swaps the service live.

**Dependencies:** Phases 1–14.

## Phase 16 — Polish + ship-ready states

**Goal:** loading skeleton, empty mosaic, share-sheet receiver,
home-screen widget, conflict-resolution UI.

**Scope:**
- `Sources/Views/States/LoadingSkeletonView.swift`
- `Sources/Views/States/EmptyMosaicView.swift`
- Share Extension target.
- Widget Extension target.
- Light QA pass; fix obvious paper cuts.

**Exit criteria:**
- App-Store submittable bundle (TestFlight first).

**Dependencies:** Phase 15.

---

## Open items not yet phased

These ride on later platform decisions; revisit when their phase
arrives:

- Voice capture (Parakeet v3) actual model integration — UI ships in
  Phase 9 as placeholder; the model integration is a separate task.
- Tailscale/relay UX once we have a real story for it.
- iPad layout — explicitly deferred per the original brief.
