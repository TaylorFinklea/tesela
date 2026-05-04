<script lang="ts">
  /**
   * Phase 9.5c — renders a Query widget's result list inline inside the
   * focus pane. Replaces the old MiddleColumn for Query-typed notes.
   *
   * Rows route through `gotoNote` so clicking a result drills (creating
   * the column-view split with the source widget on the left).
   */
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";
  import { applyTriage, attachToProject, triageActionForKey, setBlockProperty } from "$lib/triage.svelte";
  import ProjectPicker from "./ProjectPicker.svelte";
  import type { Note } from "$lib/types/Note";
  import type { QueryItem } from "$lib/types/QueryItem";
  import type { Widget } from "$lib/types/Widget";

  type Row = {
    id: string;
    label: string;
    breadcrumb?: string[];
    primaryTag?: string;
    blockId?: string;
    pageId: string;
    status?: string;
    kind: "block" | "page";
  };

  let { widget }: { widget: Widget } = $props();
  const queryClient = useQueryClient();

  let projectPickerRow = $state<Row | null>(null);
  let selectedIndex = $state(0);
  let rootEl = $state<HTMLElement | undefined>();

  // Phase 9.9 — auto-focus the result list on mount so j/k works without
  // requiring the user to click first.
  $effect(() => {
    if (rootEl && document.activeElement !== rootEl) rootEl.focus();
  });

  // Phase 9.9 — status cycle on `s`. The full Status property choice list
  // could be richer, but the daily-driver path is the same triage trio.
  const STATUS_CYCLE = ["todo", "doing", "done"] as const;
  function nextStatus(current: string | undefined): string {
    const idx = STATUS_CYCLE.indexOf((current ?? "") as typeof STATUS_CYCLE[number]);
    return STATUS_CYCLE[(idx + 1) % STATUS_CYCLE.length];
  }
  async function cycleRowStatus(row: Row): Promise<void> {
    if (!row.blockId || row.kind !== "block") return;
    const next = nextStatus(row.status);
    try {
      const note = await api.getNote(row.pageId);
      const updated = setBlockProperty(note.content, row.blockId, "status", next);
      if (updated === note.content) return;
      await api.updateNote(row.pageId, updated);
      queryClient.invalidateQueries({ queryKey: ["widget", widget.id] });
      queryClient.invalidateQueries({ queryKey: ["note", row.pageId] });
    } catch (e) {
      console.error("Status cycle failed:", e);
    }
  }
  function statusGlyph(status: string | undefined): string {
    switch (status) {
      case "todo": return "○";
      case "doing": return "◑";
      case "done": return "✓";
      case "in-review": return "◐";
      case "backlog": return "·";
      case "canceled": return "×";
      case "on-hold": return "❘❘";
      default: return "·";
    }
  }

  const widgetResultQuery = createQuery(() => ({
    queryKey: ["widget", widget.id, widget.query, widget.group, widget.sort] as const,
    queryFn: () =>
      widget.query.trim().length > 0
        ? api.executeQuery(widget.query, widget.group, widget.sort)
        : Promise.resolve({ groups: [] }),
    enabled: widget.query.trim().length > 0,
  }));

  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", "all-for-picker"] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: widget.id === "inbox",
  }));
  const allNotes = $derived((allNotesQuery.data ?? []) as Note[]);

  async function handleProjectSelect(project: Note) {
    const row = projectPickerRow;
    projectPickerRow = null;
    if (!row || !row.blockId || !row.pageId) return;
    try {
      const ok = await attachToProject(row.pageId, row.blockId, project.id);
      if (ok) queryClient.invalidateQueries({ queryKey: ["widget", widget.id] });
    } catch (e) {
      console.error("Attach to project failed:", e);
    }
  }

  const TRIAGED_PAGE_TYPES = new Set(["Tag", "Property", "Query", "Template"]);
  function isInboxableRow(item: QueryItem): boolean {
    if (item.kind !== "block") return false;
    if (/^\d{4}-\d{2}-\d{2}$/.test(item.page_id)) return false;
    if (item.page_note_type && TRIAGED_PAGE_TYPES.has(item.page_note_type)) return false;
    return true;
  }

  function itemToRow(item: QueryItem): Row {
    return {
      id: item.block_id ?? item.page_id,
      label: item.text || item.title,
      breadcrumb: item.parent_breadcrumb,
      primaryTag: item.primary_tag ?? undefined,
      blockId: item.block_id ?? undefined,
      pageId: item.page_id,
      status: item.properties?.status as string | undefined,
      kind: item.kind,
    };
  }

  type View = {
    rows: Row[];
    groups?: { key: string; rows: Row[] }[];
    error?: string;
    subtitle: string;
  };

  const view = $derived.by((): View => {
    const result = widgetResultQuery.data;
    const isInbox = widget.id === "inbox";
    const filteredGroups = (result?.groups ?? []).map((g) => ({
      ...g,
      items: isInbox ? g.items.filter(isInboxableRow) : g.items,
    }));
    const total = filteredGroups.reduce((acc, g) => acc + g.items.length, 0);
    const groups = filteredGroups.map((g) => ({
      key: g.key || "—",
      rows: g.items.map(itemToRow),
    }));
    const useGroups = !(groups.length === 1 && groups[0].key === "—");
    return {
      rows: useGroups ? [] : groups[0]?.rows ?? [],
      groups: useGroups ? groups : undefined,
      subtitle: widget.query
        ? `${total} ${widget.query.includes("kind:page") ? "pages" : "blocks"}`
        : "(empty query — edit `query::` in the focus pane)",
      error: widgetResultQuery.error
        ? (widgetResultQuery.error as Error).message
        : undefined,
    };
  });

  const flatRows = $derived<Row[]>(view.groups ? view.groups.flatMap((g) => g.rows) : view.rows);

  function openRow(row: Row) {
    gotoNote(row.pageId, row.blockId ?? null);
  }

  async function triageRow(row: Row, key: string): Promise<boolean> {
    const action = triageActionForKey(key);
    if (!action || !row.blockId || !row.pageId) return false;
    try {
      const ok = await applyTriage(row.pageId, row.blockId, action);
      if (ok) queryClient.invalidateQueries({ queryKey: ["widget", widget.id] });
      return ok;
    } catch (e) {
      console.error("Triage failed:", e);
      return false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(flatRows.length - 1, selectedIndex + 1);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && flatRows[selectedIndex]) {
      e.preventDefault();
      openRow(flatRows[selectedIndex]);
    } else if (e.key === "s" && flatRows[selectedIndex]?.kind === "block") {
      // Phase 9.9 — `s` cycles the highlighted row's status without
      // leaving the result list.
      e.preventDefault();
      void cycleRowStatus(flatRows[selectedIndex]);
    } else if (widget.id === "inbox" && flatRows[selectedIndex] && triageActionForKey(e.key) !== null) {
      e.preventDefault();
      void triageRow(flatRows[selectedIndex], e.key);
    } else if (widget.id === "inbox" && e.key === "p" && flatRows[selectedIndex]) {
      e.preventDefault();
      projectPickerRow = flatRows[selectedIndex];
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div class="qwv" tabindex="0" bind:this={rootEl} onkeydown={handleKeydown}>
  <div class="qwv-meta">{view.subtitle}</div>
  {#if view.error}
    <div class="qwv-error">Query error: {view.error}</div>
  {:else if view.groups}
    {#each view.groups as g}
      {#if g.rows.length > 0}
        <div class="qwv-grp">{g.key} <span class="qwv-grp-n">{g.rows.length}</span></div>
        {#each g.rows as row}
          {@const ri = flatRows.indexOf(row)}
          {@const sel = selectedIndex === ri}
          <div class="qwv-row {sel ? 'selected' : ''}">
            <span class="qwv-marker">{sel ? "▸" : ""}</span>
            {#if row.kind === "block" && (row.status !== undefined || row.primaryTag === "Task")}
              <button
                class="qwv-status"
                type="button"
                title="Status: {row.status ?? '(none)'} — click to cycle"
                onclick={(e) => { e.stopPropagation(); selectedIndex = ri; void cycleRowStatus(row); }}
              >{statusGlyph(row.status)}</button>
            {/if}
            <button
              class="qwv-text-btn"
              type="button"
              onclick={() => { selectedIndex = ri; openRow(row); }}
            >
              <span class="qwv-text">
                {#if row.primaryTag}
                  <span class="kind-badge kind-{row.primaryTag.toLowerCase()}">{row.primaryTag}</span>
                {/if}
                {row.label}
              </span>
            </button>
          </div>
          {#if row.breadcrumb && row.breadcrumb.length > 0}
            <div class="qwv-src">↳ {row.breadcrumb.join(" / ")}</div>
          {/if}
        {/each}
      {/if}
    {/each}
  {:else if flatRows.length === 0}
    <div class="qwv-empty">— empty —</div>
  {:else}
    {#each flatRows as row, ri}
      {@const sel = selectedIndex === ri}
      <button
        class="qwv-row {sel ? 'selected' : ''}"
        type="button"
        onclick={() => { selectedIndex = ri; openRow(row); }}
      >
        <span class="qwv-marker">{sel ? "▸" : ""}</span>
        <span class="qwv-text">
          {#if row.primaryTag}
            <span class="kind-badge kind-{row.primaryTag.toLowerCase()}">{row.primaryTag}</span>
          {/if}
          {row.label}
        </span>
      </button>
      {#if row.breadcrumb && row.breadcrumb.length > 0}
        <div class="qwv-src">↳ {row.breadcrumb.join(" / ")}</div>
      {/if}
    {/each}
  {/if}
</div>

{#if projectPickerRow}
  <ProjectPicker
    notes={allNotes}
    onselect={handleProjectSelect}
    onclose={() => (projectPickerRow = null)}
  />
{/if}

<style>
  .qwv {
    outline: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .qwv-meta {
    font-family: var(--v9-mono);
    font-size: 11px;
    color: var(--v9-ink-faint);
    margin-bottom: 8px;
  }
  .qwv-error { color: var(--v9-rose); font-family: var(--v9-mono); font-size: 12px; padding: 12px 0; }
  .qwv-empty { color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 12px; padding: 12px 0; }
  .qwv-grp {
    font-family: var(--v9-mono);
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--v9-ink-faint);
    margin-top: 16px;
    margin-bottom: 4px;
  }
  .qwv-grp-n {
    color: var(--v9-ink-faint);
    margin-left: 6px;
    opacity: 0.7;
  }
  .qwv-row {
    display: grid;
    grid-template-columns: 18px auto 1fr;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: none;
    background: transparent;
    color: var(--foreground);
    font: inherit;
    text-align: left;
    border-radius: 4px;
    width: 100%;
  }
  .qwv-row:hover { background: var(--v9-bg-2); }
  .qwv-row.selected { background: color-mix(in srgb, var(--primary) 12%, transparent); }
  .qwv-marker { color: var(--primary); font-family: var(--v9-mono); }
  .qwv-text { line-height: 1.5; }
  .qwv-text-btn {
    background: transparent;
    border: 0;
    padding: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    width: 100%;
    min-width: 0;
  }
  .qwv-status {
    background: transparent;
    border: 1px solid var(--v9-line);
    color: var(--v9-ink-faint);
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    line-height: 1;
    cursor: pointer;
    padding: 0;
  }
  .qwv-status:hover { color: var(--primary); border-color: var(--primary); }
  .qwv-src {
    font-family: var(--v9-mono);
    font-size: 10px;
    color: var(--v9-ink-faint);
    padding-left: 26px;
    margin-bottom: 4px;
  }
</style>
