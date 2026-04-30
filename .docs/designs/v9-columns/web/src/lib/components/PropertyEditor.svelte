<script lang="ts">
  import { onMount } from "svelte";

  let {
    propertyName,
    currentValue,
    valueType,
    choices,
    position,
    onselect,
    onclose,
  }: {
    propertyName: string;
    currentValue: string;
    valueType: string;
    choices: string[] | null;
    position: { x: number; y: number };
    onselect: (value: string) => void;
    onclose: () => void;
  } = $props();

  let selectedIndex = $state(0);
  let textValue = $state(currentValue);

  // For select types, find current selection
  $effect(() => {
    if (choices && currentValue) {
      const idx = choices.indexOf(currentValue);
      if (idx >= 0) selectedIndex = idx;
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (valueType === "select" && choices) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        selectedIndex = Math.min(choices.length - 1, selectedIndex + 1);
      } else if (e.key === "ArrowUp") {
        e.preventDefault();
        selectedIndex = Math.max(0, selectedIndex - 1);
      } else if (e.key === "Enter") {
        e.preventDefault();
        onselect(choices[selectedIndex]);
      } else if (e.key === "Escape") {
        e.preventDefault();
        onclose();
      }
    } else {
      if (e.key === "Enter") {
        e.preventDefault();
        onselect(textValue);
      } else if (e.key === "Escape") {
        e.preventDefault();
        onclose();
      }
    }
  }

  onMount(() => {
    const handler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".property-editor")) onclose();
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  });
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="property-editor fixed z-50 rounded-md border border-border bg-popover text-popover-foreground shadow-lg w-48"
  style="left: {position.x}px; top: {position.y}px"
  onkeydown={handleKeydown}
>
  <div class="px-2 py-1 border-b border-border">
    <span class="text-[10px] text-muted-foreground/60 uppercase tracking-widest">{propertyName}</span>
  </div>

  {#if valueType === "select" && choices}
    <div class="py-0.5 max-h-40 overflow-y-auto">
      {#each choices as choice, i}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <div
          class="px-2 py-1 text-[12px] cursor-pointer flex items-center gap-2
            {i === selectedIndex ? 'bg-accent text-accent-foreground' : ''}
            {choice === currentValue ? 'font-medium' : ''}"
          onclick={() => onselect(choice)}
          onmouseenter={() => (selectedIndex = i)}
        >
          {#if choice === currentValue}
            <span class="text-primary text-[10px]">●</span>
          {:else}
            <span class="text-[10px] opacity-0">●</span>
          {/if}
          <span>{choice}</span>
        </div>
      {/each}
    </div>
  {:else}
    <div class="p-2">
      <input
        type={valueType === "date" ? "date" : "text"}
        bind:value={textValue}
        onkeydown={handleKeydown}
        class="w-full text-[12px] bg-muted/50 rounded px-2 py-1 text-foreground outline-none border border-transparent focus:border-ring/30"
        autofocus
      />
    </div>
  {/if}
</div>
