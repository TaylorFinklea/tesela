# Graphite Redesign — Shell Phase Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the Graphite *shell* — web topbar + widget-rail host + first-class pane container + vim status line + ⌘K command palette + leader chord overlay; iOS Graphite app-shell mirroring the proven tab bar + a Graphite header + capture sheet — all **new presentation bound to the existing, vetted behavior** (no behavior rebuilt). Pane/tab CONTENT stays a placeholder this phase; the daily-driver views fill it next.

**Architecture:** The hard-won interaction logic already exists and is framework-agnostic — the new shell *imports and renders* it. Web: new Svelte components under `web/src/lib/graphite/shell/` composed by `web/src/routes/g/+page.svelte` (the foundation's primitives gallery moves to `/g/primitives`). They bind to existing stores — `lib/stores/pane-state.svelte.ts`, `lib/buffer/state.svelte.ts` (workspace), `lib/v5/leader-tree.svelte.ts`, `lib/commands.ts`, `lib/fuzzy.ts`, `lib/stores/{recents,favorites,station,colon-mode,peek}.svelte.ts`, `lib/ws-client.svelte.ts` — and mirror the keydown/overlay wiring in `web/src/routes/v4/+layout.svelte` (the ONLY place behavior connects today). iOS: new `app/Tesela-iOS/Sources/Graphite/Shell/GrAppShell.swift` mirrors `Sources/Views/AppShell.swift`'s native iOS-26 TabView (4 tabs + `.search` role) bound to the SAME `MockMosaicService` + `RelayTicker` + `CaptureComposer`, themed `.graphite`, with Graphite-styled header + capture sheet + placeholder tab content. Old v4/v5 web + old iOS `AppShell`/Views are referenced only, never edited; deleted at cutover. `GrAppShell` is NOT yet the app entry (stays `#Preview`/dev-gated until cutover).

**Tech Stack:** Web — Svelte 5.55 (runes), SvelteKit 2.57, `@tabler/icons-svelte`. iOS — SwiftUI, iOS 26, xcodegen; existing `DesignSystem/` + the foundation's `Sources/Graphite/` primitives (GrButton/GrChip/GrTypeDot/GrTypeTag/GrRow/GrWidget/GrIcon).

**Depends on:** the foundation phase (`2026-05-29-graphite-foundation-plan.md`, landed `7083956`/`e316a6f`) — tokens + primitives exist. Reuse them; do not re-create.

**Scope note (writing-plans Scope Check):** web (Part A) and iOS (Part B) are independent subsystems sharing only token values; parallel-executable. Daily-driver VIEWS, the configurable widget system, splits *management*, and graph are explicitly OUT (later plans). This phase delivers the chrome with placeholder content.

**Tiering note (per AGENTS.md):** the Graphite CSS below is **spec-derived** — prescribed verbatim (extracted from `gr-shell.jsx`/`gr-overlays.jsx`). The store wiring is **codebase-derived** — the plan names the exact reuse APIs but requires the implementer to READ `v4/+layout.svelte` and the named store modules to mirror the idiom, NOT copy hand-written dispatch code. Where a step says "mirror v4," read it first.

---

## Verbatim CSS reference (all shell + overlay regions)

Scoped under `.gr-root` (the foundation's tokens.css already defines the vars there). Class names are the mockup's — keep them.

**Topbar** `.gr-top{display:grid;grid-template-columns:auto 1fr auto;align-items:center;gap:18px;padding:0 16px;border-bottom:1px solid var(--line);background:var(--surface);height:48px;}` · brand `.gr-brand{display:flex;gap:9px;align-items:center;} .gr-brand .nm{font-size:13.5px;font-weight:600;color:var(--fg);letter-spacing:-.01em;}` · tabs `.gr-tabs{display:flex;gap:4px;margin-left:6px;} .gr-tab{height:30px;padding:0 11px;border-radius:8px;display:flex;align-items:center;gap:7px;font-size:12.5px;color:var(--subtle);cursor:pointer;transition:all .14s;} .gr-tab.active{color:var(--fg);background:var(--raised);border:1px solid var(--line-2);} .gr-tab .kdot{width:6px;height:6px;border-radius:50%;background:var(--coral);}` · command bar `.gr-cmd{justify-self:center;width:min(440px,100%);height:32px;display:flex;align-items:center;gap:9px;padding:0 11px;border-radius:9px;background:var(--bg);border:1px solid var(--line-2);color:var(--subtle);cursor:pointer;font-size:12.5px;}` · right `.gr-icons{display:flex;gap:2px;}` (reuse foundation `.gr-ic` for icon buttons) · connection `.gr-conn i{width:7px;height:7px;border-radius:50%;background:var(--query);box-shadow:0 0 0 3px rgba(133,188,99,.16);}` (disconnected → `var(--faint)`, no glow).

**Body / rail / main** `.gr-body{display:flex;min-height:0;overflow:hidden;position:relative;flex:1;} .gr-rail{width:256px;flex-shrink:0;background:var(--surface);border-right:1px solid var(--line);display:flex;flex-direction:column;min-height:0;} .gr-rail-scroll{flex:1;overflow:auto;padding:12px 10px;display:flex;flex-direction:column;gap:8px;} .gr-main{flex:1;display:flex;min-width:0;min-height:0;}` (the foundation `.gr-w`/`.gr-w-head`/`.gr-w-body`/`.gr-addw`/`.gr-row`/`.gr-capture` are reused inside the rail).

**Pane** `.gr-pane{flex:1;min-width:0;display:flex;flex-direction:column;background:var(--bg);min-height:0;} .gr-pane.focus{flex:1.7;} .gr-pane.side{flex:1;background:var(--surface);border-left:1px solid var(--line);max-width:420px;} .gr-pane-head{display:flex;align-items:center;gap:11px;padding:14px 18px 12px;border-bottom:1px solid var(--line);flex-shrink:0;} .gr-pane-head .gr-back{color:var(--subtle);cursor:pointer;} .gr-pane-head .ttl{font-size:16px;font-weight:600;letter-spacing:-.01em;color:var(--fg);white-space:nowrap;} .gr-pane-head .sub{font-family:var(--mono);font-size:10.5px;color:var(--faint);} .gr-pane-head .sp{flex:1;} .gr-pane-head .meta{font-family:var(--mono);font-size:10.5px;color:var(--faint);white-space:nowrap;} .gr-pane-body{flex:1;overflow:auto;padding:14px 18px;}`

**Status line** `.gr-status{display:flex;align-items:center;gap:12px;padding:0 14px;height:30px;background:var(--surface);white-space:nowrap;overflow:hidden;border-top:1px solid var(--line);font-family:var(--mono);font-size:11px;color:var(--subtle);} .gr-status .mode{color:var(--coral);font-weight:700;letter-spacing:.10em;font-size:10px;} .gr-status .sep{color:var(--faint);} .gr-status .keys{margin-left:auto;display:flex;gap:14px;} .gr-status .keys kbd{color:var(--fg2);font-family:var(--mono);} .gr-status .clk{color:var(--faint);display:flex;align-items:center;gap:5px;}`

**Scrim + overlays** `.gr-scrim{position:absolute;inset:0;z-index:40;background:rgba(8,9,12,.58);backdrop-filter:blur(3px);display:flex;flex-direction:column;align-items:center;}` (leader variant sets `justify-content:flex-end` via inline). Command `.gr-cmdk{width:min(640px,92%);margin-top:72px;background:var(--raised);border:1px solid var(--line-2);border-radius:14px;box-shadow:0 28px 90px rgba(0,0,0,.55);overflow:hidden;} .gr-cmdk-in{display:flex;align-items:center;gap:12px;padding:16px 18px;border-bottom:1px solid var(--line);} .gr-cmdk-body{padding:8px;max-height:430px;overflow:auto;} .gr-cmdk-grp{font-family:var(--mono);font-size:9.5px;letter-spacing:.10em;text-transform:uppercase;color:var(--faint);padding:9px 12px 5px;} .gr-cmdk-row{display:grid;grid-template-columns:26px 1fr auto;align-items:center;gap:13px;padding:10px 12px;border-radius:9px;cursor:pointer;} .gr-cmdk-row.sel{background:var(--raised-3);} .gr-cmdk-row .lb{font-size:13.5px;color:var(--fg2);display:flex;align-items:center;gap:9px;white-space:nowrap;} .gr-cmdk-row .lb .desc{color:var(--faint);font-size:12px;} .gr-cmdk-row .rk{font-family:var(--mono);font-size:10.5px;color:var(--faint);display:flex;gap:4px;} .gr-cmdk-row .rk kbd{background:var(--surface);border:1px solid var(--line);border-radius:4px;padding:2px 6px;} .gr-cmdk-foot{display:flex;align-items:center;gap:16px;padding:10px 16px;border-top:1px solid var(--line);font-family:var(--mono);font-size:10.5px;color:var(--faint);}` Leader `.gr-leader{width:min(460px,92%);margin-top:auto;margin-bottom:46px;background:var(--raised);border:1px solid var(--line-2);border-radius:14px;box-shadow:0 28px 90px rgba(0,0,0,.55);overflow:hidden;} .gr-leader-head{display:flex;align-items:center;gap:9px;padding:13px 16px;border-bottom:1px solid var(--line);font-family:var(--mono);font-size:11px;color:var(--subtle);} .gr-leader-body{padding:8px;display:grid;grid-template-columns:1fr 1fr;gap:2px;} .gr-chord{display:flex;align-items:center;gap:11px;padding:9px 11px;border-radius:9px;cursor:pointer;} .gr-chord:hover{background:var(--raised-2);} .gr-chord .key{width:24px;height:24px;border-radius:6px;display:grid;place-items:center;background:var(--surface);border:1px solid var(--line-2);font-family:var(--mono);font-size:12px;color:var(--coral);flex-shrink:0;font-weight:600;} .gr-chord .cl{flex:1;font-size:13px;color:var(--fg2);} .gr-chord .more{color:var(--faint);} .gr-leader-foot{padding:9px 16px;border-top:1px solid var(--line);font-family:var(--mono);font-size:10.5px;color:var(--faint);}`

---

## Part A — Web shell

### Task A1: Move the primitives gallery; scaffold the shell entry

**Files:**
- Create: `web/src/routes/g/primitives/+page.svelte` (move the current gallery here)
- Modify: `web/src/routes/g/+page.svelte` (becomes the shell mount)

- [ ] **Step 1** — Move the current `/g/+page.svelte` gallery body into a new `web/src/routes/g/primitives/+page.svelte` (verbatim; it keeps working at `/g/primitives` for primitive QA).
- [ ] **Step 2** — Replace `web/src/routes/g/+page.svelte` with a mount of the shell component (built in A8):

```svelte
<!-- web/src/routes/g/+page.svelte -->
<script lang="ts">
  import GraphiteShell from '$lib/graphite/shell/GraphiteShell.svelte';
</script>
<GraphiteShell />
```

- [ ] **Step 3** — Commit: `git add web/src/routes/g/ && git commit -m "feat(graphite-web): move primitives gallery to /g/primitives; /g mounts the shell"`

### Task A2: GrTopBar

**Files:**
- Create: `web/src/lib/graphite/shell/GrTopBar.svelte`

Reuse: `getConnected()` (ws-client) for the connection dot; the workspace store for open-page tabs (`getActiveTab()`, the tab list — READ `lib/buffer/state.svelte.ts` for the exact tab accessor + shape; `switchTab(tabId)`); `openStation()` (station store) for the ⌘K bar click; `openFullscreenGraph()` + `openSettingsOverlay()` (pane-state) for the graph/settings icons. Use the foundation `GrIcon` + `.gr-ic`.

- [ ] **Step 1** — READ `lib/buffer/state.svelte.ts` (tab accessors) + `lib/ws-client.svelte.ts` (`getConnected`) + `lib/stores/station.svelte.ts` + `lib/stores/pane-state.svelte.ts` to confirm the exact exported names.
- [ ] **Step 2** — Write `GrTopBar.svelte`: the `.gr-top` grid — left `.gr-brand` (a mosaic-mark SVG placeholder + "tesela"), center `.gr-tabs` mapping the workspace tabs (`.gr-tab`, active gets `.kdot`, click → `switchTab`), the `.gr-cmd` bar (click → `openStation()`; shows a `GrIcon name="search"` + "Search or run a command…" + a mono "⌘K"), right `.gr-icons` with `.gr-ic` buttons: mic, connection dot (`.gr-conn` colored by `getConnected()`), graph (→ `openFullscreenGraph()`), settings (→ `openSettingsOverlay()`). All colors via the CSS above (vars only).
- [ ] **Step 3** — Verify it compiles in isolation (`pnpm exec svelte-check --threshold error` shows no new error in the file). Commit `feat(graphite-web): GrTopBar`.

### Task A3: GrRail (widget host)

**Files:**
- Create: `web/src/lib/graphite/shell/GrRail.svelte`

Parity widget set (fixed; configurability is iterate-phase): **Quick capture** (a `.gr-capture` row → `openColonMode()` / capture), **Pinned** (`getPinnedTabs()` / `getFavorites()` → `GrRow`s), **Today** (badge = count; placeholder rows or `getRecents()`), **Tasks** (Doing/Next groups — placeholder rows this phase). Each widget = the foundation `GrWidget` containing `GrRow`s. Bottom `.gr-addw` "Add widget" (stub — `console.log` or a toast; configurability deferred).

- [ ] **Step 1** — READ `lib/stores/{favorites,recents,station,colon-mode}.svelte.ts` for the exact accessors.
- [ ] **Step 2** — Write `GrRail.svelte`: `.gr-rail > .gr-rail-scroll` containing the four `GrWidget`s (reuse foundation component) + the `.gr-addw` affordance. Pinned/Today pull from the stores where available; Tasks rows are static placeholders labeled as such (real data = views phase). Quick-capture row → `openColonMode()`.
- [ ] **Step 3** — Verify compiles; commit `feat(graphite-web): GrRail widget host`.

### Task A4: GrPane (container, placeholder body)

**Files:**
- Create: `web/src/lib/graphite/shell/GrPane.svelte`

The first-class pane chrome. Props: `title`, `subtitle?`, `meta?`, `canBack?`, `variant?: 'focus' | 'side'`, a content slot. Reuse pane-state for the focused buffer's title/path if available, but accept props so the shell can pass them. Splits-ready: the `variant` sets `.focus`/`.side` flex. Body = a slot; the shell passes a placeholder this phase.

- [ ] **Step 1** — Write `GrPane.svelte` with the `.gr-pane`/`.gr-pane-head`/`.gr-pane-body` CSS above; head renders optional `.gr-back` (GrIcon arrow-left), `.ttl`, `.sub`, `.sp`, `.meta`, and an actions slot (for `GrButton`/`GrTypeTag`). Body = `{@render children()}`.
- [ ] **Step 2** — Verify compiles; commit `feat(graphite-web): GrPane container`.

### Task A5: GrStatus

**Files:**
- Create: `web/src/lib/graphite/shell/GrStatus.svelte`

Reuse `getVimMode()` (pane-state) for the mode pill; the breadcrumb from the focused buffer/journey (READ `lib/stores/pane-state` + `lib/stores/journey.svelte.ts` for the path accessor; placeholder text if not trivially available). Clock = a `$state` updated on an interval. Contextual keys = a prop (the shell/active view supplies them; static defaults this phase).

- [ ] **Step 1** — Write `GrStatus.svelte`: `.gr-status` with `.mode` (`getVimMode()` uppercased, default NORMAL), a `.sep` + breadcrumb path text, `.keys` (margin-left:auto) mapping a `keys` prop of `{k,label}` to `<span><kbd>{k}</kbd> {label}</span>`, and `.clk` (GrIcon clock + `HH:MM` from a clock `$state`, updated via `setInterval` in `$effect`, cleared on teardown).
- [ ] **Step 2** — Verify compiles; commit `feat(graphite-web): GrStatus line`.

### Task A6: GrCommandPalette (⌘K) — new presentation, reused behavior

**Files:**
- Create: `web/src/lib/graphite/shell/GrCommandPalette.svelte`

Reuse (do NOT reimplement): `isStationOpen()`, `getStationInitialQuery()`, `closeStation()` (station store); `buildCommands(deps)` + `matchesQuery` (commands.ts); `scoreFuzzy(label, filter)` + `highlightRuns` (fuzzy.ts). READ `web/src/routes/v4/+layout.svelte` + the existing Station component it renders to see how `buildCommands` deps are assembled + how selection/exec works — MIRROR that logic; only the markup/CSS is new (the `.gr-cmdk` spec above). Group by `Command.category` into the `.gr-cmdk-grp` sections ("Jump to" / "Actions" per the mockup), render each as a `.gr-cmdk-row` (GrIcon + `.lb` (+`.desc`) + `.rk` kbd chips from `Command.shortcut`/`keywords`). Selected row `.sel`; ↑/↓ moves selection, Enter runs `command.action()` + `closeStation()`, Esc closes. Footer = nav hints.

- [ ] **Step 1** — READ `v4/+layout.svelte` (station render + the `buildCommands` deps object) and the existing Station/command-palette component to learn the exact `Command` shape, deps, fuzzy usage, and exec path.
- [ ] **Step 2** — Write `GrCommandPalette.svelte`: `{#if isStationOpen()}` → `.gr-scrim` (center) > `.gr-cmdk` with input (bound `$state` query, seeded from `getStationInitialQuery()`), `.gr-cmdk-body` rendering filtered+grouped commands via `buildCommands`+`scoreFuzzy`, keyboard handling (↑/↓/Enter/Esc), footer. Use `highlightRuns` for match highlighting in `.lb`.
- [ ] **Step 3** — Verify compiles; commit `feat(graphite-web): GrCommandPalette (Graphite ⌘K over existing command registry)`.

### Task A7: GrLeaderOverlay (Space) — new presentation, reused behavior

**Files:**
- Create: `web/src/lib/graphite/shell/GrLeaderOverlay.svelte`

Reuse: `isLeaderOpen()`, `getLeaderInitialPath()`, `openLeader(path?)`, `closeLeader()`, `getLeaderTree(): ChordNode[]` (leader-tree). READ `v4/+layout.svelte` + the existing ChordMenu component to see how a chord node's `action?`/`children?` are traversed (selecting a key with children descends; with an action executes + closes) — MIRROR that; only markup/CSS is new (the `.gr-leader` spec). Render the current level's nodes in the `.gr-leader-body` 2-col grid as `.gr-chord` (`.key` glyph = the node `key`; `.cl` = `label`; `.more` chevron if `children`). Header shows the breadcrumb (`Space` kbd + path); footer = esc hint. Pressing a node's key descends/executes.

- [ ] **Step 1** — READ `lib/v5/leader-tree.svelte.ts` (the `ChordNode` shape + traversal helpers) + the existing ChordMenu component for the keypress→descend/execute idiom.
- [ ] **Step 2** — Write `GrLeaderOverlay.svelte`: `{#if isLeaderOpen()}` → `.gr-scrim` with inline `justify-content:flex-end` > `.gr-leader`. Render `getLeaderTree()` at `getLeaderInitialPath()`; key handler descends (`openLeader([...path, key])`) or runs `node.action()` + `closeLeader()`; Esc closes (or pops a level). Mirror v4's ChordMenu exactly for the dispatch.
- [ ] **Step 3** — Verify compiles; commit `feat(graphite-web): GrLeaderOverlay (Graphite leader over existing chord tree)`.

### Task A8: GraphiteShell — compose + wire keydown

**Files:**
- Create: `web/src/lib/graphite/shell/GraphiteShell.svelte`

The root: composes GrTopBar / (GrRail + GrMain with one GrPane) / GrStatus, mounts GrCommandPalette + GrLeaderOverlay, and attaches the capture-phase keydown listener. READ `web/src/routes/v4/+layout.svelte` lines around the keydown listener (the recon flagged: a single capture-phase keydown at mount dispatching Space→`openLeader()`, ⌘K→`openStation()`, `:`→`openColonMode()`, all behind text-entry guards via the `isTextEntry`/`isTextEntry()`-style helper at ~line 101) — MIRROR that wiring exactly (import the same guard + store fns). The pane body for this phase = a placeholder component (`GrPanePlaceholder` inline: a centered faint "Daily-driver views land in the next phase" + the focused buffer title if available).

- [ ] **Step 1** — READ `v4/+layout.svelte` fully (the keydown dispatch + text-entry guard + which stores it touches). List the exact guard helper + store fns to import.
- [ ] **Step 2** — Write `GraphiteShell.svelte`: the `.gr-root` is provided by the `/g` `+layout` (foundation) — so this renders the topbar + `.gr-body`(rail + `.gr-main` > one `GrPane variant="focus"` with the placeholder body) + status, plus the two overlays. Attach the keydown listener in `$effect` (capture phase, removed on teardown), mirroring v4. Wire GrStatus mode/path from the stores.
- [ ] **Step 3** — Verify compiles; commit `feat(graphite-web): GraphiteShell — compose chrome + mirror v4 keydown wiring`.

### Task A9: Web shell gate

- [ ] **Step 1** — `cd web && pnpm exec svelte-check --threshold error 2>&1 | tail -20`. No NEW errors in `src/lib/graphite/` or `src/routes/g/` (the lone pre-existing v4 VoiceCaptureButton error is OK).
- [ ] **Step 2** — Visual + interaction QA (Chrome DevTools MCP, if the browser is free): load `/g`, confirm the topbar/rail/pane/status render to spec; press **⌘K** → Graphite command palette opens with grouped real commands, typing filters, Esc closes; press **Space** (outside a text field) → Graphite leader overlay opens with the chord grid, a key descends/executes, Esc closes; the connection dot reflects `getConnected()`. Note any drift; fix in the offending component.
- [ ] **Step 3** — Commit any fixes (`fix(graphite-web): …`).

---

## Part B — iOS shell

### Task B1: GrAppShell — Graphite tab shell (mirror AppShell, reuse services)

**Files:**
- Read: `app/Tesela-iOS/Sources/Views/AppShell.swift`, `Components/AppTab.swift`, `Data/MosaicService.swift`, `Data/RelayTicker.swift`, `Components/CaptureBar.swift`
- Create: `app/Tesela-iOS/Sources/Graphite/Shell/GrAppShell.swift`

- [ ] **Step 1** — READ `AppShell.swift` to learn the exact structure: `TabView(selection:)`, the `Tab(value:systemImage:)` calls, the `.search` role tab, the `@StateObject mosaic/relayTicker`, `CaptureComposer`/`StreamingVoiceRecorder` wiring, the `TeselaAppearance` theme wrap, and how tab content views receive `mosaic/appearance/syncState/relayTicker`. Also read `AppTab.swift` (the enum: `.daily/.agenda/.inbox/.library/.search`, `label`, `systemImage`, `places`).
- [ ] **Step 2** — Write `GrAppShell.swift`: a new `View` mirroring AppShell's `TabView(selection:)` with the 4 `Tab`s (Daily/Agenda/Inbox/Library) + the `.search` role tab, bound to a `@StateObject MockMosaicService` + `@StateObject RelayTicker` (same types), wrapped in `TeselaAppearance` forced to `Theme.graphite` (or `.environment(\.theme, .graphite)`). Each tab renders a `GrTabPlaceholder(tab:)` (a new inline Graphite placeholder — see B3) — NOT the old views. Reuse the system Liquid Glass tab bar (no custom chrome).
- [ ] **Step 3** — (gate folded into B4) Commit `feat(graphite-ios): GrAppShell — Graphite tab shell over existing TabView + services`.

### Task B2: GrHeader (Graphite per-tab header)

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/Shell/GrHeader.swift`

- [ ] **Step 1** — Write `GrHeader.swift`: a Graphite-styled header `View` (large-title style per the mobile mockup `grm-shell.jsx`) — title (`theme.fgDefault`, the page-title type role), optional subtitle (mono, `theme.fgFaint`), trailing action slot. Reads `@Environment(\.theme)`. (Mirror the mobile design's header treatment; read `.docs/ai/design/graphite/mobile/grm-shell.jsx` for the exact sizes/spacing.)
- [ ] **Step 2** — Commit `feat(graphite-ios): GrHeader`.

### Task B3: GrCaptureSheet + GrTabPlaceholder

**Files:**
- Create: `app/Tesela-iOS/Sources/Graphite/Shell/GrCaptureSheet.swift`
- Create: `app/Tesela-iOS/Sources/Graphite/Shell/GrTabPlaceholder.swift`

- [ ] **Step 1** — READ `Components/CaptureBar.swift` + `CaptureComposer` to learn the pill→sheet structure + the `capture(text, target:)` call + the `manualTarget`/`draft`/`isExpanded` state. Write `GrCaptureSheet.swift`: the Graphite-styled capture sheet (the mobile `grm-sheet`/`grm-sheetdim` — input + a native input chooser + a dimmer overlay during expansion), bound to the same `CaptureComposer` + `MosaicService.capture`. Mirror CaptureBar's behavior; new Graphite presentation (use foundation primitives + theme).
- [ ] **Step 2** — Write `GrTabPlaceholder.swift`: a simple Graphite-themed placeholder `View(tab: AppTab)` showing the tab label + a faint "view lands next phase" — wraps `GrHeader`. (Real Daily/Agenda/Inbox/Library views are the next plan.)
- [ ] **Step 3** — Commit `feat(graphite-ios): GrCaptureSheet + GrTabPlaceholder`.

### Task B4: iOS shell gate — preview + build

**Files:**
- Modify: `app/Tesela-iOS/Sources/Graphite/Shell/GrAppShell.swift` (add a `#Preview`)

- [ ] **Step 1** — Add `#Preview { GrAppShell().environment(\.theme, .graphite) }` (or with a mock mosaic, matching how AppShell previews). Do NOT change `TeselaApp.swift`'s entry (GrAppShell becomes the app root only at cutover).
- [ ] **Step 2** — Build: `cd app/Tesela-iOS && xcodegen generate && xcodebuild -scheme Tesela -sdk iphonesimulator -configuration Debug -destination 'generic/platform=iOS Simulator' build 2>&1 | tail -20`. Fix compile errors until `** BUILD SUCCEEDED **`. (IDE SourceKit may show false-positive cross-file errors — xcodebuild is authoritative.)
- [ ] **Step 3** — Visual check via the Xcode preview / sim: GrAppShell shows the 4-tab glass bar + search circle, Graphite header, capture sheet opens, placeholder content themed. Commit `feat(graphite-ios): GrAppShell preview + shell gate`.

---

## Shell done — exit criteria

- Web `/g` renders the full Graphite shell (topbar + rail + pane + status); ⌘K opens the Graphite command palette over the REAL command registry; Space opens the Graphite leader over the REAL chord tree; connection dot live. `svelte-check` clean for graphite.
- iOS `GrAppShell` builds + previews: native tab bar + search circle, Graphite header + capture sheet, placeholder tabs, `.graphite` theme. `xcodebuild` SUCCEEDED. Not yet the app entry.
- Behavior is 100% reused (stores/commands/leader-tree/MosaicService/RelayTicker) — only presentation is new. Old UI untouched.
- **Next plan:** Daily-driver views — daily journal, page/outliner (reuse the CodeMirror BlockOutliner), inbox triage, agenda week, search — filling the pane/tab bodies. Then the parity check + cutover.

---

## Self-review (against the spec + recon)

- **Spec shell inventory** (topbar/widget-rail/panes/status/⌘K palette/leader overlay; iOS tab bar/header/capture sheet) → A2/A3/A4/A5/A6/A7 + B1/B2/B3. ✓
- **Reuse-vs-rebuild** → every interactive task names the existing store/fn to import + says "mirror v4," builds only presentation. Behavior modules unmodified. ✓
- **Rail = widget host, parity = fixed set** (A3) → fixed Capture/Pinned/Today/Tasks + stubbed Add widget; configurability deferred. ✓
- **Panes first-class, splits-ready** (A4) → `.focus`/`.side` variants + flex ratios; split *management* deferred. ✓
- **iOS reuses Loro FFI/MosaicService + the proven tab bar** (B1) → GrAppShell mirrors AppShell, binds same `MockMosaicService`/`RelayTicker`/`CaptureComposer`; old AppShell/Views untouched; not the entry until cutover. ✓
- **Vim status** (A5) → reads `getVimMode()`; display-only per the mockup. ✓
- **Type/name consistency** → web shell components under `lib/graphite/shell/`, all `Gr*`-prefixed, reuse the foundation primitives; iOS under `Sources/Graphite/Shell/`. ✓
- **Codebase-derived wiring not hand-coded** → A6/A7/A8/B1/B3 each begin with a READ step of the real source (`v4/+layout.svelte`, leader-tree, station/command components, AppShell, CaptureBar) before writing — no prescribed dispatch code I haven't verified. ✓
- **Content out of scope** → pane/tab bodies are placeholders; daily-driver views are the explicit next plan. ✓
