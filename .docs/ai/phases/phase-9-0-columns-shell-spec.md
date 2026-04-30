# Phase 9.0 — Columns Shell + Tokyo Night

## Product Overview

Replace the current 3-region layout with the v9 four-region columns shell (rail / middle / focus / bottom drawer) under a single Tokyo Night theme, so Tesela visually matches the approved v9 design while keeping every existing feature (vim mode, outliner, ⌘K, slash, leader, drill-in, splits) functionally identical. Vision lock: `/Users/tfinklea/git/tesela/.docs/ai/phases/v9-redesign-vision.md`.

## Current State

- `web/src/routes/+layout.svelte:202-216` — flex shell rendering Sidebar + main + StatusBar.
- `web/src/routes/+layout.svelte:62-65` — `applyTheme(localStorage.getItem("tesela:mode") ?? "day")` at mount.
- `web/src/routes/+layout.svelte:96-112` — `1 / [ / ] / /` global shortcuts; `sidebarCollapsed` state.
- `web/src/routes/+layout.svelte:114-194` — Ctrl+w chord handler (`h/l/j/k/s/q/=/+/-`); `j/k` only meaningful inside main when split open.
- `web/src/lib/themes.ts:1-138` — six themes + `applyTheme`, persists `tesela:mode`.
- `web/src/app.html:8` — Google Fonts: Newsreader + Source Sans 3.
- `web/src/app.html:11-36` — pre-hydration script that sets day/evening CSS variables.
- `web/src/app.css:1-52` — Tailwind `@theme inline` mapping `--color-*` to legacy theme variables.
- `web/src/lib/stores/pane-state.svelte.ts:15-16` — `Region = "left" | "main" | "right"`; `MainPane = "outliner" | "kanban"`.
- `web/src/lib/stores/pane-state.svelte.ts:43-103` — split state, `setActiveRegion` (releases editor focus, fires `tesela:restore-focus` returning to main).
- `web/src/lib/components/Sidebar.svelte:1-167` — left rail, ~220px, j/k keynav, hardcoded nav (Today/Timeline/Graph/Pages/Properties), Favorites, Recent.
- `web/src/lib/components/RightSidebar.svelte:1-582` — page+block properties, backlinks, forward links; pg/blk segmented control; j/k row keynav.
- `web/src/lib/components/StatusBar.svelte:1-55` — vim mode, ctrl+w pending, split state, save status, ws connection.
- `web/src/lib/components/CommandPalette.svelte:9, 79-83` — imports `applyTheme`; binds `toggleTheme` dep.
- `web/src/lib/commands.ts:25-26, 60-66, 107` — `toggleTheme` dep + `toggle-theme` command + `theme` keyword on `go-settings`.
- `web/src/routes/settings/+page.svelte:3, 44-58, 122-145` — theme picker section + shortcut reference list.
- `web/src/routes/p/[id]/+page.svelte:11-25, 142-169, 174-175, 254-430` — imports `RightSidebar`/`KanbanBoard`/`SplitDivider`; auto-opens split on tag/kanban; renders RightSidebar; tracks `focusedBlock`.
- `.docs/designs/v9-columns/columns/v9-styles.css` — token + grid + region styles to lift wholesale (`.v9`, `.v9-crumb`, `.v9-rail`, `.v9-cal`, `.v9-middle`, `.v9-focus`, `.v9-row`, `.v9-bl`, `.v9-status`, `.v9-bottom`).
- `.docs/designs/v9-columns/columns/v9-app.jsx:262-269, 305-373` — bottom-drawer tab list (Backlinks · Properties · History · Outline · Linked tasks) and shell composition. Crumb hint markup at `:336`.

## Implementation Plan

### Step 1 — Theme system removal

