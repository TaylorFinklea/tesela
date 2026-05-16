# Prism v5 — Phase 0 inventory

> Read-only catalog of the v4 surface area. Inputs for Phases 1–13.
> Generated 2026-05-15 from `main` at HEAD.

Classifications:
- **delete** — surface has no v5 equivalent; remove in Phase 13 cleanup.
- **keep-as-renderer** — survives v5 as a thin renderer adapter (no rewrite).
- **rewrite** — reshape for the v5 protocol; same conceptual surface.
- **keep-as-is** — non-v4-specific surface; v5 reuses unchanged.

---

## 1. `web/src/lib/components/v4/` — 15 files

| File | Purpose | Imports | Used by | v5 fate |
|---|---|---|---|---|
| `BacklinksTab.svelte` | List of pages linking to a note. Block-level navigation. | api-client, ParsedBlock | `ContextPane`, `PeekPopover` | **keep-as-renderer** → `derived/backlinks-of-page` |
| `ColonCommandLine.svelte` | `:` ex-mode line at bottom of viewport; parses verbs from `commands.ts`. | colon-mode store, v4/commands | `+layout.svelte` | **rewrite** — re-wire against v5 verb registry (palette + ambient launchers) |
| `ContextPane.svelte` | The v4 `context` pane host. Switches between BacklinksTab / OutlineTab / PropertiesView / HistoryTab / LinkedTasksTab. | the five tab components | `PaneShell` | **delete** — replaced wholesale; each tab becomes its own derived renderer with the host-agnostic protocol |
| `FullscreenOverlay.svelte` | The `⌘G` graph + `⌘,` settings overlay multiplexer. | api-client, GraphCanvas, SettingsOverlay, fullscreen-overlay store | `+layout.svelte` | **rewrite** — keep the overlay shell; `⌘G` remains; `⌘,` may move; verify graph mount survives Phase 13 |
| `Journey.svelte` | Slim breadcrumb of recently-opened tiles; ⌘[ / ⌘] navigation. | journey store, openInEditor | `+layout.svelte` | **rewrite** — repoint into v5 buffer model (tile id → pageId) |
| `LayoutTree.svelte` | Recursive renderer for the binary pane tree; mounts PaneShell at leaves; drag handles for resize. | pane-tree types, setSplitSizes, PaneShell | `+layout.svelte` | **rewrite** — leaf payload becomes `Buffer` union; resize stays |
| `NoteRenderer.svelte` | Picks the right per-note view component (BlockOutliner / JournalView / DocumentEditor / QueryWidgetView / TagTable / PropertyTypeConfig) based on note `type`. | many inner components | `PaneShell` | **keep-as-renderer** — this is *already* the page-type dispatch; in v5 it becomes the page registry's lookup function. Rename + relocate. |
| `OutlineTab.svelte` | Headings/structure outline of a note. | api-client, ParsedBlock | `ContextPane`, `PeekPopover` | **keep-as-renderer** → `derived/outline-of-page` |
| `PaneKindMenu.svelte` | The pane-header `kind` chip — dropdown to swap a pane's kind. | pane types | `PaneShell` | **delete** — kind-swapping no longer exists; pane *kinds* are now (page / derived / ambient) and you don't reassign them via a chip |
| `PaneShell.svelte` | Renders a single pane: header (focus dot, title/picker, kind menu, close), body (kind-specific branch). The big file with the five-way `if/else if pane.kind`. | NoteRenderer, ContextPane, QueryWidgetView, JournalView, GraphCanvas, PaneKindMenu | `LayoutTree` | **rewrite** → renamed `BufferShell.svelte`; replaces 5-way branch with 3-way `{page, derived, ambient}` dispatch + `<svelte:boundary>` |
| `PeekPopover.svelte` | `⌘I` floating context popover; currently shows BacklinksTab/OutlineTab/PropertiesView for the focused tile. | the three tab components, peek store, pane-tree | `+layout.svelte` | **rewrite** — host-agnostic renderer hosting; Tab cycles; renderer code path shared with derived-buffer pane |
| `PropertiesView.svelte` | Frontmatter properties editor for a note. | api-client, ParsedBlock | `ContextPane`, `PeekPopover` | **keep-as-renderer** → `derived/properties-of-page` |
| `SettingsOverlay.svelte` | Inline settings UI inside the FullscreenOverlay (general/devices/sync/mosaic/data subpages). | settings page components | `FullscreenOverlay` | **keep-as-is** — settings UI is independent of v5 chrome; reuse |
| `Station.svelte` | The `⌘K` Command Station — palette + dashboard, fuzzy search, recent. | v4/commands, widget-registry, fuzzy, station store | `+layout.svelte` | **rewrite** — palette + ambient launcher only; rip the dashboard tab; widgets move to ambient buffers |
| `TopBarTabs.svelte` | Tab strip in the top bar; per-tab kind-count chip. | pane-tree, leaves() | `+layout.svelte` | **rewrite** — kind-count math switches to (page count · derived count · ambient count) |

