<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
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
    const block = editingBlock;
    const propKey = editingProp.name.toLowerCase();

    // Fetch the note, update the block's property in the raw content
    try {
      const note = await api.getNote(block.note_id);
      const content = note.content;

      // Find the block in the content and update the property line
      const lines = content.split("\n");
      let updated = false;

      // Find the block by matching its text (more reliable than line-number indexing)
      const blockText = block.raw_text.split("\n")[0] ?? "";
      let inBlock = false;

      for (let i = 0; i < lines.length; i++) {
        const trimmed = lines[i].trim();

        // Match the block's first line (strip "- " prefix for comparison)
        if (trimmed.startsWith("- ") && trimmed.slice(2).startsWith(blockText.split("\n")[0])) {
          inBlock = true;
          continue;
        }

        if (inBlock) {
          // End of block: next block line or empty line
          if (trimmed.startsWith("- ") || (trimmed === "" && i > 0)) {
            // Property not found in block — insert it before this line
            const blockIndent = lines[i - 1] ? (lines[i - 1].length - lines[i - 1].trimStart().length) : 2;
            lines.splice(i, 0, " ".repeat(blockIndent) + `${propKey}:: ${value}`);
            updated = true;
            break;
          }
          const propMatch = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/);
          if (propMatch && propMatch[1].toLowerCase() === propKey) {
            // Replace the property value
            const indent = lines[i].length - lines[i].trimStart().length;
            lines[i] = " ".repeat(indent) + `${propMatch[1]}:: ${value}`;
            updated = true;
            break;
          }
        }
      }

      if (!updated && inBlock) {
        lines.push(`  ${propKey}:: ${value}`);
      }

      const newContent = lines.join("\n");
      await api.updateNote(block.note_id, newContent);
      queryClient.invalidateQueries({ queryKey: ["typed-blocks", tagName] });
      queryClient.invalidateQueries({ queryKey: ["note", block.note_id] });
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
  <div class="text-[10px] text-muted-foreground/40 mt-2 px-3">{blocks.length} blocks</div>

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