1. Delete `web/src/lib/themes.ts`.
2. `+layout.svelte`: drop `applyTheme` import, the localStorage/applyTheme block, and the `dark` class on the root div.
3. `commands.ts`: remove `toggleTheme` from `buildCommands` deps; delete the `toggle-theme` entry; strip `theme` from `go-settings` keywords.
4. `CommandPalette.svelte`: remove `applyTheme`/`getTheme` import and the `toggleTheme` dep passed to `buildCommands`.
5. `settings/+page.svelte`: delete the `<!-- Theme -->` section (`:44-58`); remove `themes / applyTheme / ThemeId` import and `themeId` state.
6. `app.html`: replace the Google Fonts `<link>` href with Inter Tight + JetBrains Mono. Replace the inline FOUC script with a one-liner that adds `data-theme="tokyo-night"` and the `dark` class — variables now live in `app.css`.

### Step 2 — Tokyo Night tokens in `app.css`

1. Wholesale rewrite. Lift the v9 tokens from `v9-styles.css:2-23` into `:root`. Also redefine the legacy tokens that components consume so reskin cascades automatically:

   | Legacy | Tokyo Night |
   |---|---|
   | `--background` | `var(--v9-bg)` |
   | `--foreground` | `var(--v9-ink)` |
   | `--surface` | `var(--v9-bg-2)` |
   | `--surface-2` | `var(--v9-bg-3)` |
   | `--muted` | `var(--v9-bg-3)` |
   | `--muted-foreground` | `var(--v9-ink-3)` |
   | `--accent` | `var(--v9-bg-3)` |
   | `--accent-foreground` | `var(--v9-ink)` |
   | `--primary` | `var(--v9-amber)` |
   | `--primary-foreground` | `var(--v9-bg)` |
   | `--destructive` | `var(--v9-rose)` |
   | `--border` | `var(--v9-line)` |
   | `--ring` | `var(--v9-amber)` |
   | `--popover` | `var(--v9-bg-3)` |
   | `--popover-foreground` | `var(--v9-ink)` |
   | `--block-bg` | `var(--v9-bg-2)` |
   | `--block-border` | `var(--v9-line)` |
   | `--block-radius` | `4px` |
   | `--block-shadow` | `none` |
   | `--focus-glow` | `0 0 0 2px rgba(255,158,100,0.18)` |
   | `--thread-border` | `var(--v9-line-soft)` |

2. In `@theme inline`, set `--font-display` and `--font-sans` to `"Inter Tight", system-ui, sans-serif`; `--font-mono` to `"JetBrains Mono", ui-monospace, monospace`. Drop Newsreader / Source Sans 3 / Fira Code.
3. Body: `font: 13px/1.5 var(--font-sans)`. Drop the `h1,h2,h3 { font-family: Newsreader }` rule.
4. Append `v9-styles.css:44-end` verbatim — supplies `.v9-crumb`, `.v9-rail`, `.v9-cal`, `.v9-middle`, `.v9-focus`, `.v9-row`, `.v9-bl`, `.v9-status`, `.v9-bottom` styles. The `.v9` class names won't collide with Tailwind/component classes.

### Step 3 — Generalize `pane-state.svelte.ts`

1. Widen `Region` to `"rail" | "middle" | "focus" | "bottom"`. Default `activeRegion = "focus"`.
2. Add `bottomDrawerOpen` (default `true`, persisted under `tesela:bottomDrawerOpen`) with `isBottomDrawerOpen() / toggleBottomDrawer() / setBottomDrawerOpen()`. Toggling closed while region is `"bottom"` resets region to `"focus"`.
3. Add `bottomTab: "backlinks" | "properties" | "outline" | "history" | "linkedTasks"` (persisted under `tesela:bottomDrawerTab`, default `"backlinks"`) with `getBottomTab() / setBottomTab()`.
4. In `setActiveRegion`, run `releaseEditorFocus()` for everything except `"focus"`, fire `tesela:restore-focus` when transitioning to `"focus"` (current behavior, renamed).
5. Keep `MainPane / openSplit / closeSplit / toggleSplit / getActivePane / setActivePane / get/setSplitRatio / adjustSplitRatio` exports — kanban split is preserved inside the focus region.

### Step 4 — Four-region grid in `+layout.svelte`