## 2. `web/src/routes/v4/` — 3 files

| File | Purpose | v5 fate |
|---|---|---|
| `+layout.svelte` | The v4 shell — top bar, Journey, LayoutTree, status line, overlays. Hosts all keymap. | **rewrite** — same role, v5-shaped: replaces `LayoutTree` mount with v5 BufferShell tree; adds left sidebar region; renames v4 storage key references |
| `+page.svelte` | URL → state adapter for `/v4`. Consumes `#tile=` hash, seeds today's daily. | **rewrite** — keep the shape; reads v5 `Buffer` union; seeds page-buffer today's daily |
| `p/[id]/+page.svelte` | Deep-link shim — `/v4/p/<id>` jumps to a tile in v4. | **rewrite** — same shim against v5 pane tree |

## 3. State files in `web/src/lib/stores/`

| File | Purpose | v5 fate |
|---|---|---|
| `pane-tree.ts` | Pure data layer. Binary-tree types (`LeafNode`, `SplitNode`, `Pane` 5-kind union), mutations (`vsplit`, `hsplit`, `closePane`, `movePane`, `jumpToTile`, `stackAdd`, `stackNext`, `swapKind`, `setPaneWidget`, `setSplitSizes`, tab ops), traversal, serialization, v1→v2 migration. | **rewrite** — replace `Pane` union with v5 `Buffer` discriminated union. Algebra (`vsplit`/`hsplit`/`closePane`/etc.) stays. Add `v2→v3` migration. Drop `swapKind`/`setPaneWidget` (no longer meaningful). |
| `pane-tree.svelte.ts` | Reactive Svelte 5 wrapper; persistence; `lastEditorByTab`; `resolveEditorTarget` / `openInEditor`. | **rewrite** — same role v5-shaped. `focusPane` becomes the single chokepoint for the follow rule (data-driven on `buffer.kind === "page"`). Rename localStorage key to `tesela:prism:state`. |
| `pane-state.svelte.ts` | Per-pane vim mode tracking (insert vs normal). | **keep-as-is** — independent of buffer kind. |
| `active-pane-nav.svelte.ts` | Cross-pane navigation helpers (graph node clicks, etc.). | **rewrite** — adapt to `Buffer` discriminator; possibly fold into `openInEditor`. |
| `colon-mode.svelte.ts` | `:` ex-mode open/close + pending text. | **keep-as-is** — orthogonal to buffer kinds. |
| `fullscreen-overlay.svelte.ts` | State for `⌘G` graph + `⌘,` settings overlays; tracks active kind + selected settings slug. | **keep-as-is** — overlay state independent of buffer kinds. |
| `journey.svelte.ts` | Tile-jump history; ⌘[ / ⌘] navigation. | **keep-as-is** — record of tile (=page) ids; v5-compatible. |
| `peek.svelte.ts` | Peek open/close state. | **rewrite** — extend with cycle position + per-page-type first-shown memory. |
| `station.svelte.ts` | Station open/close state + last query + prior-pane id. | **keep-as-is**. |
| `current-block.svelte.ts` (existing, not listed but used) | Per-pane focused block map. | **keep-as-is** — Phase 1.5 of v4 already paneId-keyed it. |

