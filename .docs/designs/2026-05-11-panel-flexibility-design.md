# Panel flexibility — design

**Date**: 2026-05-11
**Scope**: Web client. Three related layout features.

## Goals

1. **Collapsible rail** — the left sidebar can be hidden/shown the same way the bottom drawer already can.
2. **Drawer side** — the drawer (today always at the bottom) can be docked to the right edge instead, user's choice, persisted.
3. **Pinned tabs** — the user can pin a block or a page to the drawer as a tab. Pinned tabs persist globally across pages.

## Non-goals

- No new tabs in the drawer beyond the existing five fixed tabs and the user's pinned tabs (no quick-search tab, no inbox tab, etc.).
- No per-page pinning. All pins are global.
- No drag-to-reorder pinned tabs in v1.
- No overflow menu / tab-pinning quick-jump palette in v1; horizontal scrolling on the tab strip is the v1 answer to many pins.

## Existing baseline (what changes)

Files most relevant to this work:

- `web/src/lib/stores/pane-state.svelte.ts` — drawer-open and tab-selection state, localStorage keys.
- `web/src/routes/+layout.svelte` — top-level grid container, leader-menu entries, global keybindings.
- `web/src/app.css` — `.v9` grid template definitions.
- `web/src/lib/components/Rail.svelte` — left sidebar.
- `web/src/lib/components/BottomDrawer.svelte` — drawer body and tab content dispatch.
- `web/src/lib/components/TabStrip.svelte` — tab strip rendering.
- `web/src/lib/components/SplitDivider.svelte` — resizable splitter between drawer and main.

Existing patterns we mirror:

- `bottomDrawerOpen` is a `$state` boolean persisted under `tesela:bottomDrawerOpen`; toggled via `toggleBottomDrawer()`; exposed to layout via `isBottomDrawerOpen()`.
- Leader-menu entries live in `+layout.svelte`'s `leaderTree`; each entry has `{ key, label, action, hint }`.
- The active-region union (`"rail" | "middle" | "focus" | "bottom"`) drives `Ctrl+W h/l/j/k` navigation. `setBottomDrawerOpen(false)` already coerces the active region away from `"bottom"` when the drawer is hidden — we follow the same pattern for the rail.

---

## Section 1 — Collapsible rail

### State

```ts
// pane-state.svelte.ts
const RAIL_OPEN_KEY = "tesela:railOpen";

function loadRailOpen(): boolean {
  if (!browser) return true;
  try {
    const v = localStorage.getItem(RAIL_OPEN_KEY);
    return v === null ? true : v === "true";
  } catch { return true; }
}

let railOpen = $state(loadRailOpen());

export function isRailOpen(): boolean { return railOpen; }
export function setRailOpen(v: boolean): void {
  railOpen = v;
  try { localStorage.setItem(RAIL_OPEN_KEY, String(v)); } catch {}
  if (!v && activeRegion === "rail") activeRegion = "focus";
}
export function toggleRail(): void { setRailOpen(!railOpen); }
```

### Layout

`app.css` `.v9` block changes to make the rail column width a custom property:

```css
.v9 {
  /* defaults */
  --v9-rail-w: 232px;
  grid-template-columns: var(--v9-rail-w) 1fr;
  /* rest unchanged */
}
.v9.rail-collapsed { --v9-rail-w: 0px; }
```

`+layout.svelte` adds the class:

```svelte
<div class="v9 dark {drawerOpen ? 'with-bottom' : ''} {railOpen ? '' : 'rail-collapsed'} drawer-{drawerSide}">
```

The `Rail.svelte` element stays in the DOM when collapsed (preserves its internal state — selection, expanded widget groups, etc.). The grid column simply has zero width; `overflow: hidden` on `.v9-rail` (already present) clips it. CSS transition on `--v9-rail-w` (200ms ease) gives a smooth open/close.

### Triggers

- **Global hotkey `r`** — mirrors the existing `b`/`1` handler at `+layout.svelte:227`. Same `isEditing` gate; calls `toggleRail()`. Add a matching entry in the leader menu's `w` (Window) subtree for discoverability: `{ key: "r", label: "Toggle rail", action: toggleRail, hint: "r" }`.
- **Click handle** — a single chevron button. When the rail is open, the handle sits at the top-right corner of the rail (anchored just inside the right edge), showing `‹`. When collapsed, a 12-px-wide reveal strip sits flush against the viewport's left edge in the focus column, showing `›`. Clicking either toggles. Both are part of the rail component; the reveal strip is rendered when `!isRailOpen()` via `{#if}`.