1. Replace the wrapper at `+layout.svelte:202-211` with `<div class="v9 {bottomDrawerOpen ? 'with-bottom' : ''}">` containing `<CrumbBar />`, `<Rail />`, `<MiddleColumn />`, `<main class="v9-focus">{@render children()}</main>`, `{#if bottomDrawerOpen}<BottomDrawer />{/if}`, `<StatusBar />`.
2. Drop the `<Sidebar />` instance and `sidebarCollapsed` state; collapse/uncollapse is not a 9.0 feature.
3. Rewrite the Ctrl+w `h/j/k/l` cases. h: focus→middle→rail; bottom→focus. l: rail→middle→focus. j: focus→bottom (only when `isBottomDrawerOpen()`). k: bottom→focus. Keep `s/q/=/+/-` branches untouched.
4. In `panelHandler`, add `if (e.key === "b") { e.preventDefault(); toggleBottomDrawer(); }` (reuse existing `isEditing` guard).
5. Rebind `1` from sidebar-collapse to `toggleBottomDrawer()` so existing muscle memory lands somewhere; update the settings shortcut list.

### Step 5 — `CrumbBar.svelte` (new)

`<div class="v9-crumb">` with: `Tesela ›` static segment; a section segment derived from `page.url.pathname` (`/`→Pages, `/daily`→Today, `/timeline`→Timeline, `/graph`→Graph, `/properties`→Properties, `/settings`→Settings, `/p/...`→Pages › note title via `createQuery(["note", id])`); final segment uses `seg curr`; `<span class="sp"></span>`; static hints span (verbatim from `v9-app.jsx:336`).

### Step 6 — `Rail.svelte` (new, replaces `Sidebar.svelte`)

`.v9-rail` + `.v9-rail-scroll`. Three groups using `data-icon` glyphs from `v9-styles.css:183-191`:

- **Pinned**: Today (`calendar`), Pages (`cal`).
- **Browse**: Timeline (`clock`), Graph (`query`), Properties (`project`).
- **Saved**: Favorites (`pin`) + Recent (`clock`) from `getFavorites()` / `getRecents()`.

Active row when href matches `page.url.pathname` (port `Sidebar.svelte:46-52` match logic). j/k keynav gated on `getActiveRegion() === "rail"` (port `Sidebar.svelte:60-66`). Settings link at the bottom (port `:156-162`). No mini calendar, no inline preview lists — both deferred.

### Step 7 — `MiddleColumn.svelte` (new, throwaway per vision)

`.v9-middle` with `.v9-pane-head` (title + subtitle) and `.v9-pane-body`. Branch on path:

- `/` → list notes via `notesQuery`, render as `.v9-row`.
- `/daily` → today's daily note's top-level blocks via `parseBlocks`.
- `/p/[id]` → backlinks list (lift `backlinksQuery + incomingFromEdges` from `RightSidebar.svelte:167-195`).
- `/timeline | /graph | /properties | /settings` → muted "No list view in 9.0" placeholder.

j/k navigates rows when region is `"middle"`. Enter `goto`s. Selection is local; middle does not drive focus content beyond URL navigation.

### Step 8 — `BottomDrawer.svelte` (new, replaces `RightSidebar.svelte`)

`.v9-bottom` with `.tabs` strip and `.body` per `v9-app.jsx:262-303`. Five tabs in order: Backlinks · Properties · Outline · History · Linked tasks. Tab counts in `.n` spans.

- **Backlinks**: lift query+merge logic from `RightSidebar.svelte:167-195`; render `.v9-bl-card` per `v9-app.jsx:280-285`. j/k navigates rows.
- **Properties**: lift the page-vs-block split, custom-properties, tag pills, and edit handlers from `RightSidebar.svelte:438-540`; restyle into `.pchip` chips. The pg/blk segmented control survives inside the tab body header. When `focusedBlock` is null, force `pg` and disable `blk`.
- **Outline**: render `parseBlocks(noteBody)` rows with `padding-left: indent*14px` (per `v9-app.jsx:294`). Click → `goto(?block=...)`.
- **History / Linked tasks**: muted "Coming in 9.x" placeholder.

`Tab` cycles forward, `Shift+Tab` back. Esc routes to `setActiveRegion("focus")`.

