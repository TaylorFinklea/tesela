<script lang="ts">
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { PropertyDef } from "$lib/types/PropertyDef";

  let {
    block,
    properties,
    groupByProp,
    ondragstart,
    onmoverequest,
  }: {
    block: ParsedBlock;
    properties: PropertyDef[];
    groupByProp: string;
    ondragstart: (e: DragEvent, block: ParsedBlock) => void;
    onmoverequest: (block: ParsedBlock) => void;
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

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "m") {
      e.preventDefault();
      onmoverequest(block);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="kanban-card rounded-lg border p-3 cursor-grab active:cursor-grabbing transition-all hover:shadow-md focus:outline-none"
  style="background: var(--block-bg); border-color: var(--block-border); box-shadow: var(--block-shadow)"
  draggable="true"
  tabindex="0"
  ondragstart={handleDragStart}
  onkeydown={handleKeydown}
>
  <div class="text-[13px] leading-snug line-clamp-2" style="color: var(--foreground)">
    {block.text || "(empty)"}
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