### Active-region behavior

`Ctrl+W h` to focus the rail is a no-op when the rail is collapsed (the handler in `+layout.svelte:116` checks `isRailOpen()` and falls through to `focus`). `setRailOpen(false)` clears `activeRegion` away from `"rail"` (mirrors the existing `setBottomDrawerOpen` pattern).

---

## Section 2 — Drawer side toggle (bottom ↔ right)

### State

```ts
// pane-state.svelte.ts
export type DrawerSide = "bottom" | "right";

const DRAWER_SIDE_KEY = "tesela:drawerSide";
const DRAWER_HEIGHT_KEY = "tesela:drawerHeight";  // existing default 220
const DRAWER_WIDTH_KEY = "tesela:drawerWidth";    // new, default 360

function loadDrawerSide(): DrawerSide {
  if (!browser) return "bottom";
  try {
    const v = localStorage.getItem(DRAWER_SIDE_KEY);
    return v === "right" ? "right" : "bottom";
  } catch { return "bottom"; }
}

let drawerSide = $state<DrawerSide>(loadDrawerSide());
let drawerHeight = $state(loadNumber(DRAWER_HEIGHT_KEY, 220, 120, 600));
let drawerWidth = $state(loadNumber(DRAWER_WIDTH_KEY, 360, 240, 800));

export function getDrawerSide(): DrawerSide { return drawerSide; }
export function setDrawerSide(side: DrawerSide): void {
  drawerSide = side;
  try { localStorage.setItem(DRAWER_SIDE_KEY, side); } catch {}
}
export function toggleDrawerSide(): void {
  setDrawerSide(drawerSide === "bottom" ? "right" : "bottom");
}

export function getDrawerHeight(): number { return drawerHeight; }
export function setDrawerHeight(n: number): void { /* clamp + save */ }
export function getDrawerWidth(): number { return drawerWidth; }
export function setDrawerWidth(n: number): void { /* clamp + save */ }
```

### Layout

`app.css` — two new variants of `.v9.with-bottom`:

```css
.v9.with-bottom.drawer-bottom {
  grid-template-rows: 32px 1fr var(--v9-drawer-h, 220px) 24px;
  grid-template-areas:
    "crumb crumb"
    "rail focus"
    "bottom bottom"
    "status status";
}
.v9.with-bottom.drawer-right {
  grid-template-columns: var(--v9-rail-w, 232px) 1fr var(--v9-drawer-w, 360px);
  grid-template-rows: 32px 1fr 24px;
  grid-template-areas:
    "crumb crumb crumb"
    "rail focus bottom"
    "status status status";
}
```

`+layout.svelte` adds the side class:

```svelte
<div class="v9 dark {drawerOpen ? 'with-bottom' : ''} {railOpen ? '' : 'rail-collapsed'} drawer-{drawerSide}">
```

And reactively wires the width/height variables on `.v9` via `style:--v9-drawer-h={drawerHeight + 'px'}` and `style:--v9-drawer-w={drawerWidth + 'px'}`.

### Triggers

- **Icon button** in `BottomDrawer.svelte` header (top-right, before the existing close `×`). Uses `IconLayoutSidebarRightCollapse` (when side === bottom) or `IconLayoutBottombarCollapse` (when side === right) from `@tabler/icons-svelte`. `onclick={toggleDrawerSide}`.
- **Leader chord `Space w P`** — under the existing `w` (Window) subtree at `+layout.svelte:113-119`, adjacent to `h/l/j/k/q`:
  ```ts
  { key: "P", label: "Panel position",   action: toggleDrawerSide, hint: "p" }
  ```
  Uppercase `P` distinguishes it from the lowercase keys already mapped in that subtree. The leader menu is case-sensitive at the chord level.

### SplitDivider

`SplitDivider.svelte` today is horizontal (resizes drawer height). Add an `orientation: "horizontal" | "vertical"` prop. When `drawerSide === "right"`, render the divider on the drawer's left edge (vertical), bound to `drawerWidth`. When `drawerSide === "bottom"`, keep current behavior (horizontal divider on the drawer's top edge, bound to `drawerHeight`).