The drawer reads `noteId` from `page.url.pathname` and the focused block from a new module-level store `web/src/lib/stores/current-block.svelte.ts` (`getFocusedBlock() / setFocusedBlock()`). Update `routes/p/[id]/+page.svelte:354` to call `setFocusedBlock(b)` inside `onfocusedblockchange`.

### Step 9 — Note-page integration (`routes/p/[id]/+page.svelte`)

1. Remove the `<RightSidebar ... />` block (`:424-429`) and the surrounding `flex` wrapper at `:254`.
2. Remove `rightSidebarCollapsed` (`:174`).
3. Replace local `focusedBlock` state with the new store; preserve any `$derived(getFocusedBlock())` consumers.
4. Leave the kanban split (`:404-421`) untouched — it nests inside the focus region.
5. The new `<main class="v9-focus">` in `+layout.svelte` must not carry the legacy `flex flex-col min-w-0` Tailwind classes (the grid sizes for it).

### Step 10 — `StatusBar.svelte` reskin

Replace wrapper class with `class="v9-status"`. Wrap vim-mode label in `<span class="mode">`. Move save status / ws indicator into the right-side `.toggle / .keys` containers per `v9-styles.css:363-366`. Add a `[×] bottom panel` toggle bound to `toggleBottomDrawer()` (markup per `v9-app.jsx:363`). Keep the OUTLINER/KANBAN split indicator — it still applies inside focus.

### Step 11 — Settings shortcut list

In `settings/+page.svelte:122-145`: replace "Toggle sidebar" with "Toggle bottom drawer (b / 1)". Add `^w h/j/k/l → focus rail/bottom/focus/right`. Keep kanban-rail entries (still reachable on tag pages with kanban view).

### Step 12 — Component reskin pass

For `BlockEditor.svelte`, `BlockOutliner.svelte`, `CommandPalette.svelte`, `LeaderMenu.svelte`, `AutocompleteMenu.svelte`, `SlashMenu.svelte`, `DatePicker.svelte`, `KanbanBoard.svelte`, `KanbanCard.svelte`, `KanbanColumnPicker.svelte`, `TabStrip.svelte`, `ViewSwitcher.svelte`, `PropertyEditor.svelte`, `TagTable.svelte`, `QueryBlock.svelte`, `CollectionBlock.svelte`, `DocumentEditor.svelte`: grep for literal `Source Sans` / `Newsreader` / `Fira Code` / hex colors and route through `var(--font-sans|mono|...)` / Tailwind tokens. No structural / handler / prop changes. Most are no-ops thanks to Step 2's legacy-token aliasing.

## Interfaces and Data Flow

- `Region` widens; `StatusBar.svelte` is the only in-tree consumer that pattern-matches it (the deleted Sidebar + RightSidebar are the others).
- New store: `web/src/lib/stores/current-block.svelte.ts` (additive).
- New components: `Rail.svelte`, `MiddleColumn.svelte`, `BottomDrawer.svelte`, `CrumbBar.svelte`. No public types.
- `themes.ts` deletion is safe: only 4 in-tree callers, all updated by this spec.
- LocalStorage retired: `tesela:mode`. Added: `tesela:bottomDrawerOpen`, `tesela:bottomDrawerTab`. Preserved: `tesela:splitRatio`, `tesela:vimEnabled`, `tesela:fontSize`, `tesela:serverUrl`, home-views keys.
- Crumb hint markup is **static** for 9.0 (vision §9.0).

## Edge Cases and Failure Modes

- **Kanban split** preserved verbatim. The `Ctrl+w s/q/=/+/-` branches still route through `pane-state.svelte.ts`. Do not delete split-state code.
- **Esc inside bottom drawer** routes back to `"focus"` (no editor blur). Implement via top-level Escape in `BottomDrawer.svelte`.
- **Drawer closed → `^w j` no-op**. `b` opens it; opening does not auto-focus the drawer.
- **`b` chord vs vim `b`**: existing `isEditing` guard already excludes editors / inputs / contenteditable / `.cm-editor`.
- **No focused note** for backlinks/properties/outline tabs → "No note focused" empty state. History / Linked tasks always show their stub.
- **`pg/blk` toggle** with `focusedBlock = null` → force `pg`, disable `blk`.
- **Crumb overflow**: handled by `overflow:hidden white-space:nowrap` lifted from `v9-styles.css:56-58`.
- **3M.2 carry-forwards** (Cmd+Z bleed-through, cancel-and-flush vs redo race) must reproduce no worse than pre-9.0; verify in QA.

