# Type-system VIEWS (kanban / sets / table) — Lead decomposition spec

Authored 2026-07-02 (Fable / Opus). Epic `tesela-ya4`. This is the **Lead
decomposition only** — no implementation. Children filed in beads (`bd`), one
commit = this spec.

## Product locks (binding — do not re-litigate)
- **Slice order: KANBAN on web first → sets/TABLE (same data layer) → iOS.** (harness-deck 2026-07-01)
- **Keyboard-first is non-negotiable** (north-star lock 2026-06-12). Web views are vim-navigable; iOS is touch-first (no vim, per `project_mobile_strategy`).
- **Arc order: SPINE FIRST** — implement ya4 children only after `cmdd` + `pfix` substantially land. This spec unblocks; it does not authorize starting impl before the spine.
- **JQL, not more colon-DSL UX** (memory `project_query_jql_preference`): views consume the existing query engine; do not add new DSL surface.

## What ALREADY EXISTS (surveyed 2026-07-02 — read before building)
The "deferred hard part" is **mostly built on web**; the real work is (a) making
saved views' stored `display_mode` / `display_group_by` actually drive rendering
end-to-end, (b) a real TABLE mode, and (c) iOS parity (today iOS renders every
mode as a list).

### Web — built
- `web/src/lib/components/KanbanBoard.svelte` — FULL keyboard nav (`j/k/h/l`, `g/G`, `Enter`, `m` move-picker, `H/L` move card ±column, `i` open drawer), HTML5 DnD, group-by `<select>`, chip registry (`buildRegistry`), `__unset__` column. **Keyed by `tagName`** → `api.getType(tag)` + `api.getTypedBlocks(tag)`. Group-by resolves from `getGroupByProp(tagName)` (localStorage `tag-view-prefs`) or first `value_type==="select"` property.
- `web/src/lib/components/KanbanCard.svelte`, `KanbanColumnPicker.svelte` — card + move-picker.
- `web/src/lib/components/TagTable.svelte` (248 lines) — columnar table for tag pages (sortable, inline edit) — **NOT wired into saved-view / QueryWidgetView table mode.**
- `web/src/lib/components/QueryWidgetView.svelte` — `ViewSwitcher` table/kanban; mounts `KanbanBoard` when `inferredKanbanTag` (first positive `tag:X` in the DSL) is non-null AND `widgetView==="kanban"`. `widgetView` comes from per-widget localStorage (`getViewMode(widget.id)`) or `widget.view`. **"table" ViewSwitcher option = the same qwv-row list, NOT a real columnar table.**
- `web/src/lib/graphite/views/GrInbox.svelte` (the `/g` Views surface) — consumes the saved-views registry (`api.listViews()`), and for `display_mode` table/kanban **mounts `QueryWidgetView` over a synthetic widget** (`view: displayMode`, `group: selected.display_group_by`). DSL-first editor + chip inserters (`view-dsl.ts`).

### Web — registry (built)
- Server `/views` CRUD + `/views/reorder` (`crates/tesela-server/src/routes/views.rs`); `ViewRecord` fields `display_mode` / `display_group_by` / `display_show_done`; WS `views_changed`; seeded builtin Inbox (`INBOX_VIEW_DSL`). Client wrappers in `web/src/lib/api-client.ts` (`listViews`/`createView`/`updateView`/`deleteView`/`reorderViews`).

### iOS — built
- `app/Tesela-iOS/Sources/Data/SavedViews.swift` — `SavedView` model mirrors `ViewRecord` (`displayMode`/`displayGroupBy`/`displayShowDone` **stored, not rendered**).
- `app/Tesela-iOS/Sources/Graphite/Views/GrViewEditorSheet.swift` — DSL-first editor + chip inserters + validation parity. Display-mode picker exists but is a stored preference only.
- `app/Tesela-iOS/Sources/Graphite/Views/GrInboxView.swift` — view-switcher chips + result `List`. **Renders EVERY `display_mode` as a list** with an honest note `"<mode> view — shown as a list on iOS; full layout on web"` (`displayModeNote`, line 271). NO kanban, NO table.
- `app/Tesela-iOS/Sources/Data/LocalQueryEngine.swift` — `.relay` DSL executor.