The divider lives inside `BottomDrawer.svelte` and is conditional on `drawerSide`.

### Ctrl+W navigation

The active-region union stays `"rail" | "middle" | "focus" | "bottom"`. `"bottom"` semantically becomes "drawer" regardless of side. The `Ctrl+W` handlers in `+layout.svelte`:

- `j` (down) → focus drawer iff `drawerSide === "bottom"` AND drawer is open; otherwise no-op or fall through to focus.
- `l` (right) → focus drawer iff `drawerSide === "right"` AND drawer is open; otherwise the existing behavior (focus the rightmost addressable region — today middle/focus).

So the `j`/`l` keys' meaning of "drawer" follows the drawer's side. No new `Region` value, no widespread rename.

---

## Section 3 — Pinned tabs

### Data model

```ts
// pane-state.svelte.ts
export type PinnedTab =
  | { id: string; kind: "page"; noteId: string; title: string }
  | { id: string; kind: "block"; noteId: string; blockId: string; preview: string };

const PINNED_TABS_KEY = "tesela:pinnedTabs";

let pinnedTabs = $state<PinnedTab[]>(loadPinnedTabs());

export function getPinnedTabs(): PinnedTab[] { return pinnedTabs; }
export function pinPage(noteId: string, title: string): string { /* push, save, return id */ }
export function pinBlock(noteId: string, blockId: string, preview: string): string { /* push, save, return id */ }
export function unpinTab(id: string): void { /* filter, save */ }
```

- `id` is a `crypto.randomUUID()`, stable for the lifetime of the pin (lets active-tab tracking survive renames; lets the same target be pinned twice).
- `title` / `preview` are cached display labels (truncated to ~40 chars). Re-fetched lazily on render; if fetch shows the note still exists, the cached field is updated; if missing, the placeholder is shown.

### Tab union — migration

`BottomTab` changes from a flat string union to a discriminated union:

```ts
export type FixedTabId = "backlinks" | "properties" | "outline" | "history" | "linkedTasks";
export type BottomTab =
  | { kind: "fixed"; id: FixedTabId }
  | { kind: "pinned"; id: string };  // id = PinnedTab.id
```

`tesela:bottomDrawerTab` storage shape migrates:
- Read: if value is a string and matches a `FixedTabId`, wrap as `{ kind: "fixed", id }`. If it's JSON for the new shape, parse. Else default to `{ kind: "fixed", id: "backlinks" }`.
- Write: JSON.stringify the new shape.

### Tab strip rendering

`BottomDrawer.svelte` (or the `TabStrip.svelte` it uses):
1. Render the five fixed tabs in their existing order.
2. After a visual separator (a thin `1px` divider with `var(--v9-line)`), render pinned tabs in order.
3. Each pinned tab shows its label and an `×` button visible on hover/focus that calls `unpinTab(tab.id)`.
4. Tab strip becomes horizontally scrollable when contents exceed the strip width: `overflow-x: auto; flex-wrap: nowrap;` — keyboard `Tab`/`Shift+Tab` from inside the strip cycles tabs and scrolls the active one into view.

### Tab content dispatch

