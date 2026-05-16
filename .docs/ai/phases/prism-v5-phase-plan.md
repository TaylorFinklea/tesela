# Prism v5 phase plan

Companion to `.docs/designs/2026-05-15-prism-v5-chrome.md` — the architecture
spec. This file is the ordered cutover plan.

## Context

Strict rip-and-replace cutover. Each phase ends with a phase completion
report on disk (`.docs/ai/phases/prism-v5-phase-N-report.md`). Daily-driver
`/v4` may be broken between phases 2 and 3 (the "unstable window"); main is
the rollback. Phases are sized to one Build session each.

Phase order folds in the parallel design-review's A14 ordering: the kinds and
their renderers are interleaved with the pane-tree code, so there's no clean
"delete v4 first, then build v5" sequence. The pane-tree refactor *is* the
v4 delete.

## Phase 0 — v4 surface inventory

**Goal:** know exactly what's being torn out before tearing.

**Scope:**
- Enumerate every file in `web/src/lib/components/v4/` and
  `web/src/routes/v4/`.
- For each, note: depends on (imports), depended on by (re-exports /
  usages), which v4 pane kinds it touches.
- Catalog the v4 pane state file (`web/src/lib/stores/pane-tree.ts`,
  `pane-tree.svelte.ts`) and document its current shape + persistence
  envelope.
- Catalog leaf-reusable components that will be wrapped as v5 renderers:
  `BacklinksTab`, `OutlineTab`, `PropertiesTab`, `JournalView`,
  `QueryWidgetView`, `GraphCanvas`, `BlockOutliner`.
- Catalog overlays (Station, Peek, FullscreenOverlay) and their wiring.
- Enumerate every `focusPane` call site, every place reading from the v4
  five-kind union.

**Files likely touched:** none (read-only inventory).

**Exit criteria:**
- `.docs/ai/phases/prism-v5-phase-0-inventory.md` exists, listing every v4
  surface with deletion / keep-as-renderer / rewrite classification, plus
  all focus + kind-read call sites.

**Verification:** spot-check inventory against `grep -rn "from.*v4"` and
`grep -rn "Pane.*kind"`.

**Dependencies:** none.

## Phase 1 — v5 types, registries, renderer protocol (additive-only)

**Goal:** all v5 type-level scaffolding compiles. v4 still runs unchanged.

**Scope:**
- New file: `web/src/lib/buffer/types.ts` — `Buffer` discriminated union,
  `Reference` tagged union, `Leaf` / `Split` / `Node` shapes, binding
  modes, `PageId` / `LeafId` / `SplitId` brands.
- New file: `web/src/lib/buffer/protocol.ts` — `DerivedRendererProps`,
  `PageRendererProps`, `Size`, `NavigationIntent`, `RendererCascade`,
  `DerivedRenderer<K>` interface.
- New files: `web/src/lib/renderers/page/index.ts`,
  `web/src/lib/renderers/derived/index.ts`,
  `web/src/lib/renderers/ambient/index.ts` — three separate registries
  with explicit `register(name, mod)` calls, mount guard for the derived
  registry that throws `RendererReferenceMismatch`.
- Unit tests: registry rejects mismatched reference; idempotent
  registration; manifest snapshot.
- **Confirm `@tanstack/svelte-query` v5+ compatibility with runes.** If the
  library lags, add a tiny `createReactiveQuery` adapter wrapping
  `createQuery` to expose `$derived` values; lock that adapter in this
  phase before wiring data anywhere.

**Files likely touched:** new files only; v4 untouched.

**Exit criteria:**
- `pnpm check` clean.
- Unit tests cover registry mount guard + idempotency.
- Adapter (if needed) documented + tested.

**Verification:** `pnpm test:unit` green.

**Dependencies:** Phase 0.

## Phase 2 — Pane tree replacement (start of unstable window)

**Goal:** the leaf shape is v5's `Buffer` union. `focusPane` enforces the
follow rule. Persistence bumped + migrated.

**Scope:**
- Replace leaf shape in `web/src/lib/stores/pane-tree.ts` /
  `pane-tree.svelte.ts` with v5 types.
- `focusPane` chokepoint: updates `lastFocusedLeafPerTab` always; updates
  `lastFocusedPagePerTab` only when the focused buffer is a page.
  `lastFocusedPagePerTab` non-exported.
