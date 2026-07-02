<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { goto } from "$app/navigation";
  import { updateBlockProperty, clearBlockProperty } from "$lib/property-update";
  import { getGroupByProp, setGroupByProp } from "$lib/stores/tag-view-prefs.svelte";
  import { resolveKanbanGroupBy, isSelectWithChoices } from "$lib/kanban-group-by";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { QueryItem } from "$lib/types/QueryItem";
  import type { Note } from "$lib/types/Note";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";
  import type { PropertyDef } from "$lib/types/PropertyDef";
  import { buildRegistry } from "$lib/property-registry";
  import { setFocusedBlock } from "$lib/stores/current-block.svelte";
  import { setBottomDrawerOpen, setActiveRegion, setBottomTab, getActiveRegion } from "$lib/stores/pane-state.svelte";
  import KanbanCard from "./KanbanCard.svelte";
  import KanbanColumnPicker from "./KanbanColumnPicker.svelte";

  let {
    dsl,
    tagName = null,
    viewId = null,
    displayGroupBy = null,
    groupByStorageKey,
    focused = false,
  }: {
    /** tesela-ya4.1 — DSL string this board's cards come from. The single,
     *  generalized block source (spec decision 2/G3): `executeQuery(dsl)`,
     *  grouped client-side. `Kind` defaults to `block` in the parser, so a
     *  bare `tag:X` and an arbitrary non-tag-scoped filter both work. */
    dsl: string;
    /** Resolved tag name when the DSL is tag-scoped (first positive `tag:X`
     *  filter) — drives the type's own declared property order for the
     *  group-by candidate list (decision 3c) and legacy tag-keyed
     *  localStorage. `null` for a non-tag-scoped query (G2). */
    tagName?: string | null;
    /** Non-null when this board renders a saved view (`ViewRecord.id`) —
     *  group-by changes persist via `updateView` instead of localStorage
     *  (decision 4/5). */
    viewId?: string | null;
    /** The saved view's `display_group_by` — highest-priority group-by
     *  resolution (decision 3a). `null` outside a saved-view context. */
    displayGroupBy?: string | null;
    /** localStorage key for the per-surface group-by pref (decision 3b) —
     *  `tagName` for tag-scoped boards (preserves existing tag-page prefs),
     *  the widget id otherwise. */
    groupByStorageKey: string;
    focused?: boolean;
  } = $props();

  const queryClient = useQueryClient();

  // Only fetched when the DSL is tag-scoped — the type's own declared
  // property list gives the (c) candidate order tag-page kanban has always
  // used. A non-tag-scoped query has no single type to enumerate.
  const typeQuery = createQuery(() => ({
    queryKey: ["type", tagName ?? ""] as const,
    queryFn: () => api.getType(tagName as string),
    enabled: !!tagName,
  }));

  // tesela-ya4.1 — single generalized block source (spec decision 2):
  // executeQuery(dsl), ungrouped (group=null yields one flat bucket, see
  // `apply_group`), grouped into kanban columns client-side below. Replaces
  // `getTypedBlocks`, which (a) only works for tag-scoped boards (G2) and
  // (b) is NOT membership-equivalent to `executeQuery("tag:X kind:block")`
  // — a block nested under a tagged parent is included by the `tag:`
  // predicate's inherited-tags chain but excluded by `getTypedBlocks`'s
  // direct-tags-only check (proven divergent by
  // `crates/tesela-core/tests/typed_blocks_query_equivalence.rs`).
  const kanbanQueryKey = $derived(["kanban-source", dsl] as const);
  const kanbanSourceQuery = createQuery(() => ({
    queryKey: kanbanQueryKey,
    queryFn: () => api.executeQuery(dsl, null, null),
    enabled: dsl.trim().length > 0,
  }));

  /** Adapt one flat `executeQuery` row into the `ParsedBlock` shape the
   *  board/card/move machinery already speaks. `QueryItem` has no `bid` —
   *  writes fall back to the line-addressed `block_id`, same as
   *  QueryWidgetView's list/table rows already do (they never had a bid
   *  either). Callers filter out page-kind rows (`block_id === null`)
   *  before mapping — kanban only shows blocks. */
  function queryItemToParsedBlock(item: QueryItem): ParsedBlock {
    return {
      id: item.block_id as string,
      bid: null,
      text: item.text,
      raw_text: item.text,
      tags: item.primary_tag ? [item.primary_tag] : [],
      inline_tags: [],
      trailing_tags: [],
      inherited_tags: [],
      properties: item.properties,
      indent_level: 0,
      note_id: item.page_id,
      parent_note_type: item.page_note_type,
    };
  }

  // Phase 11 — property registry powers card chip rendering. Reuses the
  // same buildRegistry that BlockOutliner uses inline so cards inherit any
  // chip_icon / chip_value_format config from the Property pages.
  // Raised 500→5000 (tesela-sclr.1): a 500 cap silently dropped notes past
  // #500 from the property registry, so their chip config never applied.
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  const propertyRegistry = $derived(buildRegistry((allNotesQuery.data ?? []) as Note[]));

  const typeDef: TypeDefinition | undefined = $derived(typeQuery.data as TypeDefinition | undefined);
  const blocks: ParsedBlock[] = $derived.by(() => {
    const result = kanbanSourceQuery.data;
    if (!result) return [];
    return result.groups
      .flatMap((g) => g.items)
      .filter((item): item is QueryItem & { block_id: string } => item.block_id !== null)
      .map(queryItemToParsedBlock);
  });

  // Global property registry — the group-by candidate source for
  // non-tag-scoped queries (no type to enumerate), and the fallback lookup
  // for resolving an explicit override that isn't among a tag's own
  // declared properties.
  const propertiesQuery = createQuery(() => ({
    queryKey: ["properties"] as const,
    queryFn: () => api.listProperties(),
  }));
  const globalProperties = $derived((propertiesQuery.data ?? []) as PropertyDef[]);

  // Group-by candidates for decision-3(c) "first select property with ≥1
  // choice": a tag-scoped board uses the TYPE's own declared property order
  // (existing tag-page behavior, unchanged). A non-tag-scoped query has no
  // type to enumerate, so candidates are the global select properties that
  // actually appear on ≥1 returned block — an irrelevant global default
  // would be worse than the honest empty state (decision 3d).
  const selectProperties = $derived.by(() => {
    if (tagName) return (typeDef?.properties ?? []).filter(isSelectWithChoices);
    const present = new Set<string>();
    for (const b of blocks) for (const k of Object.keys(b.properties)) present.add(k.toLowerCase());
    return globalProperties.filter((p) => isSelectWithChoices(p) && present.has(p.name.toLowerCase()));
  });

  /** Resolve ANY property name (not just a `selectProperties` candidate) to
   *  its select-type def — an explicit `displayGroupBy` or stored pref must
   *  be honored even when no currently-visible block carries that property
   *  (decisions 3a/3b outrank "does the data have it"). */
  function resolvePropDef(name: string): (PropertyDef & { values: string[] }) | undefined {
    const fromType = tagName ? (typeDef?.properties ?? []).find((p) => p.name === name) : undefined;
    const def = fromType ?? globalProperties.find((p) => p.name === name);
    return def && isSelectWithChoices(def) ? (def as PropertyDef & { values: string[] }) : undefined;
  }

  // Group-by resolution (spec decision 3, locked order) — the pure
  // resolution logic lives in `kanban-group-by.ts` so the acceptance
  // contract (a > b > c > d) is unit-testable without mounting this
  // component. (d) honest empty state is handled by the render branch
  // below via `!groupByPropName`.
  //
  // (b) is the TAG PAGE's own localStorage pref (decision 3b says
  // "per-surface localStorage pref (tag page)"; decision 4 confirms a
  // saved-view board never WRITES there — `handleGroupByChange` below only
  // calls `setGroupByProp` when `viewId` is unset). `groupByStorageKey`
  // collapses to the bare tag name for any tag-scoped board, so a saved
  // view scoped to the same tag as a tag-page/plain-widget board would
  // otherwise read that OTHER surface's pref here. Gate the read the same
  // way the write is already gated: a saved view (`viewId` set) skips (b)
  // entirely and falls through straight to (c).
  const groupByPropName = $derived(
    resolveKanbanGroupBy({
      displayGroupBy,
      storedPref: viewId ? null : getGroupByProp(groupByStorageKey) || null,
      candidates: selectProperties,
      resolveDef: resolvePropDef,
    }),
  );

  const groupByDef = $derived(groupByPropName ? resolvePropDef(groupByPropName) : undefined);

  // Options for the group-by <select>: the (c) candidate list, plus the
  // currently-resolved property when an (a)/(b) override picked something
  // outside that list — keeps the dropdown's selection consistent with
  // what's actually rendered.
  const groupByOptions = $derived.by(() => {
    const opts = [...selectProperties];
    if (groupByPropName && groupByDef && !opts.some((p) => p.name === groupByPropName)) {
      opts.unshift(groupByDef);
    }
    return opts;
  });

  /** Persist a group-by change per decision 4/5: a saved-view board
   *  (`viewId` set) round-trips through `updateView` so `display_group_by`
   *  is round-trip-authoritative; a tag-page / plain-widget board keeps the
   *  existing localStorage pref. */
  async function handleGroupByChange(newProp: string) {
    if (viewId) {
      try {
        await api.updateView(viewId, { display_group_by: newProp });
        queryClient.invalidateQueries({ queryKey: ["views"] });
      } catch (err) {
        console.error("Failed to persist saved-view group-by:", err);
      }
    } else {
      setGroupByProp(groupByStorageKey, newProp);
    }
  }

  // Column names: Unset first, then canonical order from PropertyDef.values
  const columnNames = $derived(["__unset__", ...(groupByDef?.values ?? [])]);

  // Group blocks into columns
  const groupedBlocks = $derived.by(() => {
    const map = new Map<string, ParsedBlock[]>();
    for (const col of columnNames) map.set(col, []);

    for (const block of blocks) {
      const val = block.properties[groupByPropName] ?? block.properties[groupByPropName.toLowerCase()] ?? "";
      const col = val === "" ? "__unset__" : val;
      const list = map.get(col);
      if (list) list.push(block);
      else map.get("__unset__")!.push(block); // unknown value goes to unset
    }
    return map;
  });

  // DnD state
  let draggedBlockId = $state<string | null>(null);
  let dragOverColumn = $state<string | null>(null);

  function handleCardDragStart(e: DragEvent, block: ParsedBlock) {
    if (!e.dataTransfer) return;
    e.dataTransfer.setData("text/plain", block.id);
    e.dataTransfer.effectAllowed = "move";
    draggedBlockId = block.id;
  }

  function handleColumnDragOver(e: DragEvent, column: string) {
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    dragOverColumn = column;
  }

  function handleColumnDragLeave() {
    dragOverColumn = null;
  }

  async function handleColumnDrop(e: DragEvent, column: string) {
    e.preventDefault();
    dragOverColumn = null;
    draggedBlockId = null;

    const blockId = e.dataTransfer?.getData("text/plain");
    if (!blockId || !groupByPropName) return;

    const block = blocks.find((b) => b.id === blockId);
    if (!block) return;

    // Check if already in this column
    const currentVal = block.properties[groupByPropName] ?? block.properties[groupByPropName.toLowerCase()] ?? "";
    const targetVal = column === "__unset__" ? "" : column;
    if (currentVal === targetVal) return;

    try {
      if (column === "__unset__") {
        await clearBlockProperty({ block, propKey: groupByPropName, queryKey: kanbanQueryKey, queryClient });
      } else {
        await updateBlockProperty({ block, propKey: groupByPropName, value: column, queryKey: kanbanQueryKey, queryClient });
      }
    } catch (err) {
      console.error("Failed to move card:", err);
    }
  }

  function handleDragEnd() {
    draggedBlockId = null;
    dragOverColumn = null;
  }

  // Move picker (triggered by hover button on card)
  let movePickerBlock = $state<ParsedBlock | null>(null);
  let movePickerPosition = $state({ x: 0, y: 0 });

  function handleMoveRequest(block: ParsedBlock, event?: MouseEvent) {
    if (event) {
      const target = event.currentTarget as HTMLElement;
      const rect = target.getBoundingClientRect();
      movePickerPosition = { x: rect.right + 4, y: rect.top };
    }
    movePickerBlock = block;
  }

  async function handleMovePick(column: string) {
    if (!movePickerBlock || !groupByPropName) return;
    const block = movePickerBlock;
    movePickerBlock = null;

    try {
      if (column === "__unset__") {
        await clearBlockProperty({ block, propKey: groupByPropName, queryKey: kanbanQueryKey, queryClient });
      } else {
        await updateBlockProperty({ block, propKey: groupByPropName, value: column, queryKey: kanbanQueryKey, queryClient });
      }
    } catch (err) {
      console.error("Failed to move card:", err);
    }
  }

  function columnLabel(col: string): string {
    return col === "__unset__" ? "Unset" : col;
  }

  // Keyboard navigation (active when focused)
  let focusedColIndex = $state(0);
  let focusedCardIndex = $state(0);

  function clampCardIndex() {
    const cards = groupedBlocks.get(columnNames[focusedColIndex]) ?? [];
    focusedCardIndex = Math.min(focusedCardIndex, Math.max(0, cards.length - 1));
  }

  // Shift+H/L move helper. Drops the focused card into the column at
  // `targetIdx` and parks `pendingFocusBlockId` so the effect below can
  // resolve the card's actual landing index once the kanban source query
  // refetches. Block order in a column follows source-file order (not
  // append-to-end), so we cannot guess the index up-front.
  let pendingFocusBlockId = $state<string | null>(null);

  async function moveFocusedCardToColumn(block: ParsedBlock, targetIdx: number) {
    if (!groupByPropName) return;
    const targetCol = columnNames[targetIdx];
    pendingFocusBlockId = block.id;
    focusedColIndex = targetIdx;
    try {
      if (targetCol === "__unset__") {
        await clearBlockProperty({ block, propKey: groupByPropName, queryKey: kanbanQueryKey, queryClient });
      } else {
        await updateBlockProperty({ block, propKey: groupByPropName, value: targetCol, queryKey: kanbanQueryKey, queryClient });
      }
    } catch (err) {
      console.error("Failed to move card:", err);
      pendingFocusBlockId = null;
    }
  }

  // Resolve the cursor onto the moved card once the refetch surfaces its
  // new position. Re-runs whenever groupedBlocks changes; clears the
  // pending id once the card is found so we don't keep pinning focus.
  $effect(() => {
    if (!pendingFocusBlockId) return;
    const cards = groupedBlocks.get(columnNames[focusedColIndex]) ?? [];
    const idx = cards.findIndex((c) => c.id === pendingFocusBlockId);
    if (idx >= 0) {
      focusedCardIndex = idx;
      pendingFocusBlockId = null;
    }
  });

  // Phase 12.2 — push the currently-focused card to the drawer's focused-block
  // store on every grid-nav step, so the drawer (when open) tracks the highlight
  // without requiring a separate `i` press. Mirrors BlockOutliner's
  // `onfocusedblockchange` behavior.
  function syncFocusedCardToDrawer() {
    const cardsAtCol = groupedBlocks.get(columnNames[focusedColIndex]) ?? [];
    const card = cardsAtCol[focusedCardIndex] ?? null;
    setFocusedBlock(card);
  }

  function handleKanbanKeydown(e: KeyboardEvent) {
    if (!focused) return;
    // Region gate: when focus has moved to the drawer (`bottom`) or rail,
    // those panes own the keys. Without this, j/k/Enter etc. fire here at
    // the same time as the drawer's handler, causing double-actions like
    // drilling into a card while trying to commit a property edit.
    if (getActiveRegion() !== "focus") return;
    if (movePickerBlock) return; // picker handles its own keys

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

    const cols = columnNames;
    const currentCards = groupedBlocks.get(cols[focusedColIndex]) ?? [];

    switch (e.key) {
      case "j":
        e.preventDefault();
        focusedCardIndex = Math.min(Math.max(0, currentCards.length - 1), focusedCardIndex + 1);
        syncFocusedCardToDrawer();
        break;
      case "k":
        e.preventDefault();
        focusedCardIndex = Math.max(0, focusedCardIndex - 1);
        syncFocusedCardToDrawer();
        break;
      case "h":
        e.preventDefault();
        focusedColIndex = Math.max(0, focusedColIndex - 1);
        clampCardIndex();
        syncFocusedCardToDrawer();
        break;
      case "l":
        e.preventDefault();
        focusedColIndex = Math.min(cols.length - 1, focusedColIndex + 1);
        clampCardIndex();
        syncFocusedCardToDrawer();
        break;
      case "G":
        e.preventDefault();
        focusedCardIndex = Math.max(0, currentCards.length - 1);
        syncFocusedCardToDrawer();
        break;
      case "g":
        e.preventDefault();
        focusedCardIndex = 0;
        syncFocusedCardToDrawer();
        break;
      case "Enter": {
        e.preventDefault();
        const card = currentCards[focusedCardIndex];
        if (card) goto(`/p/${encodeURIComponent(card.note_id)}`);
        break;
      }
      case "m": {
        e.preventDefault();
        const block = currentCards[focusedCardIndex];
        if (block) {
          // Position picker next to the focused card
          const el = document.querySelector("[data-kanban-focused='true']") as HTMLElement | null;
          if (el) {
            const rect = el.getBoundingClientRect();
            movePickerPosition = { x: rect.right + 4, y: rect.top };
          }
          movePickerBlock = block;
        }
        break;
      }
      case "H": {
        // Shift+H: move focused card to previous column.
        e.preventDefault();
        const card = currentCards[focusedCardIndex];
        if (card && focusedColIndex > 0) void moveFocusedCardToColumn(card, focusedColIndex - 1);
        break;
      }
      case "L": {
        // Shift+L: move focused card to next column.
        e.preventDefault();
        const card = currentCards[focusedCardIndex];
        if (card && focusedColIndex < cols.length - 1) void moveFocusedCardToColumn(card, focusedColIndex + 1);
        break;
      }
      case "i": {
        // Open BottomDrawer for the focused card so the user can edit its
        // properties (deadline, priority, status, …) without leaving the
        // board. The drawer is a singleton fed by `current-block.svelte`,
        // so the same flow as BlockOutliner's `onfocusedblockchange`.
        e.preventDefault();
        const card = currentCards[focusedCardIndex];
        if (!card) break;
        setFocusedBlock(card);
        setBottomDrawerOpen(true);
        setActiveRegion("bottom");
        setBottomTab({ kind: "fixed", id: "properties" });
        break;
      }
    }
  }

  // Scroll focused card (or column, when the column has no cards) into view
  $effect(() => {
    if (!focused) return;
    // Read reactive dependencies
    const _c = focusedColIndex;
    const _r = focusedCardIndex;
    requestAnimationFrame(() => {
      const card = document.querySelector("[data-kanban-focused='true']");
      const column = document.querySelector("[data-kanban-col-focused='true']");
      // Horizontal scroll: always scroll the column into view (for h/l column nav)
      if (column) column.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
      // Vertical scroll: focused card into view within its column
      if (card) card.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
    });
  });
