<script lang="ts">
  /* Search surface — title-substring filter for Phase 6/7. A real index
   * (FlexSearch / Orama) lands later. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  let q = $state("");

  const allQ = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const all = $derived((allQ.data ?? []) as Note[]);
  const filtered = $derived.by(() => {
    const term = q.toLowerCase().trim();
    if (!term) return [];
    return all
      .filter((n) => n.title.toLowerCase().includes(term))
      .slice(0, 40);
  });
</script>

<div class="v5-side-surface">
  <header>Search</header>
  <input
    type="text"
    placeholder="filter by title…"
    bind:value={q}
    autocomplete="off"
  />
  {#if !q}
    <p class="muted">type to filter</p>
  {:else if filtered.length === 0}
    <p class="muted">no matches</p>
  {:else}
    <ul>
      {#each filtered as n (n.id)}
        <li>
          <button
            type="button"
            onclick={() => openPageInFocused(asPageId(n.id))}
          >{n.title}</button>
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
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  header {
    color: var(--v4-ink);
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.7px;
  }
  input {
    background: var(--v4-surface-lo);
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink);
    border-radius: 5px;
    padding: 4px 8px;
    font-family: var(--v4-mono);
    font-size: 11px;
  }
  input:focus {
    border-color: var(--v4-accent);
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li button {
    background: transparent;
    border: 1px solid transparent;
    color: var(--v4-ink2);
    text-align: left;
    padding: 3px 6px;
    border-radius: 4px;
    cursor: pointer;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--v4-mono);
    font-size: 11px;
  }
  li button:hover {
    border-color: var(--v4-hair);
    color: var(--v4-ink);
  }
  .muted {
    color: var(--v4-ink5);
  }
</style>