- One-shot localStorage migration: read old `tesela:prism4:v1`, write new
  `tesela:prism:state` (`_v: 3`), delete old. Migration walks the v4 tree
  and maps leaves per the spec.
- Golden-file tests for migration against captured v4 blobs.

**Files likely touched:** `pane-tree.ts`, `pane-tree.svelte.ts`,
`pane-tree.test.mjs`, new `migration.ts` + tests.

**Exit criteria:**
- All existing pane-tree mutation tests pass against the v5 shape.
- Golden-file migration tests pass.
- `pnpm check` clean.

**Verification:** drop a captured v4 envelope into a fresh browser and
confirm migration produces a sane tree.

**Dependencies:** Phase 1.

## Phase 3 — Page buffer + existing page renderers (end of unstable window)

**Goal:** app is editable again. Page buffers render `note`, `daily`,
`query`, `scratch` types.

**Scope:**
- Rewrite `PaneShell.svelte` (rename to `BufferShell.svelte` if cheap) to
  mount via the page-renderer registry based on the focused page's `type`
  frontmatter.
- Wrap existing components as renderers (thin adapters, not rewrites):
  - `BlockOutliner` → `note` + `scratch`
  - `JournalView` → `daily`
  - `QueryWidgetView` → `query`
  - unknown `type:` → outliner fallback + warning chip
- Each leaf wrapped in `<svelte:boundary>` so a crashing renderer fails
  soft.
- Delete v4's `widget` / `context` / `graph` / `dashboard` branches in
  PaneShell.
- Rewrite `+layout.svelte` to mount only the v5 buffer shell.
- Update `+page.svelte` seed logic to open today's daily.
- **One-time changelog modal** displays on first v5 boot if migration
  dropped or converted any panes.

**Files likely touched:** PaneShell (or BufferShell), `+layout.svelte`,
`+page.svelte`, new modal component.

**Exit criteria:**
- App loads, opens today's daily as a page buffer.
- vsplit / hsplit / focus motions work.
- All v4 chord shortcuts still bind correctly.
- Modal appears once per first v5 boot.

**Verification:** dogfood via Chrome DevTools MCP — open daily, vsplit,
hsplit, ⌘W, ⌘T, tab switching, hjkl focus, Ctrl+W motions.

**Dependencies:** Phase 2.

## Phase 4 — Derived buffer machinery + renderers

**Goal:** derived buffers exist with Follow + Pinned binding. Initial five
renderers work.

**Scope:**
- Implement `DerivedBuffer` mount in the pane tree, with the renderer
  protocol from Phase 1.
- Follow resolution: per-tab `lastFocusedPagePerTab`, read via `$derived`
  inside each leaf component. Renderer receives a resolved `Reference`,
  not the binding mode.
- Register five derived renderers (thin adapters over v4 components):
  - `backlinks-of-page` (wraps `BacklinksTab`)
  - `outline-of-page` (wraps `OutlineTab`)
  - `properties-of-page` (wraps `PropertiesTab`)
  - `local-graph-of-page` (new — small wrapper around `GraphCanvas`)
  - `tasks-linked-to-page` (new — wraps the v4 `LinkedTasksTab` if
    extracted, else new)
- Palette verbs: `:backlinks`, `:outline`, `:properties`, `:tasks`,
  `:graph-local` (optional `pin <page>` suffix).
- Mount guard exercises: each renderer registers with its declared
  `accepts` discriminator; mismatched reference throws and is caught by
  the buffer shell into a soft-fail card.

**Files likely touched:** buffer registry, new
`web/src/lib/renderers/derived/*`, Station verb registry, BufferShell.

**Exit criteria:**
- Open daily + vsplit a backlinks Follow derived buffer; navigate to a
  different page in the page-buffer; backlinks pane updates.
- Pin a derived buffer to a specific page; verify it doesn't update when
  focused page changes.
- Focusing the derived buffer does NOT update follow source (status line
  confirms).

**Verification:** dogfood follow vs pinned; spot-check focus rule via the
status-line binding indicator.

**Dependencies:** Phase 3.

## Phase 5 — Ambient buffers + Command Station launcher

**Goal:** ambient buffers work as workspace-level singletons. Station is
reframed as palette + ambient launcher.

**Scope:**
- Ambient registry + workspace-level state stores. Convention:
  `web/src/lib/ambients/<name>/state.svelte.ts` exports reactive state +
  public API per ambient.
