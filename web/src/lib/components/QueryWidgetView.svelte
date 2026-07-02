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
  import { setFocusedBlock } from "$lib/stores/current-block.svelte";
  import {
    setBottomDrawerOpen,
    setActiveRegion,
    setBottomTab,
    getActiveRegion,
  } from "$lib/stores/pane-state.svelte";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import {
    applyTriage,
    attachToProject,
    triageActionForKey,
    setBlockText,
    deleteBlock as removeBlockFromContent,
  } from "$lib/triage.svelte";
  import ProjectPicker from "./ProjectPicker.svelte";
  import ViewSwitcher from "./ViewSwitcher.svelte";
  import KanbanBoard from "./KanbanBoard.svelte";
  import { IconTable, IconLayoutKanban } from "@tabler/icons-svelte";
  import { parseQuery } from "$lib/query-language";
  import { getViewMode, setViewMode } from "$lib/stores/tag-view-prefs.svelte";
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

  let {
    widget,
    onOpenRow,
  }: {
    widget: Widget;
    /** Prism v4 — when set, row activation routes here instead of the
     *  legacy `gotoNote` column-view drill. The v4 widget pane wires
     *  this to `jumpToTile` so a result opens in the focused editor
     *  pane. Unset for the legacy rail/route mount. */
    onOpenRow?: (pageId: string, blockId: string | null) => void;
  } = $props();
  const queryClient = useQueryClient();

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
   * board the type's own declared property order (decision 3c). `null`
   * when the query isn't tag-scoped (e.g. `kind:page note_type:Project`)
   * — kanban still renders (tesela-ya4.1/G2: the block source generalized
   * to `executeQuery`, so a non-tag-scoped view no longer silently falls
   * back to the table list).
   */
  const inferredKanbanTag: string | null = $derived.by(() => {
    if (!widget.query) return null;
    try {
      const parsed = parseQuery(widget.query);
      const tagFilter = parsed.filters.find((f) => f.key === "tag" && f.op === "Eq");
      return tagFilter ? tagFilter.value : null;
    } catch { return null; }
  });

  const showKanban = $derived(widgetView === "kanban");

  function handleViewChange(mode: string) {
    if (mode === "table" || mode === "kanban") setViewMode(widget.id, mode);
  }

  let projectPickerRow = $state<Row | null>(null);
  let selectedIndex = $state(0);
  let rootEl = $state<HTMLElement | undefined>();

  // Phase 9.9 — auto-focus the result list on mount so j/k works without
  // requiring the user to click first.
  $effect(() => {
    if (rootEl && document.activeElement !== rootEl) rootEl.focus();
  });

  // Phase 12.2 — when the active region flips back to "focus" (e.g. user hit
  // Escape from the BottomDrawer), restore DOM focus to this list. The
  // drawer's Escape handler only changes the region store; without this,
  // focus stays on whatever the drawer blurred and j/k don't reach our
  // div-bound keyhandler.
  $effect(() => {
    const region = getActiveRegion();
    if (region === "focus" && rootEl && document.activeElement !== rootEl) {
      rootEl.focus();
    }
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
      // P1.13 structured-first: `status` is a CONTAINER property — set it via
      // the block-granular endpoint (which strips any in-text `status::` line
      // and materializes exactly ONE line) instead of rewriting the markdown
      // with a whole-note PUT. The old PUT left the text `status::` alongside
      // the container's value → a duplicate `status::` line on materialize.
      await api.setBlockProperty(row.blockId, "status", next);
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

  // Raised 500→5000 (tesela-sclr.1): 500 silently hid notes past #500 from
  // the inbox widget's project picker.
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", "all-for-picker"] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
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
    if (onOpenRow) {
      onOpenRow(row.pageId, row.blockId ?? null);
      return;
    }
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

  // Phase 10.1 — in-place edit. `e` swaps the highlighted row's text into
  // an inline input; Enter saves via `setBlockText`+`updateNote`, Esc bails.
  // Saves invalidate both the widget query and the underlying note query so
  // the outliner picks up the new text without a full reload.
  let editingRowId = $state<string | null>(null);
  let editingValue = $state("");
  let editInputRef = $state<HTMLInputElement | undefined>();

  function startEditRow(row: Row): void {
    if (row.kind !== "block" || !row.blockId) return;
    editingRowId = row.id;
    editingValue = row.label;
    requestAnimationFrame(() => { editInputRef?.focus(); editInputRef?.select(); });
  }
  function cancelEditRow(): void {
    editingRowId = null;
    editingValue = "";
    rootEl?.focus();
  }
  async function commitEditRow(row: Row): Promise<void> {
    const newText = editingValue;
    editingRowId = null;
    editingValue = "";
    rootEl?.focus();
    if (!row.blockId || row.kind !== "block") return;
    if (newText === row.label) return;
    try {
      const note = await api.getNote(row.pageId);
      const updated = setBlockText(note.content, row.blockId, newText);
      if (updated === note.content) return;
      await api.updateNote(row.pageId, updated);
      queryClient.invalidateQueries({ queryKey: ["widget", widget.id] });
      queryClient.invalidateQueries({ queryKey: ["note", row.pageId] });
    } catch (err) {
      console.error("Edit row save failed:", err);
    }
  }

  // Phase 10.1 follow-up #4 — leader-style chord menu on highlighted row.
  // `/` opens the menu; each visible key is a single-letter chord that
  // directly runs an action (no arrow-nav, no filter typing). Mirrors the
  // emacs/spacemacs/neovim leader-key UX. Position is anchored under the
  // selected row so the user always knows which row the chord targets.
  type RowChord = { key: string; label: string; action: () => void };
  let chordOpen = $state(false);
  let chordPos = $state({ x: 0, y: 0 });

  async function deleteRow(row: Row): Promise<void> {
    if (!row.blockId || row.kind !== "block") return;
    if (!window.confirm(`Delete "${row.label}"? This cannot be undone.`)) return;
    try {
      const note = await api.getNote(row.pageId);
      const updated = removeBlockFromContent(note.content, row.blockId);
      if (updated === note.content) return;
      await api.updateNote(row.pageId, updated);
      queryClient.invalidateQueries({ queryKey: ["widget", widget.id] });
      queryClient.invalidateQueries({ queryKey: ["note", row.pageId] });
    } catch (err) {
      console.error("Delete row failed:", err);
    }
  }
  async function setRowStatus(row: Row, status: string): Promise<void> {
    if (!row.blockId || row.kind !== "block") return;
    try {
      // P1.13 structured-first: container property, not a whole-note text
      // rewrite (see cycleRowStatus). Empty value clears the property.
      if (status === "") {
        await api.clearBlockProperty(row.blockId, "status");
      } else {
        await api.setBlockProperty(row.blockId, "status", status);
      }
      queryClient.invalidateQueries({ queryKey: ["widget", widget.id] });
      queryClient.invalidateQueries({ queryKey: ["note", row.pageId] });
    } catch (err) {
      console.error("Set status failed:", err);
    }
  }

  function buildChords(row: Row): RowChord[] {
    if (row.kind === "block") {
      return [
        { key: "e", label: "Edit text",      action: () => startEditRow(row) },
        { key: "o", label: "Open in split",  action: () => openRow(row) },
        { key: "t", label: "Mark todo",      action: () => void setRowStatus(row, "todo") },
        { key: "i", label: "Mark doing",     action: () => void setRowStatus(row, "doing") },
        { key: "d", label: "Mark done",      action: () => void setRowStatus(row, "done") },
        { key: "b", label: "Mark backlog",   action: () => void setRowStatus(row, "backlog") },
        { key: "x", label: "Delete block",   action: () => void deleteRow(row) },
      ];
    }
    return [
      { key: "o", label: "Open in split",    action: () => openRow(row) },
    ];
  }

  function openChordAtRow(row: Row): void {
    const rowEl = rootEl?.querySelector(`[data-row-id="${CSS.escape(row.id)}"]`) as HTMLElement | null;
    const rect = rowEl?.getBoundingClientRect();
    chordPos = rect ? { x: rect.left + 24, y: rect.bottom + 4 } : { x: 200, y: 200 };
    chordOpen = true;
  }
  function closeChord(): void {
    chordOpen = false;
    rootEl?.focus();
  }

  // Phase 12.2 — push the currently-selected row to the drawer's focused-block
  // store on every nav step. Constructs a stub `ParsedBlock` with the row's id
  // + page id; the drawer's existing live-resolve path (`blockSourceNote`)
  // re-parses the source note to surface real properties, so this stub is just
  // the routing key.
  function rowToStub(row: Row): ParsedBlock | null {
    if (row.kind !== "block" || !row.blockId) return null;
    return {
      id: row.blockId,
      note_id: row.pageId,
      text: row.label,
      raw_text: row.label,
      tags: row.primaryTag ? [row.primaryTag] : [],
      inline_tags: [],
      trailing_tags: [],
      inherited_tags: [],
      properties: row.status ? { status: row.status } : {},
      indent_level: 0,
      parent_note_type: null,
    };
  }
  function syncSelectedRowToDrawer() {
    const row = flatRows[selectedIndex];
    setFocusedBlock(row ? rowToStub(row) : null);
  }

  function handleKeydown(e: KeyboardEvent) {
    // Phase 10.1 follow-up — when an inline rename input is active, all
    // keys belong to the input (Enter / Esc are handled there directly,
    // typing chars go via bind:value). Without this guard, the `e`
    // shortcut bubbles up from the input and re-runs `startEditRow` →
    // `editingValue = row.label` → the user's in-progress text reverts.
    if (editingRowId !== null) return;
    if (showKanban) return;
    if (chordOpen) {
      // Leader-chord menu: every typed key is either an action (matched by
      // its `key`) or Esc to close. No arrow nav, no filter — chords run
      // immediately, mirroring spacemacs/which-key.
      if (e.key === "Escape") {
        e.preventDefault();
        closeChord();
        return;
      }
      const row = flatRows[selectedIndex];
      if (!row) { closeChord(); return; }
      const match = buildChords(row).find((c) => c.key === e.key);
      if (match) {
        e.preventDefault();
        e.stopPropagation();
        chordOpen = false;
        match.action();
      } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey) {
        // Unknown chord key — swallow so it doesn't bubble into qwv nav.
        e.preventDefault();
      }
      return;
    }
    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(flatRows.length - 1, selectedIndex + 1);
      syncSelectedRowToDrawer();
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
      syncSelectedRowToDrawer();
    } else if (e.key === "Enter" && flatRows[selectedIndex]) {
      e.preventDefault();
      openRow(flatRows[selectedIndex]);
    } else if (e.key === "i" && flatRows[selectedIndex]?.kind === "block") {
      // Mirrors Kanban's `i` — push the selected block to the drawer and
      // focus it, so the user can edit properties without leaving the table.
      e.preventDefault();
      const stub = rowToStub(flatRows[selectedIndex]);
      if (!stub) return;
      setFocusedBlock(stub);
      setBottomDrawerOpen(true);
      setActiveRegion("bottom");
      setBottomTab({ kind: "fixed", id: "properties" });
    } else if (e.key === "s" && flatRows[selectedIndex]?.kind === "block") {
      // Phase 9.9 — `s` cycles the highlighted row's status without
      // leaving the result list.
      e.preventDefault();
      void cycleRowStatus(flatRows[selectedIndex]);
    } else if (e.key === "e" && flatRows[selectedIndex]?.kind === "block") {
      // Phase 10.1 — `e` opens in-place edit on the highlighted row.
      e.preventDefault();
      startEditRow(flatRows[selectedIndex]);
    } else if (e.key === "/" && flatRows[selectedIndex]) {
      // Phase 10.1 — `/` opens the leader-chord menu anchored to the row.
      e.preventDefault();
      openChordAtRow(flatRows[selectedIndex]);
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
  <div class="qwv-header">
    <div class="qwv-meta">{view.subtitle}</div>
    {#if widget.query.trim().length > 0}
      <ViewSwitcher
        views={[
          { id: "table",  label: "Table",  Icon: IconTable },
          { id: "kanban", label: "Kanban", Icon: IconLayoutKanban },
        ]}
        active={widgetView}
        onChange={handleViewChange}
      />
    {/if}
  </div>
  {#if showKanban}
    <!-- tesela-ya4.1 — generalized kanban block source (decision 2/G2):
         KanbanBoard is fully driven by the DSL now, tag-scoped or not.
         `viewId`/`displayGroupBy` only carry the saved-view override
         (decision 3a) when `widget.viewId` marks this as a saved-view
         mount (`GrInbox`'s `modeWidget`) — a plain Query-note widget's
         `group::` frontmatter is NOT treated as an override, matching the
         pre-ya4.1 tag-page kanban behavior. -->
    <KanbanBoard
      dsl={widget.query}
      tagName={inferredKanbanTag}
      viewId={widget.viewId ?? null}
      displayGroupBy={widget.viewId ? widget.group : null}
      groupByStorageKey={inferredKanbanTag ?? widget.id}
      focused={true}
    />
  {:else if view.error}
    <div class="qwv-error">Query error: {view.error}</div>
  {:else if view.groups}
    {#each view.groups as g}
      {#if g.rows.length > 0}
        <div class="qwv-grp">{g.key} <span class="qwv-grp-n">{g.rows.length}</span></div>
        {#each g.rows as row}
          {@const ri = flatRows.indexOf(row)}
          {@const sel = selectedIndex === ri}
          {@const editing = editingRowId === row.id}
          <div class="qwv-row {sel ? 'selected' : ''}" data-row-id={row.id}>
            <span class="qwv-marker">{sel ? "▸" : ""}</span>
            {#if row.kind === "block" && (row.status !== undefined || row.primaryTag === "Task")}
              <button
                class="qwv-status"
                type="button"
                title="Status: {row.status ?? '(none)'} — click to cycle"
                onclick={(e) => { e.stopPropagation(); selectedIndex = ri; void cycleRowStatus(row); }}
              >{statusGlyph(row.status)}</button>
            {/if}
            {#if editing}
              <input
                bind:this={editInputRef}
                class="qwv-edit-input"
                type="text"
                bind:value={editingValue}
                onkeydown={(e) => {
                  if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); void commitEditRow(row); }
                  else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); cancelEditRow(); }
                }}
                onblur={() => { if (editingRowId === row.id) void commitEditRow(row); }}
              />
            {:else}
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
            {/if}
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
      {@const editing = editingRowId === row.id}
      <div class="qwv-row {sel ? 'selected' : ''}" data-row-id={row.id}>
        <span class="qwv-marker">{sel ? "▸" : ""}</span>
        {#if editing}
          <input
            bind:this={editInputRef}
            class="qwv-edit-input"
            type="text"
            bind:value={editingValue}
            onkeydown={(e) => {
              if (e.key === "Enter") { e.preventDefault(); e.stopPropagation(); void commitEditRow(row); }
              else if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); cancelEditRow(); }
            }}
            onblur={() => { if (editingRowId === row.id) void commitEditRow(row); }}
          />
        {:else}
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
        {/if}
      </div>
      {#if row.breadcrumb && row.breadcrumb.length > 0}
        <div class="qwv-src">↳ {row.breadcrumb.join(" / ")}</div>
      {/if}
    {/each}
  {/if}
</div>

{#if chordOpen && flatRows[selectedIndex]}
  {@const chords = buildChords(flatRows[selectedIndex])}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="qwv-chord-overlay"
    onclick={closeChord}
  ></div>
  <div
    class="qwv-chord"
    style="left: {chordPos.x}px; top: {chordPos.y}px"
    role="menu"
  >
    <div class="qwv-chord-head">/</div>
    {#each chords as c (c.key)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="qwv-chord-row" onclick={() => { chordOpen = false; c.action(); }}>
        <kbd class="qwv-chord-key">{c.key}</kbd>
        <span class="qwv-chord-label">{c.label}</span>
      </div>
    {/each}
  </div>
{/if}

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
  .qwv-edit-input {
    flex: 1;
    background: var(--v9-bg-2);
    border: 1px solid var(--primary);
    color: var(--foreground);
    font: inherit;
    line-height: 1.5;
    padding: 2px 6px;
    border-radius: 3px;
    outline: none;
  }
  .qwv-chord-overlay {
    position: fixed;
    inset: 0;
    z-index: 49;
  }
  .qwv-chord {
    position: fixed;
    z-index: 50;
    min-width: 180px;
    background: var(--popover, var(--v9-bg-2));
    color: var(--popover-foreground, var(--foreground));
    border: 1px solid var(--border, var(--v9-line));
    border-radius: 6px;
    box-shadow: 0 6px 24px rgba(0, 0, 0, 0.3);
    padding: 4px;
    font-family: var(--v9-mono);
    font-size: 12px;
  }
  .qwv-chord-head {
    padding: 4px 8px 6px;
    font-size: 10px;
    text-transform: uppercase;
    color: var(--v9-ink-faint);
    letter-spacing: 0.08em;
    border-bottom: 1px solid var(--v9-line);
    margin-bottom: 4px;
  }
  .qwv-chord-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 4px 8px;
    border-radius: 4px;
    cursor: pointer;
  }
  .qwv-chord-row:hover { background: color-mix(in srgb, var(--primary) 10%, transparent); }
  .qwv-chord-key {
    display: inline-block;
    min-width: 18px;
    padding: 1px 5px;
    text-align: center;
    background: var(--v9-bg-3, color-mix(in srgb, var(--foreground) 8%, transparent));
    color: var(--primary);
    border: 1px solid var(--v9-line);
    border-radius: 3px;
    font-family: inherit;
    font-size: 11px;
    font-weight: 600;
  }
  .qwv-chord-label { color: var(--foreground); }
  .qwv-src {
    font-family: var(--v9-mono);
    font-size: 10px;
    color: var(--v9-ink-faint);
    padding-left: 26px;
    margin-bottom: 4px;
  }
</style>
