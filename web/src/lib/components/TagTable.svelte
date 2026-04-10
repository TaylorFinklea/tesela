<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";

  let { tagName }: { tagName: string } = $props();

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
  const columns = $derived(typeDef?.properties.map((p) => p.name) ?? []);

  // Sort state
  let sortColumn = $state<string | null>(null);
  let sortAsc = $state(true);

  const sortedBlocks = $derived.by(() => {
    if (!sortColumn) return blocks;
    return [...blocks].sort((a, b) => {
      const key = sortColumn!;
      const av = a.properties[key] ?? a.properties[key.toLowerCase()] ?? "";
      const bv = b.properties[key] ?? b.properties[key.toLowerCase()] ?? "";
      const cmp = av.localeCompare(bv);
      return sortAsc ? cmp : -cmp;
    });
  });

  function toggleSort(col: string) {
    if (sortColumn === col) {
      sortAsc = !sortAsc;
    } else {
      sortColumn = col;
      sortAsc = true;
    }
  }
</script>

{#if blocksQuery.isLoading || typeQuery.isLoading}
  <div class="text-sm text-muted-foreground py-4">Loading…</div>
{:else if blocks.length === 0}
  <div class="text-sm text-muted-foreground py-4 italic">No blocks tagged #{tagName}</div>
{:else}
  <div class="overflow-x-auto">
    <table class="w-full text-sm">
      <thead>
        <tr class="border-b border-border">
          <th class="text-left px-3 py-2 text-xs font-medium text-muted-foreground">Block</th>
          <th class="text-left px-3 py-2 text-xs font-medium text-muted-foreground">Note</th>
          {#each columns as col}
            <th
              class="text-left px-3 py-2 text-xs font-medium text-muted-foreground cursor-pointer hover:text-foreground select-none"
              onclick={() => toggleSort(col)}
            >
              {col}
              {#if sortColumn === col}
                <span class="ml-0.5">{sortAsc ? "↑" : "↓"}</span>
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each sortedBlocks as block (block.id)}
          <tr class="border-b border-border/50 hover:bg-accent/30 transition-colors">
            <td class="px-3 py-2">
              <a
                href="/p/{encodeURIComponent(block.note_id)}"
                class="hover:underline"
              >
                {block.text || "(empty)"}
              </a>
            </td>
            <td class="px-3 py-2 text-muted-foreground">
              <a
                href="/p/{encodeURIComponent(block.note_id)}"
                class="hover:underline"
              >
                {block.note_id}
              </a>
            </td>
            {#each columns as col}
              <td class="px-3 py-2 text-muted-foreground">
                {block.properties[col] ?? block.properties[col.toLowerCase()] ?? "—"}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  </div>
  <div class="text-xs text-muted-foreground mt-2 px-3">{blocks.length} blocks</div>
{/if}
