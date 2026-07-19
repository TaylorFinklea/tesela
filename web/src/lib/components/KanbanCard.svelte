<script lang="ts">
  import { IconArrowsExchange } from "@tabler/icons-svelte";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { PropertyDef } from "$lib/types/PropertyDef";
  import type { PropertyRegistry } from "$lib/property-registry";
  import type { MultiSelectDelta } from "$lib/property-editing";
  import DisplayChip from "./DisplayChip.svelte";

  let {
    block,
    properties,
    groupByProp,
    propertyRegistry,
    isFocused = false,
    ondragstart,
    onmoverequest,
    onsetproperty,
    onlistchange,
  }: {
    block: ParsedBlock;
    properties: PropertyDef[];
    groupByProp: string;
    /**
     * Phase 11 — passed in by `KanbanBoard` so each card can render its
     * non-group-by properties via the same `<DisplayChip>` system used
     * inline on blocks (calendar icon for deadline, flag bars for
     * priority, etc). Falls back to a crude text badge for any property
     * not in the registry, so unconfigured tags still get something.
     */
    propertyRegistry: PropertyRegistry;
    isFocused?: boolean;
    ondragstart: (e: DragEvent, block: ParsedBlock) => void;
    onmoverequest: (block: ParsedBlock, event: MouseEvent) => void;
    onsetproperty: (propKey: string, value: string) => void;
    onlistchange: (propKey: string, delta: MultiSelectDelta) => void;
  } = $props();

  /**
   * Pair each non-group-by property with its block value AND its registry
   * def. Defs come from the registry built off all notes; if a def is
   * missing the chip falls back to plain text rendering. Skip when value
   * is empty so the card stays compact.
   */
  const chipRows = $derived.by(() => {
    const groupKey = groupByProp.toLowerCase();
    return properties
      .filter((p) => p.name.toLowerCase() !== groupKey)
      .map((p) => {
        const value = block.properties[p.name] ?? block.properties[p.name.toLowerCase()] ?? "";
        const def = propertyRegistry.get(p.name.toLowerCase());
        return { propKey: p.name.toLowerCase(), value, def };
      })
      .filter((r) => r.value.trim() !== "")
      .slice(0, 4);
  });

  function handleDragStart(e: DragEvent) {
    ondragstart(e, block);
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="kanban-card group rounded-lg border p-3 cursor-grab active:cursor-grabbing transition-all hover:shadow-md"
  class:ring-2={isFocused}
  style="
    background: var(--block-bg);
    border-color: {isFocused ? 'var(--primary)' : 'var(--block-border)'};
    box-shadow: var(--block-shadow);
    {isFocused ? 'ring-color: color-mix(in srgb, var(--primary) 40%, transparent)' : ''}
  "
  draggable="true"
  ondragstart={handleDragStart}
>
  <div class="flex items-start gap-1">
    <div class="flex-1 text-[13px] leading-snug line-clamp-2" style="color: var(--foreground)">
      {block.text || "(empty)"}
    </div>
    <button
      onclick={(e: MouseEvent) => { e.stopPropagation(); onmoverequest(block, e); }}
      class="shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-60 hover:!opacity-100 transition-opacity"
      style="color: var(--muted-foreground)"
      title="Move to column"
    >
      <IconArrowsExchange size={12} />
    </button>
  </div>

  {#if chipRows.length > 0}
    <div class="flex flex-wrap gap-1 mt-2">
      {#each chipRows as row}
        {#if row.def}
          <DisplayChip
            propKey={row.propKey}
            value={row.value}
            def={row.def}
            onset={(value) => onsetproperty(row.propKey, value)}
            onlistchange={(delta) => onlistchange(row.propKey, delta)}
          />
        {:else}
          <span
            class="text-[10px] px-1.5 py-0.5 rounded-full"
            style="background: color-mix(in srgb, var(--primary) 8%, transparent); color: var(--muted-foreground)"
            title="{row.propKey}: {row.value}"
          >{row.value}</span>
        {/if}
      {/each}
    </div>
  {/if}

  <a
    href="/p/{encodeURIComponent(block.note_id)}"
    class="block text-[10px] mt-1.5 truncate transition-colors hover:underline"
    style="color: color-mix(in srgb, var(--muted-foreground) 50%, transparent)"
  >
    {block.note_id}
  </a>
</div>
