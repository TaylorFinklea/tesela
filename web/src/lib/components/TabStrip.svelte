<script lang="ts">
  let {
    tabs,
    activeIdx,
    onSelect,
    onAdd,
    onDelete,
    onRename,
  }: {
    tabs: { name: string }[];
    activeIdx: number;
    onSelect: (idx: number) => void;
    /** When provided, a trailing `+` button is shown to add a new tab. */
    onAdd?: () => void;
    /** When provided, the active tab gets a hover-revealed `×` button when more than one tab exists. */
    onDelete?: (idx: number) => void;
    /** When provided, double-click on a tab opens a rename input. */
    onRename?: (idx: number, name: string) => void;
  } = $props();

  let editingIdx = $state<number | null>(null);
  let editingName = $state("");
</script>

<div class="flex items-center gap-0.5 flex-wrap min-w-0">
  {#each tabs as tab, i (i)}
    {@const active = i === activeIdx}
    <div class="group/tab inline-flex items-center gap-0.5">
      {#if editingIdx === i && onRename}
        <!-- svelte-ignore a11y_autofocus -->
        <input
          autofocus
          class="text-[12px] bg-surface border border-primary/40 rounded px-2 py-0.5 outline-none w-28"
          bind:value={editingName}
          onblur={() => { onRename?.(i, editingName); editingIdx = null; }}
          onkeydown={(e) => {
            if (e.key === "Enter") { onRename?.(i, editingName); editingIdx = null; }
            if (e.key === "Escape") { editingIdx = null; }
          }}
        />
      {:else}
        <button
          class="text-[12px] px-2.5 py-0.5 rounded transition-all {active ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-foreground/70 hover:bg-muted/30'}"
          onclick={() => onSelect(i)}
          ondblclick={onRename ? () => { editingIdx = i; editingName = tab.name; } : undefined}
          title={onRename ? "Click to switch · double-click to rename" : "Click to switch"}
        >{tab.name}</button>
        {#if onDelete && active && tabs.length > 1}
          <!-- svelte-ignore a11y_consider_explicit_label -->
          <button
            class="opacity-0 group-hover/tab:opacity-100 leading-none text-muted-foreground/40 hover:text-destructive text-[10px] transition-opacity"
            onclick={() => onDelete?.(i)}
            title="Delete tab"
          >×</button>
        {/if}
      {/if}
    </div>
  {/each}
  {#if onAdd}
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="text-[12px] px-1.5 py-0.5 rounded text-muted-foreground/40 hover:text-primary hover:bg-muted/30 transition-colors"
      onclick={onAdd}
      title="Add new tab"
    >+</button>
  {/if}
</div>