- Register four ambient buffers:
  - `calendar` (basic month view, no event-source plumbing yet — stub OK
    at this phase)
  - `today-in-progress` (lists in-progress tasks across the workspace)
  - `workspace-dashboard` (pinned widgets card grid, recycled from v4
    dashboard kind)
  - `ai-workspace` (placeholder card — "coming in a later phase" + one
    disabled teaser action)
- Wire palette verbs: `:calendar`, `:in-progress`, `:dashboard`, `:ai`.
- Strip widget-related code from Station; reframe as palette + launcher.
- Per-tab mount, workspace state. `unmountAmbient` removes the pane; state
  survives.

**Files likely touched:** new `ambients/*`, ambient registry, Station,
`system-widgets.ts` (audit/simplify).

**Exit criteria:**
- Open calendar ambient in tab A, also in tab B; verify state shared.
- Close calendar in tab A; verify tab B unaffected.

**Verification:** dogfood cross-tab behavior.

**Dependencies:** Phase 4.

## Phase 6 — Left sidebar (switchable surfaces)

**Goal:** dedicated, collapsible left sidebar with icon-strip switching
between five surfaces.

**Scope:**
- New component: `web/src/lib/components/v4/Sidebar.svelte` (rename
  conventions tbd).
- Five surface components: notes tree, search, recent, pinned, tags.
- Icon-strip swaps the visible surface; collapse toggle persists per
  workspace.
- Default fresh-boot state: open, notes tree active.

**Files likely touched:** new sidebar + surfaces, `+layout.svelte`,
workspace state stores.

**Exit criteria:**
- Sidebar visible on fresh boot with notes tree.
- Toggle collapse via `◄` button or chord; state persists across reload.
- Switching surfaces works via icon click + keyboard chord.

**Verification:** dogfood the surface swap, collapse, persistence.

**Dependencies:** Phase 3.

## Phase 7 — Shared workspace state (pinned · recent · search)

**Goal:** single-source workspace state read by both sidebar and Station
palette.

**Scope:**
- TanStack Query layer for pinned + recent (disk-persisted) and search
  (in-memory, query-string-keyed).
- Tiny consumer wrappers: `usePinned()`, `useRecent()`, `useSearch(q)`.
- Pinning a page from sidebar mutates the on-disk pinned file and
  invalidates `["pinned"]`. Same code path called from Station.
- Recent LRU updated on every `focusPane(page)` transition.

**Files likely touched:** new `web/src/lib/state/pinned.ts`, `recent.ts`,
`search.ts`; sidebar surface components; Station palette dataset.

**Exit criteria:**
- Pin a page from the sidebar tree; it appears in Station's pinned picker.
- Open a page from Station; it appears in sidebar's recent surface.
- Search backend wired (even if backed by a stub corpus).

**Verification:** dogfood pinning parity, recent LRU.

**Dependencies:** Phase 6.

## Phase 8 — Status line with binding indicator

**Goal:** status line accurately shows mode, kind, name, modified,
position, workspace, and binding indicators.

**Scope:**
- New component: `web/src/lib/components/v4/StatusLine.svelte`.
- Binding indicator: `$derived.by` walks the focused tab's pane tree +
  focus state, returns a tagged `BindingIndicator`.
- Indicator chips: "↪ N followers" on a page with derived followers;
  "📌 backlinks · X" on a pinned derived; ambient name on ambient buffers.

**Files likely touched:** new StatusLine, layout wiring.

**Exit criteria:**
- Status line reflects focused buffer kind/name accurately.
- "↪" indicator appears on a page when a sibling follow-derived buffer
  exists.

**Verification:** dogfood with two-pane setups exercising each indicator
state.

**Dependencies:** Phase 4.

## Phase 9 — Peek (host-agnostic renderer hosting)

**Goal:** ⌘I Peek hosts derived renderers identically to derived buffer
panes. Chord-cycle through renderers.

**Scope:**
- Rewrite `PeekPopover.svelte` to host one derived renderer at a time.
- `Tab` cycles forward, `Shift-Tab` back, `Esc` dismisses, `Enter` jumps.
- Peek's `onNavigate` closes the popover before navigating; the pane's
  `onNavigate` just navigates.
- Per-page-type first-shown memory map.
- Hide-list for per-workspace renderer exclusion from Peek cycle.
- Audit: confirm derived renderers don't read host-specific stores;
  they're truly host-agnostic.

