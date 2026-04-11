<script lang="ts">
  import { onMount } from "svelte";

  export type SlashCommand = {
    id: string;
    label: string;
    description: string;
    icon: string;
    action: () => void;
  };

  let {
    commands,
    filter,
    position,
    onclose,
  }: {
    commands: SlashCommand[];
    filter: string;
    position: { x: number; y: number };
    onclose: () => void;
  } = $props();

  let selectedIndex = $state(0);

  const filtered = $derived(
    filter
      ? commands.filter(
          (c) =>
            c.label.toLowerCase().includes(filter.toLowerCase()) ||
            c.id.includes(filter.toLowerCase()),
        )
      : commands,
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
    } else if (e.key === "Enter" && filtered[selectedIndex]) {
      e.preventDefault();
      filtered[selectedIndex].action();
      onclose();
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
      if (!target.closest(".slash-menu")) onclose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  });
</script>

{#if filtered.length > 0}
  <div
    class="slash-menu fixed z-50 rounded-lg border border-border bg-popover text-popover-foreground shadow-xl w-64 max-h-60 overflow-y-auto"
    style="left: {position.x}px; top: {position.y}px"
  >
    <div class="p-1">
      {#each filtered as cmd, i (cmd.id)}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="flex items-center gap-2 rounded-md px-2 py-1.5 text-sm cursor-pointer {i === selectedIndex ? 'bg-accent text-accent-foreground' : ''}"
          onclick={() => { cmd.action(); onclose(); }}
          onmouseenter={() => (selectedIndex = i)}
        >
          <span class="text-base w-5 text-center shrink-0">{cmd.icon}</span>
          <div class="flex flex-col min-w-0">
            <span class="text-sm">{cmd.label}</span>
            <span class="text-xs text-muted-foreground truncate">{cmd.description}</span>
          </div>
        </div>
      {/each}
    </div>
  </div>
{/if}
