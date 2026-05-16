<script lang="ts">
  /* Notes tree surface. Phase 6: alphabetical flat list of pages (no
   * folder hierarchy yet). Phase 11 wires scratch-filtering. */
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
  const notes = $derived(
    ((q.data ?? []) as Note[])
      .slice()
      .sort((a, b) => a.title.localeCompare(b.title)),
  );

  function open(id: string) {
    openPageInFocused(asPageId(id));
  }
</script>

<div class="v5-side-surface">
  <header>Notes</header>
  {#if q.isLoading}
    <p class="muted">loading…</p>
  {:else if notes.length === 0}
    <p class="muted">no notes</p>
  {:else}
    <ul>
      {#each notes as n (n.id)}
        <li>
          <button type="button" class="row" onclick={() => open(n.id)}>
            <span class="title">{n.title}</span>
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
    color: var(--v4-ink);
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.7px;
    margin-bottom: 8px;
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
  }
  .row:hover {
    border-color: var(--v4-hair);
    color: var(--v4-ink);
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
