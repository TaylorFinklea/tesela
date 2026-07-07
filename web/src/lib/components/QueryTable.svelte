<script lang="ts">
  /**
   * tesela-ya4.3 — real TABLE ("Sets") render mode over the generalized
   * query result source (spec decisions 1/2, gap G4). Sibling of
   * `KanbanBoard.svelte`: same single block source (`executeQuery(dsl)`,
   * decision 2), same property registry / chip rendering, same
   * focus-gated command-registry bridge pattern (tesela-ya4.2's
   * `kanban-commands.ts` mirrored here as `table-commands.ts`).
   *
   * Columns resolve per `table-columns.ts`: a tag-scoped table (DSL has a
   * positive `tag:X`) uses the type's own declared property order; a
   * non-tag-scoped query uses the global properties actually present on
   * the returned blocks. Cells render via the same `DisplayChip` chip
   * system Kanban cards use, so a property looks the same in both display
   * modes. Supersedes `TagTable.svelte` (orphaned since the legacy `/p/[id]`
   * chrome was deleted — see the hard-swap commit `dda11a17`): this is now
   * the ONE table component.
   */
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { updateBlockProperty } from "$lib/property-update";
  import { queryItemToParsedBlock } from "$lib/query-item-adapt";
  import { resolveTableColumns, type TableColumnCandidate } from "$lib/table/table-columns";
  import { sortByColumn, type SortDirection } from "$lib/table/table-sort";
  import { clampTableCursor, moveTableCursor, type TableCursor } from "$lib/table/table-nav";
  import { setTableFocused } from "$lib/table/table-focus.svelte";
  import { buildRegistry } from "$lib/property-registry";
  import { setFocusedBlock } from "$lib/stores/current-block.svelte";
  import { setBottomDrawerOpen, setActiveRegion, setBottomTab, getActiveRegion } from "$lib/stores/pane-state.svelte";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { PropertyDef } from "$lib/types/PropertyDef";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";
  import type { Note } from "$lib/types/Note";
  import type { QueryItem } from "$lib/types/QueryItem";
  import PropertyEditor from "./PropertyEditor.svelte";
  import DisplayChip from "./DisplayChip.svelte";

  let {
    dsl,
    tagName = null,
    focused = false,
    onOpenRow,
  }: {
    /** DSL string this table's rows come from — the same generalized
     *  `executeQuery(dsl)` source KanbanBoard uses (decision 2). */
    dsl: string;
    /** Resolved tag name when the DSL is tag-scoped (first positive `tag:X`
     *  filter) — drives column resolution from the type's own declared
     *  property order (decision 3c parity). `null` for a non-tag-scoped
     *  query. */
    tagName?: string | null;
    focused?: boolean;
    /** When set, opening the focused row routes here instead of a full
     *  `goto` navigation — mirrors QueryWidgetView's own `onOpenRow`
     *  passthrough (the old row-list's drill behavior). */
    onOpenRow?: (pageId: string, blockId: string | null) => void;
  } = $props();

  const queryClient = useQueryClient();

  const typeQuery = createQuery(() => ({
    queryKey: ["type", tagName ?? ""] as const,
    queryFn: () => api.getType(tagName as string),
    enabled: !!tagName,
  }));
  const typeDef: TypeDefinition | undefined = $derived(typeQuery.data as TypeDefinition | undefined);

  // Single generalized block source (decision 2) — same shape/cache pattern
  // as KanbanBoard's `kanbanSourceQuery`, kept under its own key so editing
  // a block in one display mode doesn't stomp the other mode's cache.
  const tableQueryKey = $derived(["table-source", dsl] as const);
  const tableSourceQuery = createQuery(() => ({
    queryKey: tableQueryKey,
    queryFn: () => api.executeQuery(dsl, null, null),
    enabled: dsl.trim().length > 0,
  }));

  const blocks: ParsedBlock[] = $derived.by(() => {
    const result = tableSourceQuery.data;
    if (!result) return [];
    return result.groups
      .flatMap((g) => g.items)
      .filter((item): item is QueryItem & { block_id: string } => item.block_id !== null)
      .map(queryItemToParsedBlock);
  });

  // Property registry powers cell chip rendering (same registry Kanban
  // cards use, so a property looks identical in both display modes).
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  const propertyRegistry = $derived(buildRegistry((allNotesQuery.data ?? []) as Note[]));

  // Global property registry — the column candidate source for a
  // non-tag-scoped query (no type to enumerate).
  const propertiesQuery = createQuery(() => ({
    queryKey: ["properties"] as const,
    queryFn: () => api.listProperties(),
  }));
  const globalProperties = $derived((propertiesQuery.data ?? []) as PropertyDef[]);

  const presentKeys = $derived.by(() => {
    const s = new Set<string>();
    for (const b of blocks) for (const k of Object.keys(b.properties)) s.add(k.toLowerCase());
    return s;
  });

  // Column resolution (table-columns.ts) — tag-scoped: the type's own
  // declared property order; non-tag-scoped: global properties present on
  // the returned blocks.
  const columns: TableColumnCandidate[] = $derived(
    resolveTableColumns({
      tagName,
      typeProperties: typeDef?.properties ?? [],
      globalProperties,
      presentKeys,
    }),
  );

  function getPropertyValue(block: ParsedBlock, propName: string): string {
    return block.properties[propName] ?? block.properties[propName.toLowerCase()] ?? "";
  }

  // ── sort ──────────────────────────────────────────────────────────────
  let sortColumnName = $state<string | null>(null);
  let sortDirection = $state<SortDirection>("asc");
  const sortColumn = $derived(columns.find((c) => c.name === sortColumnName) ?? null);
  const sortedBlocks = $derived(
    sortColumn ? sortByColumn(blocks, (b) => getPropertyValue(b, sortColumn.name), sortColumn, sortDirection) : blocks,
  );

  function toggleSortColumn(name: string): void {
    if (sortColumnName === name) {
      sortDirection = sortDirection === "asc" ? "desc" : "asc";
    } else {
      sortColumnName = name;
      sortDirection = "asc";
    }
  }

  // ── keyboard cursor — row AND column nav (acceptance) ───────────────────
  // Column 0 is the fixed "Block" label column; columns 1..N are the
  // resolved property columns (table-nav.ts's contract).
  let cursor = $state<TableCursor>({ row: 0, col: 0 });
  const rowCount = $derived(sortedBlocks.length);
  const colCount = $derived(columns.length + 1);
  const focusedCell = $derived(clampTableCursor(cursor, rowCount, colCount));
  const focusedBlockRow = $derived(sortedBlocks[focusedCell.row] ?? null);

  function syncFocusedRowToDrawer(): void {
    setFocusedBlock(focusedBlockRow);
  }

  function moveCursor(step: Parameters<typeof moveTableCursor>[1]): void {
    cursor = moveTableCursor(focusedCell, step, rowCount, colCount);
    syncFocusedRowToDrawer();
  }

  function openFocusedRow(): void {
    if (!focusedBlockRow) return;
    if (onOpenRow) {
      onOpenRow(focusedBlockRow.note_id, focusedBlockRow.id);
      return;
    }
    goto(`/p/${encodeURIComponent(focusedBlockRow.note_id)}`);
  }

  function openPropertiesDrawer(): void {
    if (!focusedBlockRow) return;
    setFocusedBlock(focusedBlockRow);
    setBottomDrawerOpen(true);
    setActiveRegion("bottom");
    setBottomTab({ kind: "fixed", id: "properties" });
  }

  function sortFocusedColumn(): void {
    if (focusedCell.col === 0) return; // label column isn't a typed property
    const col = columns[focusedCell.col - 1];
    if (col) toggleSortColumn(col.name);
  }

  // ── cell editing (typed, per value_type — mirrors TagTable's popover) ──
  let editingBlock = $state<ParsedBlock | null>(null);
  let editingColumn = $state<TableColumnCandidate | null>(null);
  let editorPosition = $state({ x: 0, y: 0 });

  function getEffectiveChoices(col: TableColumnCandidate): string[] | null {
    const def = propertyRegistry.get(col.name.toLowerCase());
    if (def?.choices?.length) return def.choices;
    return col.values ?? null;
  }

  function openCellEditor(block: ParsedBlock, col: TableColumnCandidate, event: MouseEvent): void {
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    editorPosition = { x: rect.left, y: rect.bottom + 2 };
    editingBlock = block;
    editingColumn = col;
  }

  function editFocusedCell(): void {
    if (focusedCell.col === 0) return; // no typed property to edit on the label column
    const block = focusedBlockRow;
    const col = columns[focusedCell.col - 1];
    if (!block || !col) return;
    const el = document.querySelector("[data-table-cell-focused='true']") as HTMLElement | null;
    const rect = el?.getBoundingClientRect();
    editorPosition = rect ? { x: rect.left, y: rect.bottom + 2 } : { x: 200, y: 200 };
    editingBlock = block;
    editingColumn = col;
  }

  async function handleCellEdit(value: string): Promise<void> {
    if (!editingBlock || !editingColumn) return;
    const block = editingBlock;
    const col = editingColumn;
    editingBlock = null;
    editingColumn = null;
    try {
      await updateBlockProperty({ block, propKey: col.name, value, queryKey: tableQueryKey, queryClient });
    } catch (e) {
      console.error("Failed to update cell:", e);
    }
  }

  function closeCellEditor(): void {
    editingBlock = null;
    editingColumn = null;
  }

  // ── command-registry dispatch bridge (mirrors KanbanBoard/ya4.2) ────────
  function routeTableCommand(id: string): void {
    switch (id) {
      case "table.focus-down": moveCursor("down"); break;
      case "table.focus-up": moveCursor("up"); break;
      case "table.focus-left": moveCursor("left"); break;
      case "table.focus-right": moveCursor("right"); break;
      case "table.focus-first-row": moveCursor("first-row"); break;
      case "table.focus-last-row": moveCursor("last-row"); break;
      case "table.focus-first-col": moveCursor("first-col"); break;
      case "table.focus-last-col": moveCursor("last-col"); break;
      case "table.open-row": openFocusedRow(); break;
      case "table.edit-cell": editFocusedCell(); break;
      case "table.edit-properties": openPropertiesDrawer(); break;
      case "table.sort-column": sortFocusedColumn(); break;
    }
  }

  function handleTableKeydown(e: KeyboardEvent): void {
    if (!focused) return;
    // Region gate — mirrors KanbanBoard: when focus has moved to the
    // drawer/rail, those panes own the keys.
    if (getActiveRegion() !== "focus") return;
    if (editingBlock) return; // the cell-editor popover handles its own keys

    const target = e.target;
    if (target instanceof HTMLElement) {
      const isEditing =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.tagName === "SELECT" ||
        target.isContentEditable ||
        target.closest(".cm-editor") !== null;
      if (isEditing) return;
    }

    switch (e.key) {
      case "j": e.preventDefault(); moveCursor("down"); break;
      case "k": e.preventDefault(); moveCursor("up"); break;
      case "h": e.preventDefault(); moveCursor("left"); break;
      case "l": e.preventDefault(); moveCursor("right"); break;
      case "g": e.preventDefault(); moveCursor("first-row"); break;
      case "G": e.preventDefault(); moveCursor("last-row"); break;
      case "0": e.preventDefault(); moveCursor("first-col"); break;
      case "$": e.preventDefault(); moveCursor("last-col"); break;
      case "Enter": e.preventDefault(); openFocusedRow(); break;
      case "e": e.preventDefault(); editFocusedCell(); break;
      case "i": e.preventDefault(); openPropertiesDrawer(); break;
      case "s": e.preventDefault(); sortFocusedColumn(); break;
    }
  }

  // Publish table-focus to the command registry's `when` predicates —
  // mirrors kanban's setKanbanFocused effect.
  $effect(() => {
    if (!focused) return;
    setTableFocused(true);
    return () => setTableFocused(false);
  });

  // Command-registry dispatch bridge — mirrors kanban's listener effect.
  $effect(() => {
    if (!focused) return;
    function onRun(e: Event): void {
      const id = (e as CustomEvent<{ id?: string }>).detail?.id;
      if (id) routeTableCommand(id);
    }
    document.addEventListener("tesela:run-table-command", onRun);
    return () => document.removeEventListener("tesela:run-table-command", onRun);
  });

  // Scroll the focused cell into view on every nav step.
  $effect(() => {
    if (!focused) return;
    const _r = focusedCell.row;
    const _c = focusedCell.col;
    requestAnimationFrame(() => {
      const cell = document.querySelector("[data-table-cell-focused='true']");
      cell?.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
    });
  });