### Engine / conformance — built
- `SqliteIndex::get_typed_blocks` (`crates/tesela-core/src/db/sqlite.rs:985`) — `SELECT id,title,body FROM notes WHERE body LIKE ? OR tags LIKE ?` then `parse_blocks` **per matched note, every call** (LIKE prefilter, no cache).
- Three query engines with a shared conformance fixture already in place: `crates/tesela-core/tests/fixtures/query-conformance.json` consumed by Rust (`query_conformance.rs`), web (`web/tests/unit/query-conformance.test.mjs`), iOS (`app/Tesela-iOS/Tests/QueryConformanceTests.swift`).

## GAPS (the epic's actual remaining work)
- **G1 — `display_group_by` ignored at render.** A saved kanban view stores `display_group_by`, but `KanbanBoard` resolves group-by from localStorage/first-select and never reads it. `GrInbox` passes it as `widget.group` (server list grouping only). A saved kanban view can't pin its column.
- **G2 — kanban is per-TYPE only.** It renders only when the DSL has a positive `tag:X` (via `getTypedBlocks(tag)`). A saved view scoped by `status:`/other with no tag silently falls back to a list even in kanban mode. The epic's charter — "group blocks by a select property" — implies grouping **arbitrary query results**, not just one type's blocks.
- **G3 — two block sources unreconciled.** Tag-page kanban uses `getTypedBlocks`; saved-view list uses `executeQuery`. Kanban and list can diverge (different filters/caps).
- **G4 — no real TABLE mode for saved views.** `TagTable.svelte` exists but the QueryWidgetView / GrInbox "table" path renders the row-list. Sets-as-table (columns = resolved type properties, typed cells, keyboard cell nav) is unbuilt.
- **G5 — no table column config** (hide / reorder / sort persistence) for saved views.
- **G6 — iOS has no kanban and no table.** Every mode renders as a list.
- **G7 — iOS can't edit `display_group_by`** (stored, not edited).
- **G8 — perf: views multiply query executions.** Each saved view + type page re-runs `get_typed_blocks` / `executeQuery`, each re-`parse_blocks`-ing matched note bodies with no server cache. `tesela-sclr.2` (per-note parsed-blocks cache keyed by content hash) covers the SAME `parse_blocks` cost. **Sequencing decision below.**

## Architecture decisions (this spec locks these)
1. **Canonical model: a per-type page IS a saved-view-shaped surface scoped to `tag:Type`.** kanban / table / list are **display modes over grouped query results**, not separate features. New per-type view work routes through the views/query path, not a parallel `getTypedBlocks` UI.
2. **Single block source = `executeQuery(dsl)`.** Kanban groups the query result client-side by the resolved group-by select property. Keep `getTypedBlocks` as an internal optimization ONLY if it returns byte-identical membership to `executeQuery("tag:X kind:block")`; otherwise retire the divergent path (G3). Implementer proves equivalence with a test.
3. **Group-by resolution order (locked):** (a) explicit `display_group_by` on the active saved view → (b) per-surface localStorage pref (tag page) → (c) first `value_type==="select"` property with ≥1 choice → (d) honest "no groupable select property" empty state (never silently fall back to a list under a kanban toggle).
4. **`display_group_by` is round-trip-authoritative.** Changing group-by inside a saved-view kanban writes back to the view (`updateView`), not localStorage. Tag-page (non-saved) kanban keeps localStorage.
5. **iOS is touch-first.** kanban = horizontally-paged columns + long-press/move-sheet to change column (mirror the web move-picker semantics, not DnD-vim). table = compact columnar list. No vim keys.
6. **Conformance stays shared.** Any DSL/grouping semantics added extend `query-conformance.json` and pass in all three engines (drift = per-device view divergence).

