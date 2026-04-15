<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { updateBlockProperty } from "$lib/property-update";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";
  import type { PropertyDef } from "$lib/types/PropertyDef";
  import PropertyEditor from "./PropertyEditor.svelte";

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
  const columns = $derived(typeDef?.properties ?? []);

  let sortColumn = $state<string | null>(null);
  let sortAsc = $state(true);
  let filters = $state<Record<string, string>>({});
  let showFilters = $state(false);

  const filteredBlocks = $derived.by(() => {
    const activeFilters = Object.entries(filters).filter(([, v]) => v.trim() !== "");
    if (activeFilters.length === 0) return blocks;
    return blocks.filter((b) =>
      activeFilters.every(([key, query]) => {
        const val = (b.properties[key] ?? b.properties[key.toLowerCase()] ?? "").toLowerCase();
        return val.includes(query.toLowerCase());
      }),
    );
  });

  const sortedBlocks = $derived.by(() => {
    if (!sortColumn) return filteredBlocks;
    return [...filteredBlocks].sort((a, b) => {
      const key = sortColumn!;
      const av = a.properties[key] ?? a.properties[key.toLowerCase()] ?? "";
      const bv = b.properties[key] ?? b.properties[key.toLowerCase()] ?? "";
      const cmp = av.localeCompare(bv);
      return sortAsc ? cmp : -cmp;
    });
  });

  const activeFilterCount = $derived(Object.values(filters).filter((v) => v.trim() !== "").length);

  function toggleSort(col: string) {
    if (sortColumn === col) sortAsc = !sortAsc;
    else { sortColumn = col; sortAsc = true; }
  }

  // Property editing state
  let editingBlock = $state<ParsedBlock | null>(null);
  let editingProp = $state<PropertyDef | null>(null);
  let editorPosition = $state({ x: 0, y: 0 });

  function openPropertyEditor(block: ParsedBlock, prop: PropertyDef, event: MouseEvent) {
    editingBlock = block;
    editingProp = prop;
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    editorPosition = { x: rect.left, y: rect.bottom + 2 };
  }

  async function handlePropertyUpdate(value: string) {
    if (!editingBlock || !editingProp) return;
    try {
      await updateBlockProperty({
        block: editingBlock,
        propKey: editingProp.name,
        value,
        tagName,
        queryClient,
      });
    } catch (e) {
      console.error("Failed to update property:", e);
    }
    editingBlock = null;
    editingProp = null;
  }

  function getPropertyValue(block: ParsedBlock, propName: string): string {
    return block.properties[propName] ?? block.properties[propName.toLowerCase()] ?? "";
  }
</script>

{#if blocksQuery.isLoading || typeQuery.isLoading}
  <div class="text-[12px] text-muted-foreground py-4">Loading…</div>
{:else if blocks.length === 0}
  <div class="text-[12px] text-muted-foreground py-4 italic">No blocks tagged #{tagName}</div>
{:else}
  <!-- Filter toggle -->
  <div class="flex items-center gap-2 mb-2 px-3">
    <button
      onclick={() => { showFilters = !showFilters; if (!showFilters) filters = {}; }}
      class="text-[11px] px-2 py-1 rounded-md transition-all {showFilters || activeFilterCount > 0 ? 'bg-primary/10 text-primary' : 'text-muted-foreground/60 hover:text-foreground/70 hover:bg-muted/30'}"
    >
      Filter{activeFilterCount > 0 ? ` (${activeFilterCount})` : ""}
    </button>
    {#if activeFilterCount > 0}
      <button
        onclick={() => { filters = {}; }}
        class="text-[10px] text-muted-foreground/50 hover:text-foreground/70 transition-colors"
      >
        Clear
      </button>
    {/if}
    <span class="flex-1"></span>
    <span class="text-[10px] text-muted-foreground/40">{filteredBlocks.length} of {blocks.length} blocks</span>
  </div>

  <div class="overflow-x-auto">
    <table class="w-full text-[12px]">
      <thead>
        <tr class="border-b border-border">
          <th class="text-left px-3 py-1.5 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Block</th>
          <th class="text-left px-3 py-1.5 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Note</th>
          {#each columns as prop}
            <th
              class="text-left px-3 py-1.5 text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest cursor-pointer hover:text-foreground select-none"
              onclick={() => toggleSort(prop.name)}
            >
              {prop.name}
              {#if sortColumn === prop.name}
                <span class="ml-0.5">{sortAsc ? "↑" : "↓"}</span>
              {/if}
            </th>
          {/each}
        </tr>
        {#if showFilters}
          <tr class="border-b border-border/50 bg-muted/20">
            <th></th>
            <th></th>
            {#each columns as prop}
              <th class="px-2 py-1">
                <input
                  type="text"
                  placeholder="Filter…"
                  value={filters[prop.name] ?? ""}
                  oninput={(e) => { filters = { ...filters, [prop.name]: (e.target as HTMLInputElement).value }; }}
                  class="w-full text-[11px] bg-transparent border border-border/50 rounded px-2 py-0.5 text-foreground/80 placeholder:text-muted-foreground/30 outline-none focus:border-primary/40 transition-colors"
                />
              </th>
            {/each}
          </tr>
        {/if}
      </thead>
      <tbody>
        {#each sortedBlocks as block (block.id)}
          <tr class="border-b border-border/30 hover:bg-accent/20 transition-colors">
            <td class="px-3 py-1.5">
              <a href="/p/{encodeURIComponent(block.note_id)}" class="hover:underline">
                {block.text || "(empty)"}
              </a>
            </td>
            <td class="px-3 py-1.5 text-muted-foreground/60">
              <a href="/p/{encodeURIComponent(block.note_id)}" class="hover:underline">
                {block.note_id}
              </a>
            </td>
            {#each columns as prop}
              {@const val = getPropertyValue(block, prop.name)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <td
                class="px-3 py-1.5 text-muted-foreground cursor-pointer hover:text-foreground hover:bg-accent/30 rounded transition-colors"
                onclick={(e) => openPropertyEditor(block, prop, e)}
              >
                {val || "—"}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>

  {#if editingBlock && editingProp}
    <PropertyEditor
      propertyName={editingProp.name}
      currentValue={getPropertyValue(editingBlock, editingProp.name)}
      valueType={editingProp.value_type}
      choices={editingProp.values ?? null}
      position={editorPosition}
      onselect={handlePropertyUpdate}
      onclose={() => { editingBlock = null; editingProp = null; }}
    />
  {/if}
{/if}
