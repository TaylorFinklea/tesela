<script lang="ts">
  /**
   * Modal autocomplete picker over Project pages (Phase 9.4 — `p` triage key).
   * Filters by typed text; arrow keys navigate; Enter selects; Esc cancels.
   */
  import type { Note } from "$lib/types/Note";

  type Props = {
    notes: Note[];
    onselect: (project: Note) => void;
    onclose: () => void;
  };
  let { notes, onselect, onclose }: Props = $props();

  let filter = $state("");
  let selectedIndex = $state(0);
  let inputEl = $state<HTMLInputElement | undefined>();

  const projects = $derived(notes.filter((n) => n.metadata.note_type === "Project"));
  const filtered = $derived(
    filter
      ? projects.filter((p) => p.title.toLowerCase().includes(filter.toLowerCase()))
      : projects,
  );

  $effect(() => {
    filter; // reset selection on filter change
    selectedIndex = 0;
  });

  $effect(() => {
    if (inputEl && document.activeElement !== inputEl) inputEl.focus();
  });

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(filtered.length - 1, selectedIndex + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (filtered[selectedIndex]) onselect(filtered[selectedIndex]);
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="picker-backdrop" onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}>
  <div class="picker">
    <input
      bind:this={inputEl}
      bind:value={filter}
      onkeydown={handleKey}
      placeholder="Filter projects…"
      type="text"
    />
    <div class="list">
      {#if filtered.length === 0}
        <div class="empty">No projects.</div>
      {:else}
        {#each filtered as p, i}
          <button
            class="item {i === selectedIndex ? 'selected' : ''}"
            type="button"
            onclick={() => onselect(p)}
          >
            <span class="dot"></span>
            <span class="t">{p.title}</span>
            <span class="id">{p.id}</span>
          </button>
        {/each}
      {/if}
    </div>
    <div class="hint">↑↓ navigate · ↵ select · Esc cancel</div>
  </div>
</div>

<style>
  .picker-backdrop {
    position: fixed; inset: 0; background: rgba(0,0,0,0.55);
    display: grid; place-items: center; z-index: 200;
  }
  .picker {
    background: var(--v9-bg-2);
    border: 1px solid var(--v9-line);
    border-radius: 6px;
    width: min(420px, 90vw);
    max-height: 60vh;
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  input {
    background: var(--v9-bg-3);
    color: var(--v9-ink);
    border: none;
    border-bottom: 1px solid var(--v9-line);
    padding: 10px 14px;
    font-family: var(--v9-mono);
    font-size: 12px;
    outline: none;
  }
  input::placeholder { color: var(--v9-ink-faint); }
  .list {
    flex: 1; overflow-y: auto;
    padding: 4px 0;
  }
  .empty {
    padding: 14px;
    color: var(--v9-ink-faint);
    font-family: var(--v9-mono);
    font-size: 11px;
  }
  .item {
    display: grid;
    grid-template-columns: 12px 1fr auto;
    gap: 8px;
    align-items: center;
    width: 100%;
    padding: 6px 14px;
    background: transparent;
    color: var(--v9-ink);
    border: none;
    text-align: left;
    font-size: 12.5px;
    cursor: pointer;
  }
  .item:hover, .item.selected {
    background: var(--v9-bg-3);
  }
  .item.selected { color: var(--v9-amber); }
  .item .dot {
    width: 7px; height: 7px; border-radius: 50%;
    background: var(--v9-indigo);
    justify-self: center;
  }
  .item .t { color: var(--v9-ink); }
  .item.selected .t { color: var(--v9-amber); }
  .item .id {
    color: var(--v9-ink-faint);
    font-family: var(--v9-mono);
    font-size: 10px;
  }
  .hint {
    padding: 6px 14px;
    border-top: 1px solid var(--v9-line);
    color: var(--v9-ink-faint);
    font-family: var(--v9-mono);
    font-size: 10px;
  }
</style>
