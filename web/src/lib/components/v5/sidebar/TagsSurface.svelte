<script lang="ts">
  /* Tags surface — flat tag list extracted from note metadata.
   * Clicking a tag opens its tag page in the focused buffer. Tags page
   * resolution mirrors BufferShell's `open-tag` intent handler: the tag's
   * NoteId is the slug, lowercased. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  const q = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const all = $derived((q.data ?? []) as Note[]);
  const tags = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const n of all) {
      const ts = (n.metadata.tags ?? []) as string[];
      for (const t of ts) counts.set(t, (counts.get(t) ?? 0) + 1);
    }
    return Array.from(counts.entries()).sort((a, b) => b[1] - a[1]);
  });

  function openTag(name: string) {
    openPageInFocused(asPageId(name.toLowerCase()));
  }
</script>

<div class="v5-side-surface">
  <header>Tags</header>
  {#if q.isLoading}
    <p class="muted">loading…</p>
  {:else if tags.length === 0}
    <p class="muted">no tags</p>
  {:else}
    <ul>
      {#each tags as [name, count] (name)}
        <li>
          <button
            type="button"
            class="row"
            onclick={() => openTag(name)}
            title="open {name}"
          >
            <span class="name">#{name}</span>
            <span class="count">{count}</span>
          </button>
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
    display: block;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 4px;
    padding: 3px 6px;
    border-radius: 4px;
    background: transparent;
    border: 0;
    color: inherit;
    text-align: left;
    font-family: inherit;
    font-size: inherit;
    width: 100%;
    cursor: pointer;
  }
  .row:hover {
    background: var(--v4-surface-lo);
  }
  .name {
    color: var(--v4-ink2);
  }
  .count {
    color: var(--v4-ink5);
  }
  .muted {
    color: var(--v4-ink5);
  }
</style>