</script>

<svelte:window onkeydown={handleKanbanKeydown} />

{#if kanbanSourceQuery.isLoading || (tagName && typeQuery.isLoading)}
  <div class="text-[12px] text-muted-foreground py-4">Loading...</div>
{:else if !groupByPropName}
  <!-- Decision 3(d) — honest empty state. Never silently fall back to a
       list under a kanban toggle: if nothing groupable was found (no
       explicit display_group_by, no stored pref, no select property with
       choices), say so instead of pretending the board is empty. -->
  <div class="text-[12px] text-muted-foreground py-4 italic">
    No groupable select property found for this view. Add a select property
    with choices, or set a group-by on this view.
  </div>
{:else}
  <!-- Group-by picker -->
  <div class="flex items-center gap-2 mb-3 px-1">
    <span class="text-[10px] text-muted-foreground/60 uppercase tracking-widest">Group by</span>
    <select
      value={groupByPropName}
      onchange={(e) => void handleGroupByChange((e.target as HTMLSelectElement).value)}
      class="text-[11px] px-2 py-0.5 rounded-md border transition-colors outline-none"
      style="background: var(--surface); border-color: var(--border); color: var(--foreground)"
    >
      {#each groupByOptions as prop}
        <option value={prop.name}>{prop.name}</option>
      {/each}
    </select>
    <span class="flex-1"></span>
    <span class="text-[10px]" style="color: color-mix(in srgb, var(--muted-foreground) 50%, transparent)">
      {blocks.length} blocks
    </span>
  </div>

  <!-- Columns -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="flex gap-3 overflow-x-auto pb-4 px-1" ondragend={handleDragEnd}>
    {#each columnNames as column, colIdx (column)}
      {@const columnBlocks = groupedBlocks.get(column) ?? []}
      {@const isUnset = column === "__unset__"}
      {@const isDragOver = dragOverColumn === column}
      {@const isColumnFocused = focused && colIdx === focusedColIndex}
      <div
        data-kanban-col-focused={isColumnFocused ? "true" : undefined}
        class="flex-shrink-0 w-64 min-w-[256px] flex flex-col rounded-lg transition-all"
        class:ring-2={isDragOver || isColumnFocused}
        style="
          background: color-mix(in srgb, var(--surface) 50%, transparent);
          {isDragOver ? `ring-color: color-mix(in srgb, var(--primary) 30%, transparent); background: color-mix(in srgb, var(--primary) 5%, transparent)` : ''}
          {isColumnFocused && !isDragOver ? `ring-color: color-mix(in srgb, var(--primary) 25%, transparent)` : ''}
        "
        ondragover={(e) => handleColumnDragOver(e, column)}
        ondragleave={handleColumnDragLeave}
        ondrop={(e) => handleColumnDrop(e, column)}
      >
        <!-- Column header -->
        <div
          class="flex items-center gap-2 px-3 py-2 rounded-t-lg {isUnset ? 'border-dashed' : ''}"
          style="border-bottom: 1px solid var(--border)"
        >
          <span
            class="text-[11px] font-medium {isUnset ? 'italic' : ''}"
            style="color: {isUnset ? 'color-mix(in srgb, var(--muted-foreground) 50%, transparent)' : 'var(--foreground)'}"
          >
            {columnLabel(column)}
          </span>
          <span
            class="text-[10px] px-1.5 py-0 rounded-full"
            style="background: color-mix(in srgb, var(--muted) 50%, transparent); color: var(--muted-foreground)"
          >
            {columnBlocks.length}
          </span>
        </div>

        <!-- Cards -->
        <div class="flex flex-col gap-2 p-2 flex-1 min-h-[80px] overflow-y-auto max-h-[60vh]">
          {#each columnBlocks as block, cardIdx (block.id)}
            {@const isCardFocused = focused && colIdx === focusedColIndex && cardIdx === focusedCardIndex}
            <div
              data-kanban-focused={isCardFocused ? "true" : undefined}
              class="transition-opacity {draggedBlockId === block.id ? 'opacity-40' : ''}"
            >
              <KanbanCard
                {block}
                properties={tagName ? (typeDef?.properties ?? []) : globalProperties}
                groupByProp={groupByPropName}
                {propertyRegistry}
                isFocused={isCardFocused}
                ondragstart={handleCardDragStart}
                onmoverequest={handleMoveRequest}
              />
            </div>
          {/each}
          {#if columnBlocks.length === 0}
            <div
              class="text-[11px] text-center py-4 rounded-lg border border-dashed"
              style="color: color-mix(in srgb, var(--muted-foreground) 40%, transparent); border-color: var(--border)"
            >
              Drop here
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>

  {#if movePickerBlock}
    <KanbanColumnPicker
      columns={columnNames}
      currentColumn={movePickerBlock.properties[groupByPropName] ?? movePickerBlock.properties[groupByPropName.toLowerCase()] ?? "__unset__"}
      position={movePickerPosition}
      onselect={handleMovePick}
      onclose={() => (movePickerBlock = null)}
    />
  {/if}
{/if}