## sclr.2 coordination (perf gate)
Views multiply the exact `parse_blocks`-per-note cost that `tesela-sclr.2` eliminates
(per-note parsed-blocks cache, or indexed blocks table). Decision:
- The **data-layer beads that broaden how often the query path runs** (ya4.1 generalized kanban source; ya4.3 table-over-query) are **blocked-by `tesela-sclr.2`** — do not multiply an O(whole-corpus-reparse) query across 6–12 saved views before the cache lands.
- Pure-UI beads (keyboard UX, iOS render) are NOT gated on sclr.2.
- Every data-layer bead **benches before/after on the 5k-note synthetic mosaic** (`crates/tesela-fixtures` `MosaicBuilder`) and records numbers in its close note. sclr.2's fix MUST cover `get_typed_blocks` (or its replacement `executeQuery` path), not just the Inbox scan — verify.

## Phase plan → child beads
Phases are sequential; beads within a phase may parallelize where deps allow.
All impl beads `tier_floor: senior` (epic decomposition was the only lead item).

### Phase 1 — Web kanban data layer
- **ya4.1** — Thread saved-view `display_group_by` + generalize the kanban block source (G1/G2/G3). Kanban renders from `executeQuery` results grouped by the resolved select property; honors decision-3 resolution order; honest empty state; group-by change in a saved-view context persists via `updateView`. Reconcile/retire `getTypedBlocks` per decision-2. **blocked-by sclr.2.** `complexity: L`.

### Phase 2 — Web kanban keyboard UX
- **ya4.2** — Keyboard-complete the kanban: change group-by from the keyboard (not just the `<select>`), create-card-in-column, and register the board's actions in the command registry / leader menu (`cmdd`) so ⌘K + leader reach them. Preserve existing `j/k/h/l/g/G/Enter/m/H/L/i`. `complexity: M`. depends: ya4.1.

### Phase 3 — Sets / table (same data layer)
- **ya4.3** — Real TABLE render mode for saved views + type pages: columns = resolved type properties, typed cells, keyboard row/column nav; wired into QueryWidgetView "table" + GrInbox `display_mode==="table"`; reconcile with `TagTable.svelte` (reuse or supersede). **blocked-by sclr.2.** `complexity: L`. depends: ya4.1.
- **ya4.4** — Table column config persistence (hide / reorder / sort) per saved view (extend the view display config; server + web) or per-tag prefs for non-saved pages. `complexity: M`. depends: ya4.3.

### Phase 4 — iOS
- **ya4.5** — iOS kanban view: native columns grouped by the resolved select property, touch move via a column-picker sheet (mirror web move-picker), honoring `display_mode==="kanban"` + `display_group_by`; replaces the list-with-honest-note for kanban. `.relay` (FFI) + `.http`. `complexity: L`. depends: ya4.1.
- **ya4.6** — iOS table view: native compact columnar table for `display_mode==="table"`, columns = resolved properties; replaces the honest-note for table. `complexity: M`. depends: ya4.3, ya4.5.
- **ya4.7** — iOS view-editor: edit `display_mode` + `display_group_by` on iOS (today stored-not-edited) — add a group-by picker to `GrViewEditorSheet`, sourced from the type's select properties. `complexity: M`. depends: ya4.5.

## Dependency graph
```
sclr.2 ──▶ ya4.1 ──▶ ya4.2
              │  └──▶ ya4.5 ──▶ ya4.6
              └──▶ ya4.3 ──▶ ya4.4
sclr.2 ──▶ ya4.3          ya4.5 ──▶ ya4.7
                          ya4.3 ──▶ ya4.6
```

## Out of scope (explicit)
Cross-view dashboards; view sharing between mosaics; per-view notifications;
NL→DSL ("show me overdue"); calendar/timeline/gallery display modes; multi-value
group-by (one select property per board). All later.