**Files likely touched:** `PeekPopover.svelte`, renderer audits.

**Exit criteria:**
- ⌘I opens Peek with backlinks of focused page.
- Tab cycles renderer; per-page-type first-shown remembered.
- Same renderer code proven to mount inside a derived buffer pane and
  inside Peek with no host knowledge.

**Verification:** dogfood; spot-check the renderer module for host
references.

**Dependencies:** Phase 4.

## Phase 10 — Cascade wiring for renderer min-sizing

**Goal:** renderers gracefully degrade in small panes. Hosts pick cascade
members based on size.

**Scope:**
- Each existing renderer module exports a `RendererCascade<P>`. Default is
  the full version; modes list narrower versions with `minSize`.
- Pane tree passes `(cols, rows)` to BufferShell; shell picks the right
  cascade member.
- Cascade members for:
  - `query` (table → compact list)
  - `daily` (multi-day → today-only)
  - `local-graph-of-page` (graph → node-count chip)
- Renderer module receives `size` for in-mode layout (margins, columns
  shown) but doesn't choose modes.

**Files likely touched:** renderer modules, BufferShell.

**Exit criteria:**
- Resize a query-buffer pane below threshold; renderer swaps to compact
  list (single instantiation, not re-render storm).

**Verification:** dogfood resize; quick code-check for self-decided mode
swapping.

**Dependencies:** Phase 3.

## Phase 11 — `type: scratch` + promote verb

**Goal:** scratch as a page type works end-to-end.

**Scope:**
- Backend: ensure note creation can take `type: scratch` frontmatter.
- Page-type renderer for `scratch` registers (same as `note` plus a
  "scratch" chip in the buffer header).
- Palette verb `:scratch` (creates and opens `scratch/<timestamp>.md`).
- Leader chord `Space n s` (which-key tree: `Space n` = new menu).
- Palette verb `:promote` (focused scratch → real page; prompts title +
  location, default seed `notes/` root, moves file, removes
  `type: scratch` frontmatter).
- Filter scratches out of default surfaces: sidebar tree, search, recent,
  default queries. Toggle: "show scratches" in the tree surface +
  explicit `type:scratch` queries.

**Files likely touched:** API client, page-type registry, sidebar tree,
search, palette verbs.

**Exit criteria:**
- `:scratch` creates a scratch and opens it; scratch absent from default
  surfaces.
- `Space n s` does the same.
- `:promote` round-trips a scratch to a regular page; file is renamed on
  disk.

**Verification:** dogfood scratch → promote.

**Dependencies:** Phase 3.

## Phase 12 — Prune sweep

**Goal:** optional prune of stale scratch pages.

**Scope:**
- Setting: `scratchPruneAfterDays` (default OFF when unset).
- Background sweep on app startup (and once daily thereafter): lists
  scratches with no edits past threshold; deletes. Quiet — no toast per
  prune.

**Files likely touched:** settings, API client, small sweep module.

**Exit criteria:**
- Settings UI exposes the toggle.
- With value set + a stale scratch on disk, the sweep deletes it.

**Verification:** unit test the sweep predicate; manual end-to-end.

**Dependencies:** Phase 11.

## Phase 13 — Fullscreen graph + v4 surface deletion + cleanup

**Goal:** delete every v4 surface the inventory marked as "delete";
validate no dead code; `⌘G` still opens fullscreen graph.

**Scope:**
- Audit + delete v4 components no longer referenced (Phase 0 inventory
  drives the list).
- Confirm `⌘G` fullscreen graph still works (only the *pane-kind* graph
  was removed; the overlay survives).
- Run `pnpm check`; fix drift.
- Run full unit + perf test suites.
- Update `AGENTS.md` / memory entries that reference v4 buffer kinds.

**Files likely touched:** lots (deletions); minor wiring fixes.

**Exit criteria:**
- `pnpm check` clean.
- All tests green.
- No imports from deleted modules.
- `⌘G` overlay still opens.

**Verification:** full smoke test through Chrome DevTools MCP; CI green.

**Dependencies:** all prior phases.

## Open product questions (not blocking)

- Promote-scratch default location prompt seed — defaulting to `notes/`
  root (matches existing convention); revisit if surprising.
- Prune sweep — defaulted to OFF per v5 spec; no further question.
