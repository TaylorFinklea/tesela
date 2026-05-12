<script lang="ts">
  type Item = { label: string; action: () => void };

  let { items, x, y, onclose }: {
    items: Item[];
    x: number;
    y: number;
    onclose: () => void;
  } = $props();

  function handleOutside(e: MouseEvent) {
    const el = e.target as HTMLElement | null;
    if (!el?.closest(".v9-ctxmenu")) onclose();
  }
  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); onclose(); }
  }

  $effect(() => {
    document.addEventListener("mousedown", handleOutside, true);
    document.addEventListener("keydown", handleKey, true);
    return () => {
      document.removeEventListener("mousedown", handleOutside, true);
      document.removeEventListener("keydown", handleKey, true);
    };
  });
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="v9-ctxmenu"
  style:left="{x}px"
  style:top="{y}px"
  role="menu"
  onkeydown={(e) => { if (e.key === "Escape") { e.preventDefault(); onclose(); } }}
>
  {#each items as it}
    <button
      class="v9-ctxmenu-item"
      role="menuitem"
      onclick={() => { it.action(); onclose(); }}
    >
      {it.label}
    </button>
  {/each}
</div>

<style>
  .v9-ctxmenu {
    position: fixed;
    min-width: 160px;
    background: var(--v9-bg-2);
    border: 1px solid var(--v9-line);
    border-radius: 4px;
    padding: 4px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.4);
    z-index: 1000;
    font-size: 12px;
  }
  .v9-ctxmenu-item {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    color: var(--v9-ink-2);
    padding: 6px 8px;
    cursor: pointer;
    border-radius: 3px;
  }
  .v9-ctxmenu-item:hover { background: var(--v9-bg-3); color: var(--v9-ink); }
</style>