</script>

<svelte:window onkeydown={handleTableKeydown} />

{#if tableSourceQuery.isLoading || (tagName && typeQuery.isLoading)}
  <div class="text-[12px] text-muted-foreground py-4">Loading…</div>
{:else if blocks.length === 0}
  <div class="text-[12px] text-muted-foreground py-4 italic">No blocks match this query</div>
{:else}
  <div class="flex items-center gap-2 mb-2 px-1">
    <span class="flex-1"></span>
    <span class="text-[10px]" style="color: color-mix(in srgb, var(--muted-foreground) 50%, transparent)">
      {blocks.length} blocks
    </span>
  </div>

  <div class="overflow-x-auto">
    <table class="w-full text-[12px]">
      <thead>
        <tr class="border-b" style="border-color: var(--border)">
          <th class="text-left px-3 py-1.5 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Block</th>
          {#each columns as col, ci}
            <th
              class="text-left px-3 py-1.5 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest cursor-pointer hover:text-foreground select-none"
              class:text-primary={focused && focusedCell.col === ci + 1}
              onclick={() => { cursor = { row: focusedCell.row, col: ci + 1 }; toggleSortColumn(col.name); }}
            >
              {col.name}
              {#if sortColumnName === col.name}
                <span class="ml-0.5">{sortDirection === "asc" ? "↑" : "↓"}</span>
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each sortedBlocks as block, ri (block.id)}
          {@const isRowFocused = focused && ri === focusedCell.row}
          <tr class="border-b transition-colors" style="border-color: color-mix(in srgb, var(--border) 30%, transparent)" class:bg-accent={isRowFocused}>
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <td
              data-table-cell-focused={isRowFocused && focusedCell.col === 0 ? "true" : undefined}
              class="px-3 py-1.5"
              class:ring-2={isRowFocused && focusedCell.col === 0}
              onclick={() => { cursor = { row: ri, col: 0 }; }}
            >
              <a href="/p/{encodeURIComponent(block.note_id)}" class="hover:underline">
                {block.text || "(empty)"}
              </a>
              <div class="text-[10px] text-muted-foreground/50">{block.note_id}</div>
            </td>
            {#each columns as col, ci}
              {@const val = getPropertyValue(block, col.name)}
              {@const def = propertyRegistry.get(col.name.toLowerCase())}
              {@const isCellFocused = isRowFocused && focusedCell.col === ci + 1}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <td
                data-table-cell-focused={isCellFocused ? "true" : undefined}
                class="px-3 py-1.5 cursor-pointer hover:bg-accent/30 rounded transition-colors"
                class:ring-2={isCellFocused}
                onclick={(e) => { cursor = { row: ri, col: ci + 1 }; openCellEditor(block, col, e); }}
              >
                {#if val && def}
                  <DisplayChip propKey={col.name.toLowerCase()} value={val} {def} />
                {:else if val}
                  <span class="text-muted-foreground">{val}</span>
                {:else}
                  <span class="text-muted-foreground/30">—</span>
                {/if}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if editingBlock && editingColumn}
    <PropertyEditor
      propertyName={editingColumn.name}
      currentValue={getPropertyValue(editingBlock, editingColumn.name)}
      valueType={editingColumn.value_type}
      choices={getEffectiveChoices(editingColumn)}
      position={editorPosition}
      onselect={handleCellEdit}
      onclose={closeCellEditor}
    />
  {/if}
{/if}
