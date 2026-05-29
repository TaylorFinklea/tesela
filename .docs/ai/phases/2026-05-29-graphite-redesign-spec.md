# Graphite Redesign — Spec (2026-05-29)

Status: **APPROVED approach, spec for review.** Executes AFTER the Loro cutover finishes.
Design source of truth: `.docs/ai/design/graphite/` (`Tesela Graphite.html`, `Tesela Mobile.html`, `graphite/*.jsx`, `mobile/*.jsx`, `screenshots/`). Render locally: `cd .docs/ai/design/graphite && python3 -m http.server` → open the HTML files; render one screen via the same `_render-one.html` pattern (`?screen=ScreenInbox|ScreenAgenda|ScreenCommand|ScreenLeader|ScreenGraph|ScreenTagPage|ScreenSettings`).

## Goal

Rebuild Tesela's web **and** iOS frontends as **brand-new frontends** against the Graphite design system, reach **daily-driver parity** with the current v5 app, then **delete the old frontends**. Graphite is a *productionized polish of the existing v5 IA* — same model (panes, ⌘K palette, vim leader/status line, outliner with inline block properties, ambients), new coherent visual system. NOT a new IA, NOT new product flows.

### Why new frontends (not a re-skin)
Past attempts updated the existing frontend in place, accreting hidden bugs + dead code. Single-user, pre-daily-driver — the right moment for a clean slate. New code trees, new components; port only the **vetted non-UI logic**; delete the old once at parity.

## Locked decisions (from 2026-05-29 brainstorm)

1. **Web stack:** clean **SvelteKit** rebuild — new app/route-tree, all-new components to the Graphite spec; port vetted non-UI logic; delete old v5 UI at parity. (NOT React, despite React mockups; NOT in-place reskin.)
2. **Scope:** web **+** iOS rebuilt **in parallel** against the shared Graphite system.
3. **iOS strategy:** new SwiftUI screen/view layer to the Mobile design; REUSE the working Loro FFI (`SyncEngineHandle`/`SyncCoordinator`), `RelayTicker`, `MosaicService` data layer, sync. (Mirrors web: new UI, keep vetted logic.)
4. **Timing:** **after** the Loro cutover completes (flag-day delete of SqliteEngine/DualEngine/op-wire, ai-business snapshot-dedup, DR drill). This doc is the spec; execution follows.
5. **Cutover bar:** **daily-driver parity, then iterate.** Cut over (delete old) once the real daily flows work better than the old app; defer rarely-touched surfaces to post-cutover polish.
6. **Rail = AnyType-style widget system.** Build the rail as a pluggable **widget host**. Parity pass ships a fixed widget set (Capture/Pinned/Today/Tasks); full add/remove/reorder + widget types is iterate-phase.
7. **Window splits: post-redesign.** Panes are first-class in the new shell (splits drop in later); split *management* is not built in the parity pass. (The leader already reserves a `w window…` chord.)

## Design system (Graphite)

One source of truth, expressed for both platforms (web CSS custom properties + iOS SwiftUI theme). Keep in sync via a single token definition.