`BottomDrawer.svelte`'s content area renders based on `tab.kind`:
- `kind === "fixed"` — existing per-id switch (backlinks → component, etc.).
- `kind === "pinned"` — look up the pin by `id`, then:
  - `pin.kind === "page"` → `<BlockOutliner noteId={pin.noteId} body={…} … />`. The note is fetched via `createQuery` keyed on `["note", pin.noteId]` (already deduplicated against the main page's query thanks to Tanstack Query).
  - `pin.kind === "block"` → fetch the note, then render the BlockOutliner with `drillBlockId={pin.blockId}` (the existing prop that already filters `visibleBlocks` to a block subtree for drill-in views; we reuse it as-is).
  - If the note fetch returns 404, render a `Note no longer exists — [Unpin]` placeholder. The placeholder calls `unpinTab(tab.id)` on click.

Pinned-tab BlockOutliners pass `setLastActiveOutliner` calls but receive a flag `isPinnedTab={true}` that suppresses the singleton registration — the focus-area outliner remains the only "last active" for global handlers like `tesela:restore-focus`.

### Pin actions

- **Leader chord** — new top-level `Space P` (uppercase, distinct from the existing lowercase `p` "Page" subtree):
  ```ts
  // appended to leaderTree in +layout.svelte
  {
    key: "P",
    label: "Pin",
    children: [
      { key: "b", label: "Pin block",  action: pinFocusedBlock, hint: "b" },
      { key: "p", label: "Pin page",   action: pinCurrentPage,  hint: "p" },
    ],
  }
  ```
  - `pinFocusedBlock()` reads `getFocusedBlock()`. If null, toast `No block focused`. Else `pinBlock(block.note_id, block.id, block.raw_text.slice(0, 40))`, then `setBottomDrawerOpen(true)` and `setBottomTab({ kind: "pinned", id })`.
  - `pinCurrentPage()` reads the current `noteId` from `page.url`. If empty (e.g., on `/`), toast `No page to pin`. Else `pinPage(noteId, note.title)`, then open drawer and switch tab.

- **Context menu** — there is no existing context-menu primitive in the codebase, so this work adds a small `ContextMenu.svelte` component: a positioned popover triggered on right-click, with a list of `{ label, action }` items. For v1 the only item on the block-bullet menu is "Pin to drawer", which calls `pinBlock(block.note_id, block.id, block.raw_text.slice(0, 40))`. The component lives at `web/src/lib/components/ContextMenu.svelte` and can be reused for future context menus (yank, delete, copy-link). Page-level context-menu pinning is deferred to v2 (no current right-click target on page headers).

### Lifecycle

- **Unpin** — `×` on tab. If the unpinned tab was active, fall back to the previous tab in the strip's flat order (fixed-then-pinned).
- **Deleted note / missing block** — placeholder content as described above. No auto-unpin (preserves the pin across rename-style flows).
- **Renamed note** — the `title` / `preview` field is refreshed each render by reading from the freshly fetched note, so renames flow through automatically.

---

## Storage keys summary

New keys:
- `tesela:railOpen` — `"true"` / `"false"`.
- `tesela:drawerSide` — `"bottom"` / `"right"`.
- `tesela:drawerWidth` — number (clamped 240–800).
- `tesela:pinnedTabs` — JSON array of `PinnedTab`.

Modified key:
- `tesela:bottomDrawerTab` — was raw string of `FixedTabId`; becomes JSON `BottomTab`. Migration: if the existing value is a plain string matching a known fixed id, wrap as `{ kind: "fixed", id }` on first read.

Existing keys, unchanged: `tesela:bottomDrawerOpen`, `tesela:splitRatio`, `tesela:vSplitRatio:v2`, `tesela:vimEnabled`, `tesela:drawerHeight` (if present).

## Testing

This work is end-to-end UI behavior in a Svelte 5 app with Vim-style keybindings; the existing test surface is mostly Playwright smoke + manual QA. Coverage plan:

- **Manual / browser QA** (primary): exercise each trigger combination — leader chord, click handle / icon button, context menu — with the drawer open + closed and rail open + collapsed. Verify localStorage persistence across reload.
- **Playwright smoke** (`web/tests/perf/perf-smoke.spec.ts` and siblings): add an assertion that the rail's `aria-label` reaches a defined state after `toggleRail()` and that the `.v9` element has the expected class set, to catch grid-class regressions.
- **Unit-ish**: `pane-state.svelte.ts` pure functions (load/save/migrate) get a small Vitest spec covering the `BottomTab` migration: pre-migration string → post-migration object, unknown string → default.

## Risks / open questions

- **Pinned-tab content height** — a page-pinned tab in a 360-px-wide right-docked drawer will be very narrow; line-wrapping in `BlockOutliner` already handles narrow widths, but bullet indentation may compress harshly. We accept the v1 trade-off; the user can resize the drawer via the divider.
- **Same-note fetch race** — when a pinned page is the same note as the focus area's note, both Tanstack Query subscribers share the same key, so they're deduplicated. No special handling needed.
- **Vim focus inside pinned-tab BlockOutliner** — each pinned tab's outliner is its own `BlockEditor` instance with its own vim mode. `Ctrl+W j`/`l` lands in the drawer region; `Tab` cycles tabs; within a tab, j/k navigation works inside that BlockOutliner. We do not attempt cross-tab j/k.
- **Right-docked drawer + collapsed rail** — both states coexist fine in the CSS grid; rail is column 1 with width 0, drawer is column 3, focus is column 2 (1fr).
