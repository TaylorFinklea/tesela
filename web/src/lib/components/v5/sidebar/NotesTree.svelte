<script lang="ts">
  /* Notes tree surface. Alphabetical list of pages, scratches filtered
   * out by default (toggle to show). Phase 11. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  import { isPinned, togglePin } from "$lib/state/shared.svelte";

  const q = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));

  let showScratches = $state(false);

  const notes = $derived.by(() => {
    const all = (q.data ?? []) as Note[];
    const filtered = showScratches
      ? all
      : all.filter((n) => n.metadata.note_type !== "scratch");
    return filtered.slice().sort((a, b) => a.title.localeCompare(b.title));
  });
  const scratchCount = $derived(
    ((q.data ?? []) as Note[]).filter(
      (n) => n.metadata.note_type === "scratch",
    ).length,
  );

  function open(id: string) {
    openPageInFocused(asPageId(id));
  }
</script>

<div class="v5-side-surface">
  <header>
    Notes
    {#if scratchCount > 0}
      <button
        type="button"
        class="toggle"
        class:on={showScratches}
        title="show / hide scratch pages"
        onclick={() => (showScratches = !showScratches)}
      >
        {showScratches ? "✓" : "·"} scratches · {scratchCount}
      </button>
    {/if}
  </header>
  {#if q.isLoading}
    <p class="muted">loading…</p>
  {:else if notes.length === 0}
    <p class="muted">no notes</p>
  {:else}
    <ul>
      {#each notes as n (n.id)}
        <li>
          <button
            type="button"
            class="row"
            class:scratch={n.metadata.note_type === "scratch"}
            onclick={() => open(n.id)}
          >
            <span class="title">{n.title}</span>
            {#if n.metadata.note_type === "scratch"}
              <span class="chip">scratch</span>
            {/if}
          </button>
          <button
            type="button"
            class="pin"
            class:active={isPinned(n.id)}
            onclick={() => togglePin(n.id)}
            title="pin / unpin"
          >★</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .v5-side-surface {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 12px;
    font-family: var(--v4-mono);
    font-size: 11px;
    color: var(--v4-ink2);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
    color: var(--v4-ink);
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.7px;
    margin-bottom: 8px;
  }
  .toggle {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink5);
    border-radius: 4px;
    padding: 1px 6px;
    cursor: pointer;
    font-family: var(--v4-mono);
    font-size: 9.5px;
    text-transform: none;
    letter-spacing: 0;
  }
  .toggle.on {
    color: var(--v4-accent);
    border-color: var(--v4-accent-dim);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 4px;
  }
  .row {
    background: transparent;
    border: 1px solid transparent;
    color: var(--v4-ink2);
    text-align: left;
    padding: 3px 6px;
    border-radius: 4px;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--v4-mono);
    font-size: 11px;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .row:hover {
    border-color: var(--v4-hair);
    color: var(--v4-ink);
  }
  .row.scratch .title {
    color: var(--v4-ink5);
    font-style: italic;
  }
  .chip {
    color: var(--v4-ink5);
    border: 1px solid var(--v4-hair);
    border-radius: 3px;
    padding: 0 4px;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }
  .pin {
    background: transparent;
    border: 0;
    color: var(--v4-ink6);
    cursor: pointer;
    font-size: 12px;
    padding: 0 4px;
  }
  .pin.active {
    color: var(--v4-accent);
  }
  .muted {
    color: var(--v4-ink5);
  }
</style>
