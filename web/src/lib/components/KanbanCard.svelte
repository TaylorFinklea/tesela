<script lang="ts">
  import { IconArrowsExchange } from "@tabler/icons-svelte";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { PropertyDef } from "$lib/types/PropertyDef";

  let {
    block,
    properties,
    groupByProp,
    isFocused = false,
    ondragstart,
    onmoverequest,
  }: {
    block: ParsedBlock;
    properties: PropertyDef[];
    groupByProp: string;
    isFocused?: boolean;
    ondragstart: (e: DragEvent, block: ParsedBlock) => void;
    onmoverequest: (block: ParsedBlock, event: MouseEvent) => void;
  } = $props();

  // Show up to 3 non-group-by properties that have values
  const badges = $derived.by(() => {
    return properties
      .filter((p) => p.name.toLowerCase() !== groupByProp.toLowerCase())
      .map((p) => ({
        name: p.name,
        value: block.properties[p.name] ?? block.properties[p.name.toLowerCase()] ?? "",
        valueType: p.value_type,
      }))
      .filter((b) => b.value !== "")
      .slice(0, 3);
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

  {#if badges.length > 0}
    <div class="flex flex-wrap gap-1 mt-2">
      {#each badges as badge}
        <span
          class="text-[10px] px-1.5 py-0.5 rounded-full"
          style="background: color-mix(in srgb, var(--primary) 8%, transparent); color: var(--muted-foreground)"
        >
          {badge.value}
        </span>
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