## 4. `web/src/lib/v4/`

| File | Purpose | v5 fate |
|---|---|---|
| `commands.ts` | V4 verb registry — vsplit/hsplit/quit/tabnew/jump/stack/daily/settings-*. | **rewrite** — replaces the verb set: drop `swap-kind`, `set-widget`; add `:scratch`, `:promote`, `:backlinks`, `:outline`, `:properties`, `:tasks`, `:graph-local`, `:calendar`, `:in-progress`, `:dashboard`, `:ai`. |
| `tokens.css` | CSS custom properties for the v4 palette (--v4-accent, --v4-bg, etc.). | **keep-as-is** — rename to `v5/tokens.css` (cosmetic). |

## 5. Leaf-reusable components for v5 renderers (in `web/src/lib/components/`)

| File | Used today as | v5 fate |
|---|---|---|
| `BlockOutliner.svelte` | Block-level outliner editor for notes | **keep-as-renderer** → wrap as page-renderer for `type: note` and `type: scratch` |
| `JournalView.svelte` | Logseq-style continuous journal | **keep-as-renderer** → page-renderer for `type: daily` |
| `QueryWidgetView.svelte` | Renders a Query-typed note's `query::` DSL as a results list | **keep-as-renderer** → page-renderer for `type: query` |
| `GraphCanvas.svelte` | Graph visualization | **keep-as-renderer** → used by `derived/local-graph-of-page` and by `⌘G` fullscreen overlay |
| `LinkedTasksTab.svelte` | Tasks that reference a given page | **keep-as-renderer** → `derived/tasks-linked-to-page` |
| `HistoryTab.svelte` | Edit history of a note | **defer** — not in initial v5 derived set; revisit |
| `DocumentEditor.svelte` | Long-form prose editor (alt view mode) | **keep-as-renderer** → page-renderer for prose mode (or fold into outliner; investigate Phase 3) |
| `TagTable.svelte` | Tabular view of pages tagged with X | **keep-as-renderer** → page-renderer for `type: tag` (deferred page type) |
| `PropertyTypeConfig.svelte` | Property type editor | **keep-as-is** — settings-shape UI, not a page renderer |

## 6. Overlays — already separate from pane kinds

| Overlay | Trigger | v5 fate |
|---|---|---|
| Command Station | `⌘K` | **rewrite** — palette + launcher; drop dashboard tab |
| Peek | `⌘I` | **rewrite** — host-agnostic renderer hosting, Tab cycle |
| Fullscreen graph | `⌘G` | **rewrite** — keep overlay shell; underlying `GraphCanvas` survives |
| Settings | `⌘,` (via FullscreenOverlay multiplexer) | **keep-as-is** — settings UI unchanged |

## 7. `focusPane` call sites (5 files)

| File | Call sites | Notes |
|---|---|---|
| `web/src/lib/stores/pane-tree.svelte.ts` | `focusPane`, `openInEditor` (via `focusPane`) | The wrapper API — keep, modify to enforce follow rule. |
| `web/src/lib/stores/pane-tree.ts` | Internal — the pure mutation | Renamed in v5 to use `Buffer` discriminator. |
| `web/src/lib/components/v4/Station.svelte` | 3 sites (lines 130, 199, 222) | All "jump to tile" paths; reroute through `openInEditor`. |
| `web/src/lib/components/v4/PaneShell.svelte` | 4 sites (lines 130, 241, 279, 326) | Click → focus this pane patterns. Translate to BufferShell. |
| `web/src/routes/v4/+page.svelte` | 2 sites (lines 73, 79) | Seed flow; survives v5 with kind check shifted to `buffer.kind === "page"`. |
| `web/src/routes/v4/p/[id]/+page.svelte` | 1 site (line 28) | Deep-link shim. |

## 8. Kind-read call sites (5-way `pane.kind` reads)

Concentrated in a small set of files:

- `web/src/lib/stores/pane-tree.ts` — 5 sites (firstEditorTile, findTile, swapKind, setPaneWidget).
- `web/src/lib/stores/pane-tree.svelte.ts` — 3 sites (trackLastEditor, resolveEditorTarget x2).
- `web/src/lib/components/v4/PaneShell.svelte` — ~22 sites (the big 5-way branch). All replaced by 3-way `buffer.kind`.
- `web/src/lib/components/v4/PaneKindMenu.svelte` — 2 sites (chip label, dropdown value). File gets deleted.
- `web/src/lib/components/v4/PeekPopover.svelte` — 2 sites (only matches `editor`). Re-shape with v5 follow resolution.
- `web/src/lib/components/v4/TopBarTabs.svelte` — 1 site (kind counting). Re-shape to 3-kind counting.
- `web/src/routes/v4/+layout.svelte` — 1 site (focusedPane.kind === "editor"). Re-shape.
- `web/src/routes/v4/+page.svelte` — 2 sites (editor seeding logic). Re-shape.
- `web/src/routes/graph/+page.ts` — 1 comment-only mention.

## 9. Notes on dependencies and migration risk

- **`NoteRenderer.svelte` is already the page-type dispatch.** That's a substantial v5 head-start. Its switch over `note.metadata.note_type` is exactly the v5 page-renderer registry's lookup. In Phase 3, NoteRenderer becomes the page registry's `register(name, renderer)` call site rather than a hardcoded `if/else` ladder.
- **`ContextPane` becomes obsolete.** v5 derived buffers replace it; each derived renderer (BacklinksTab, OutlineTab, PropertiesView, LinkedTasksTab) lives independently in `web/src/lib/renderers/derived/<name>/index.ts`.
- **`PaneKindMenu` is deleted with no v5 equivalent.** Swapping kinds via UI no longer makes sense; the kind is determined by the verb that opened the pane.
- **`PeekPopover` and the new derived-buffer pane share renderer code.** Phase 9 verifies host-agnosticism by running the same modules in both hosts.
- **The localStorage rename (`tesela:prism4:v1` → `tesela:prism:state`) is one-shot.** Migration walks the v4 tree and produces v5 `Buffer` leaves. Captured v4 blobs needed for golden-file tests; obtain them in Phase 2 prep.
- **Five-way kind branches are concentrated in PaneShell.** Replacing PaneShell with BufferShell (Phase 3) eliminates 22 of the ~36 total branch sites. The remaining sites are small audits in routes/stores/Station.

## 10. Deletion candidates (Phase 13 cleanup list)

Direct deletions (no v5 equivalent):
- `web/src/lib/components/v4/ContextPane.svelte`
- `web/src/lib/components/v4/PaneKindMenu.svelte`

Renames + rewrites (Phase 3+ deletes the original):
- `web/src/lib/components/v4/PaneShell.svelte` → `BufferShell.svelte`

Possible later renames (not blocking):
- `web/src/lib/v4/` → `web/src/lib/v5/` or `web/src/lib/prism/`
- `web/src/lib/components/v4/` → `web/src/lib/components/prism/`
- `web/src/routes/v4/` → `web/src/routes/` (move shell to root once v5 stabilizes)

Renames are cosmetic; defer until after Phase 13.

---

## Exit summary

- 15 v4 components catalogued; 2 marked **delete**, 1 marked **delete + rename target**, 6 marked **rewrite**, 4 marked **keep-as-renderer**, 1 **keep-as-is**, 1 **rewrite (overlay shell)**.
- 3 route files; all **rewrite** in place.
- 10 store files; 4 **rewrite**, 6 **keep-as-is**.
- 9 leaf-reusable component candidates from `web/src/lib/components/`; 7 land as renderers, 2 deferred.
- 4 overlays; all survive (3 rewrite, 1 unchanged).
- 14 `focusPane` call sites across 6 files; all reachable from grep.
- 36+ `pane.kind` read sites across 9 files; 22 collapse with the PaneShell → BufferShell rewrite.

Phase 0 complete. Ready to start Phase 1 (additive v5 type system + registries).