- **Surfaces:** `bg #0E1014` → `surface #14171D` → `raised #1A1E26` → `raised-2 #20242D` → `raised-3 #272C37`; lines `rgba(255,255,255,.07/.12/.18)`.
- **Foreground ramp (contrast-lifted):** `fg #EDEFF2`, `fg2 #CBD0D9`, `muted #AAB0BB`, `subtle #8A909C`, `faint #646B78`.
- **Accent:** coral `#FF6B5A` (+ dim/line variants).
- **Type semantics:** task `#E8697F`, event `#62B8CE`, note `#E4AE66`, project `#7493E8`, person `#AE90E6`, query `#85BC63`. (Used for dots, chips, type tags, event blocks, graph legend.)
- **Fonts:** Geist (sans / content), JetBrains Mono (metadata, keys, properties, status line). **Icons:** Tabler.
- **Theming is a real feature** — the command palette shows a theme switcher ("Prism · Tokyo Night · +28"). Build Graphite as the default theme over a **tokenized, swappable** theme layer; ship Graphite-only at parity, additional themes (incl. the brand's Prism Light) in iterate. Tokens MUST be theme-indirection-ready from day one (no hardcoded hex in components).

Full component CSS is exhaustively defined in `graphite/gr-shell.jsx` (the single stylesheet) — treat it as the spec for spacing, radii, states, and every view's styling.

## IA + view inventory

### Shell (desktop, `gr-shell.jsx`)
`topbar (48px) / body (rail + panes + overlays) / status line (30px)`.
- **Top bar:** brand (mosaic mark + "tesela"), open-page tabs (active = coral dot), centered ⌘K command bar, right icons (mic, connection dot, graph ⌘G, settings).
- **Widget rail (256px):** the AnyType-style widget host. Parity widgets: Quick capture, Pinned, Today (badge), Tasks (Doing/Next groups), + "Add widget".
- **Panes:** first-class container (outliner / side-refs / focus variants). Splits-ready.
- **Status line:** vim mode (NORMAL/INSERT/LEADER/GRAPH) · breadcrumb path · contextual keys · clock.
- **Overlays:** command palette (⌘K, grouped: Jump to / Actions, keybind hints) + leader chord menu (Space → o/s/c/g/t/w/p/d/l/r) + graph.

### Daily-driver views (PARITY scope — required before cutover)
- **Daily — journal:** day-divider blocks, outliner editing, inline block properties, `#tags`, `[[links]]`, `@mentions`, type bullets.
- **Page / project — outliner:** block tree, inline block properties (`status::`, `priority::`, etc. as chips), linked-references side pane, properties pane (with chord shortcuts), type tag header.
- **Inbox — triage:** filter chips (All/Tasks/Notes/Voice/Events with counts), source-tagged cards (voice w/ duration, capture, sync), per-card actions (file/tag/archive/pin/done), contextual status keys.
- **Agenda — week:** time-grid (time gutter + day columns), type-colored event blocks, now-indicator, vim nav (h/l week, d day, t today).
- **Capture:** quick-capture (desktop rail widget + `c`; mobile sheet).
- **Search:** ⌘K search + command run.

### Deferred (iterate, post-cutover)
Graph (⌘G fullscreen), tag-as-table (schema + tagged-blocks table), full Settings (Sync/devices/etc.), the configurable widget system (add/remove/reorder/new types), window splits (`w` leader), Apple Watch companion (present in the design package, not in scope), additional themes.

### Mobile (iOS, `mobile/grm-*.jsx`)
- iOS-26 liquid-glass **tab bar: Daily · Agenda · Inbox · Library** + Search (trailing glass circle).
- **Capture = sheet** (the `tabViewBottomAccessory` bar → sheet), native input chooser.
- **Library** = workspace widget grid (the mobile expression of the rail/widget system).
- Same palette/contrast/Tabler icons/mono metadata as desktop.

## Architecture

### Web (new SvelteKit)
- **New route tree / app surface**, isolated from the old v5 (`/v4`). All-new components built to the Graphite stylesheet (`gr-shell.jsx` → Svelte components + a tokens stylesheet).
- **PORT (keep, don't rewrite)** — the vetted non-UI logic in `web/src/lib/`: `api-client.ts`, `block-parser.ts`, the CodeMirror block-editing engine (the hard-won outliner editing — `BlockOutliner` logic, incl. the recent empty-day/seed/save fixes), `date-parser.ts`, leader-tree, keybindings, sync/WS wiring, query layer. Move these into the new tree (or a shared `lib/`), trimming dead code as encountered (not wholesale refactor).
- **Tokens:** CSS custom properties from a single token source; components reference vars only (theme-swappable).
- **Panes** modeled as a first-class container so the deferred split manager slots in.
- **Cutover:** when daily-driver parity holds, delete the old v5 route tree + components.

### iOS (new SwiftUI)
- **New SwiftUI screen/view layer** to the Mobile design (`app/Tesela-iOS/Sources/Views` + `Components`).
- **REUSE (keep)** the working sync stack: the Loro FFI (`SyncEngineHandle.openLoro`, `SyncCoordinator`), `RelayTicker`, `MosaicService` (sandbox `.md` read + materialize), pairing, the design tokens (Swift mirror).
- Liquid-glass tab bar + capture sheet per the Mobile design; Library = workspace widget grid.

### Shared design tokens (web ↔ iOS)
Single token definition (colors/type/spacing/radii) → web CSS vars + iOS Swift constants. Avoid drift: one canonical file, generated/mirrored to both. This is the seam that keeps the two parallel rebuilds visually identical.

## Build phasing (per platform, foundation-first)

1. **Foundation** — tokens (web CSS vars + iOS Swift theme from one source), Tabler icon set, primitives (buttons, chips, rows, widget shell, type tags/dots).
2. **Shell** — web: topbar + widget-rail host (fixed widgets) + panes container + status line + command palette + leader overlay. iOS: tab bar + header + capture sheet + nav.
3. **Daily-driver views** (priority order): daily journal → page/outliner editing → capture → inbox → agenda → search. Wire to Loro-authoritative API (web) + FFI (iOS).
4. **Parity check** on real daily flows → **cut over** → delete old web v5 + old iOS UI.
5. **Iterate:** graph, tag-table, full settings, configurable widget system, additional themes, window splits, (maybe) watch.

## Parity checklist (cutover gate)

Web + iOS each: daily journal renders + edits (blocks, properties, tags/links, empty-day seed), page/outliner editing, capture, inbox triage, agenda week, ⌘K palette + leader, search, live sync (web↔iOS via the relay — already proven), keyboard-first throughout (web), liquid-glass nav (iOS). Cut over when these beat the old app for daily use.

## Risks + mitigations

- **Re-skin trap (the stated failure mode):** new isolated trees + all-new components; old code only referenced, never edited; deleted wholesale at cutover. Port list is explicit + minimal.
- **Two parallel rebuilds:** shared tokens + shell-first sequencing keep them aligned; the proven sync layer is reused on both (not rebuilt).
- **Reuse-vs-rewrite boundary:** the CodeMirror editing engine + sync are the crown jewels — REUSE, never rewrite. Only the presentational shell/components are new.
- **Token drift web↔iOS:** single canonical token source.
- **Scope creep into deferred surfaces:** the parity checklist is the gate; graph/tag-table/settings/widget-config/splits explicitly out until after cutover.

## Roadmap placement

`Now/Next:` finish Loro cutover (flag-day, ai-business dedup, DR drill). → `Then:` this redesign (foundation → shell → daily-driver → cutover → iterate), web + iOS parallel. → `Later:` window splits, additional themes, graph/tag-table/settings polish, watch.

## Open decisions (resolve at execution)

- Exact new web surface: a fresh `web/src/routes/g/` tree in the existing SvelteKit app vs a separate SvelteKit app. (Lean: fresh route tree + new `lib/` namespace in the same repo app — isolation without a second toolchain.)
- Token format/generation (hand-authored CSS vars + Swift, or a generator).
- Whether any non-Graphite theme ships at parity (lean: no; Graphite-only, tokenized).
