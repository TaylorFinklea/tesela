<script lang="ts">
  import { onMount } from "svelte";

  let {
    columns,
    currentColumn,
    position,
    onselect,
    onclose,
  }: {
    columns: string[];
    currentColumn: string;
    position: { x: number; y: number };
    onselect: (column: string) => void;
    onclose: () => void;
  } = $props();

  let selectedIndex = $state(0);

  $effect(() => {
    const idx = columns.indexOf(currentColumn);
    if (idx >= 0) selectedIndex = idx;
  });

  function label(col: string): string {
    return col === "__unset__" ? "Unset" : col;
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "j") {
      e.preventDefault();
      selectedIndex = Math.min(columns.length - 1, selectedIndex + 1);
    } else if (e.key === "ArrowUp" || e.key === "k") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      onselect(columns[selectedIndex]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }

  onMount(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".column-picker")) onclose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="column-picker fixed z-50 rounded-md border shadow-lg w-44"
  style="left: {position.x}px; top: {position.y}px; background: var(--popover); border-color: var(--border); color: var(--popover-foreground)"
>
  <div class="px-2 py-1" style="border-bottom: 1px solid var(--border)">
    <span class="text-[10px] uppercase tracking-widest" style="color: color-mix(in srgb, var(--muted-foreground) 60%, transparent)">
      Move to
    </span>
  </div>
  <div class="py-0.5 max-h-48 overflow-y-auto">
    {#each columns as col, i}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="px-2 py-1 text-[12px] cursor-pointer flex items-center gap-2 transition-colors"
        style="{i === selectedIndex ? 'background: var(--accent); color: var(--accent-foreground)' : ''}"
        onclick={() => onselect(col)}
        onmouseenter={() => (selectedIndex = i)}
      >
        {#if col === currentColumn}
          <span class="text-[10px]" style="color: var(--primary)">●</span>
        {:else}
          <span class="text-[10px] opacity-0">●</span>
        {/if}
        <span class="{col === '__unset__' ? 'italic' : ''}">{label(col)}</span>
      </div>
    {/each}
  </div>
</div>
