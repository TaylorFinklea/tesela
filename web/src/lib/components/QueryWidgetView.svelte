<script lang="ts">
  /**
   * Phase 9.5c — renders a Query widget's result inline inside the focus
   * pane, as either a kanban board or a real columnar table.
   *
   * tesela-ya4.3 — "table" mode now renders a REAL columnar table
   * (`QueryTable.svelte`, spec gap G4) instead of the row list this
   * component used to fall back to for both modes. `widgetView` is a
   * strict `"table" | "kanban"` union (no third "list" state), so once
   * kanban was generalized in ya4.1 to cover any DSL (G2), every
   * non-kanban render here was ALREADY typed as "table" — it just hadn't
   * been built yet. The old row-list + triage-chord/inline-edit/project-
   * picker machinery that used to live here (Phase 9.5c–12.2's triage UX)
   * is retired: `GrInbox`'s own "list" display mode is a SEPARATE surface
   * that never routed through this component and is unaffected.
   */
  import ViewSwitcher from "./ViewSwitcher.svelte";
  import KanbanBoard from "./KanbanBoard.svelte";
  import QueryTable from "./QueryTable.svelte";
  import { IconTable, IconLayoutKanban } from "@tabler/icons-svelte";
  import { parseQuery } from "$lib/query-language";
  import { getViewMode, setViewMode } from "$lib/stores/tag-view-prefs.svelte";
  import type { Widget } from "$lib/types/Widget";

  let {
    widget,
    onOpenRow,
  }: {
    widget: Widget;
    /** Row activation (kanban card open / table row open) routes here
     *  instead of a full `goto` navigation when set. Callers wire this to
     *  `openPageInFocused` so a result opens in the focused editor pane. */
    onOpenRow?: (pageId: string, blockId: string | null) => void;
  } = $props();

  /**
   * Phase 11 — view mode (table / kanban). Default comes from the Query
   * note's `view::` directive; user toggles persist in localStorage keyed
   * by widget id (reusing the same prefs store the Tag-page kanban uses).
   * The localStorage value wins so a user's toggle survives reloads even
   * if the Query note doesn't carry `view:: kanban`.
   */
  const widgetView: "table" | "kanban" = $derived.by(() => {
    const stored = getViewMode(widget.id);
    if (stored === "kanban" || stored === "table") return stored;
    return widget.view === "kanban" ? "kanban" : "table";
  });

  /**
   * Pull the first positive `tag:X` filter out of the query DSL. Used to
   * key the tag-page localStorage group-by pref and to give the kanban
   * board / table the type's own declared property order (decision 3c).
   * `null` when the query isn't tag-scoped (e.g. `kind:page
   * note_type:Project`) — kanban and table both still render
   * (tesela-ya4.1/G2, tesela-ya4.3: the block source generalized to
   * `executeQuery`, so a non-tag-scoped view no longer silently degrades).
   */
  const inferredTag: string | null = $derived.by(() => {
    if (!widget.query) return null;
    try {
      const parsed = parseQuery(widget.query);
      const tagFilter = parsed.filters.find((f) => f.key === "tag" && f.op === "Eq");
      return tagFilter ? tagFilter.value : null;
    } catch { return null; }
  });

  const showKanban = $derived(widgetView === "kanban");
  const showTable = $derived(widgetView === "table");

  function handleViewChange(mode: string) {
    if (mode === "table" || mode === "kanban") setViewMode(widget.id, mode);
  }
</script>

<div class="qwv">
  {#if widget.query.trim().length > 0}
    <div class="qwv-header">
      <span class="qwv-meta"></span>
      <ViewSwitcher
        views={[
          { id: "table",  label: "Table",  Icon: IconTable },
          { id: "kanban", label: "Kanban", Icon: IconLayoutKanban },
        ]}
        active={widgetView}
        onChange={handleViewChange}
      />
    </div>
  {/if}
  {#if widget.query.trim().length === 0}
    <div class="qwv-empty">(empty query — edit `query::` in the focus pane)</div>
  {:else if showKanban}
    <!-- tesela-ya4.1 — generalized kanban block source (decision 2/G2):
         KanbanBoard is fully driven by the DSL now, tag-scoped or not.
         `viewId`/`displayGroupBy` only carry the saved-view override
         (decision 3a) when `widget.viewId` marks this as a saved-view
         mount (`GrInbox`'s `modeWidget`) — a plain Query-note widget's
         `group::` frontmatter is NOT treated as an override, matching the
         pre-ya4.1 tag-page kanban behavior. -->
    <KanbanBoard
      dsl={widget.query}
      tagName={inferredTag}
      viewId={widget.viewId ?? null}
      displayGroupBy={widget.viewId ? widget.group : null}
      groupByStorageKey={inferredTag ?? widget.id}
      focused={true}
    />
  {:else if showTable}
    <!-- tesela-ya4.3 — generalized table block source (decision 2, gap
         G4): same DSL, same `executeQuery` source Kanban uses. -->
    <QueryTable
      dsl={widget.query}
      tagName={inferredTag}
      focused={true}
      {onOpenRow}
    />
  {/if}
</div>

<style>
  .qwv {
    outline: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .qwv-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 8px;
  }
  .qwv-meta {
    font-family: var(--v9-mono);
    font-size: 11px;
    color: var(--v9-ink-faint);
  }
  .qwv-empty { color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 12px; padding: 12px 0; }
</style>