## Test Plan

### Type / lint

- `pnpm --dir web tsc --noEmit` — must pass.
- `pnpm --dir web lint` — must pass; expect unused-import warnings post-refactor.
- `pnpm --dir web run check` — Svelte runtime check.

### Manual QA (`pnpm --dir web dev`)

1. Initial paint: four-region grid, rail 232px / middle 300px / focus 1fr, crumb 32px top, status 24px bottom, drawer 220px above status.
2. Tokyo Night palette: bg `#1a1b26`, primary amber, no Newsreader/Source Sans anywhere. Font-family of any element resolves to Inter Tight or JetBrains Mono.
3. Crumb hints: right side reads `⌘K jump · ⌃w+hjkl split · b bottom` with `<kbd>` styling.
4. Rail clicks: each of Today / Timeline / Graph / Pages / Properties navigates and shows the inset-amber active stripe.
5. `^w h/j/k/l`: from focus, h→middle→rail; l reverses. j (drawer open)→bottom; k→focus. Old `^w h main→left` no longer applies.
6. `b` chord: outside editors, toggles drawer; localStorage persists across reload. `1` does the same.
7. Drawer tabs: Backlinks shows the same data the old right sidebar showed. Properties shows page+block props with pg/blk toggle. Outline lists indented blocks. History + Linked tasks both show "Coming in 9.x".
8. Vim preserved: i / Esc / dd / yy / p / o / O / >> / << work identically.
9. Drill-in preserved: clicking a sub-block sets `?block=...`; crumb segment updates; Outline tab reflects drilled tree.
10. Kanban split preserved: tag page + kanban view + vim → `^w s` opens split inside focus, `^w j/k` swaps, `^w q` closes, `^w =` equalizes.
11. ⌘K palette: opens with Tokyo styling. No "Toggle Theme" entry. Typing "theme" returns no result.
12. `/settings`: theme section gone. Shortcut list shows new `^w h/j/k/l` map and `b` for bottom.
13. Leader menu (Space outside editor): opens, navigates as before, restyled.
14. Slash / autocomplete / DatePicker in editor: reskinned but functional.
15. 3M.2 sanity: Cmd+Z atomic insert undo and Cmd+Shift+Z redo behave no worse than pre-9.0.

### Acceptance

- Zero TypeScript errors.
- All 15 QA scenarios pass.
- `grep -r "from \"\\$lib/themes\"" web/src` returns nothing.
- Visual diff vs `.docs/designs/v9-columns/columns/v9-preview.jpg` matches in column proportions, palette, fonts, and bottom-drawer tab strip.

## Handoff

- **Tier:** Sonnet (`spec-implementer`). Bounded refactor; design-decision-free.
- **Files:**
  - Deleted: `web/src/lib/themes.ts`.
  - Replaced: `Sidebar.svelte` → `Rail.svelte`; `RightSidebar.svelte` → `BottomDrawer.svelte`.
  - New: `Rail.svelte`, `MiddleColumn.svelte`, `BottomDrawer.svelte`, `CrumbBar.svelte`, `web/src/lib/stores/current-block.svelte.ts`.
  - Heavily edited: `+layout.svelte`, `app.css`, `app.html`, `pane-state.svelte.ts`, `routes/p/[id]/+page.svelte`, `routes/settings/+page.svelte`, `StatusBar.svelte`, `CommandPalette.svelte`, `commands.ts`.
  - Light reskin: components listed in Step 12.
- **Constraints:** no regression to vim, drill-in, splits, undo. Middle column is throwaway per vision — no widget logic. No kind glyphs in content, no parent breadcrumbs, no calendar, no real history, no linked tasks. History / Linked tasks tabs are muted stubs.
- **Commit:** final commit on green QA uses `feat(m9.0): columns shell + tokyo night` to trip the auto-release hook.
- **Blocking decisions:** none — vision doc locks all product questions.
