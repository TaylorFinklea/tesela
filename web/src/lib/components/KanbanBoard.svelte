<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { updateBlockProperty, clearBlockProperty } from "$lib/property-update";
  import { getGroupByProp, setGroupByProp } from "$lib/stores/tag-view-prefs.svelte";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";
  import type { PropertyDef } from "$lib/types/PropertyDef";
  import KanbanCard from "./KanbanCard.svelte";
  import KanbanColumnPicker from "./KanbanColumnPicker.svelte";

  let { tagName }: { tagName: string } = $props();

  const queryClient = useQueryClient();

  const typeQuery = createQuery(() => ({
    queryKey: ["type", tagName] as const,
    queryFn: () => api.getType(tagName),
  }));

  const blocksQuery = createQuery(() => ({
    queryKey: ["typed-blocks", tagName] as const,
    queryFn: () => api.getTypedBlocks(tagName),
  }));

  const typeDef: TypeDefinition | undefined = $derived(typeQuery.data as TypeDefinition | undefined);
  const blocks: ParsedBlock[] = $derived((blocksQuery.data ?? []) as ParsedBlock[]);

  // Only properties with value_type "select" and defined choices
  const selectProperties = $derived(
    (typeDef?.properties ?? []).filter(
      (p): p is PropertyDef & { values: string[] } => p.value_type === "select" && Array.isArray(p.values) && p.values.length > 0,
    ),
  );

  // Resolve group-by property: stored preference or first select property
  const groupByPropName = $derived.by(() => {
    const stored = getGroupByProp(tagName);
    if (stored && selectProperties.some((p) => p.name === stored)) return stored;
    return selectProperties[0]?.name ?? "";
  });

  const groupByDef = $derived(selectProperties.find((p) => p.name === groupByPropName));

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
        await clearBlockProperty({ block, propKey: groupByPropName, tagName, queryClient });
      } else {
        await updateBlockProperty({ block, propKey: groupByPropName, value: column, tagName, queryClient });
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
        await clearBlockProperty({ block, propKey: groupByPropName, tagName, queryClient });
      } else {
        await updateBlockProperty({ block, propKey: groupByPropName, value: column, tagName, queryClient });
      }
    } catch (err) {
      console.error("Failed to move card:", err);
    }
  }

  function columnLabel(col: string): string {
    return col === "__unset__" ? "Unset" : col;
  }
</script>

{#if blocksQuery.isLoading || typeQuery.isLoading}
  <div class="text-[12px] text-muted-foreground py-4">Loading...</div>
{:else if selectProperties.length === 0}
  <div class="text-[12px] text-muted-foreground py-4 italic">
    No select properties defined. Add a select property to use Kanban view.
  </div>
{:else}
  <!-- Group-by picker -->
  <div class="flex items-center gap-2 mb-3 px-1">
    <span class="text-[10px] text-muted-foreground/60 uppercase tracking-widest">Group by</span>
    <select
      value={groupByPropName}
      onchange={(e) => setGroupByProp(tagName, (e.target as HTMLSelectElement).value)}
      class="text-[11px] px-2 py-0.5 rounded-md border transition-colors outline-none"
      style="background: var(--surface); border-color: var(--border); color: var(--foreground)"
    >
      {#each selectProperties as prop}
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
    {#each columnNames as column (column)}
      {@const columnBlocks = groupedBlocks.get(column) ?? []}
      {@const isUnset = column === "__unset__"}
      {@const isDragOver = dragOverColumn === column}
      <div
        class="flex-shrink-0 w-64 min-w-[256px] flex flex-col rounded-lg transition-all"
        class:ring-2={isDragOver}
        style="
          background: color-mix(in srgb, var(--surface) 50%, transparent);
          {isDragOver ? `ring-color: color-mix(in srgb, var(--primary) 30%, transparent); background: color-mix(in srgb, var(--primary) 5%, transparent)` : ''}
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
          {#each columnBlocks as block (block.id)}
            <div
              class="transition-opacity {draggedBlockId === block.id ? 'opacity-40' : ''}"
            >
              <KanbanCard
                {block}
                properties={typeDef?.properties ?? []}
                groupByProp={groupByPropName}
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
