<script lang="ts">
  import { onMount } from "svelte";

  export type AutocompleteItem = {
    id: string;
    label: string;
    secondary?: string;
  };

  let {
    items,
    filter,
    position,
    onselect,
    onclose,
  }: {
    items: AutocompleteItem[];
    filter: string;
    position: { x: number; y: number };
    onselect: (item: AutocompleteItem) => void;
    onclose: () => void;
  } = $props();

  let selectedIndex = $state(0);

  const filtered = $derived(
    filter
      ? items.filter((item) => item.label.toLowerCase().includes(filter.toLowerCase()))
      : items,
  );

  $effect(() => {
    filter;
    selectedIndex = 0;
  });

  export function handleKeydown(e: KeyboardEvent): boolean {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(filtered.length - 1, selectedIndex + 1);
      return true;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
      return true;
    } else if ((e.key === "Enter" || e.key === "Tab") && filtered[selectedIndex]) {
      e.preventDefault();
      onselect(filtered[selectedIndex]);
      return true;
    } else if (e.key === "Escape") {
      e.preventDefault();
      onclose();
      return true;
    }
    return false;
  }

  onMount(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".autocomplete-menu")) onclose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  });
</script>

{#if filtered.length > 0}
  <div
    class="autocomplete-menu fixed z-50 rounded-md border border-border bg-popover text-popover-foreground shadow-lg w-52 max-h-48 overflow-y-auto"
    style="left: {position.x}px; top: {position.y}px"
  >
    <div class="py-0.5">
      {#each filtered as item, i (item.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="flex items-center gap-2 px-2 py-1 text-[12px] cursor-pointer {i === selectedIndex ? 'bg-accent text-accent-foreground' : ''}"
          onclick={() => onselect(item)}
          onmouseenter={() => (selectedIndex = i)}
        >
          <span class="truncate">{item.label}</span>
          {#if item.secondary}
            <span class="ml-auto text-[10px] text-muted-foreground/50 shrink-0">{item.secondary}</span>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/if}
