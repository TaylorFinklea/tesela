<script lang="ts">
  /* Ambient workspace dashboard: shows pinned pages + recent + summary
   * counts. Phase 5 ships a simple two-column layout; Phase 7's shared
   * state stores will be wired in here once they exist. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import type { Note } from "$lib/types/Note";

  let { onNavigate }: AmbientRendererProps = $props();

  const allQ = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const all = $derived((allQ.data ?? []) as Note[]);
  const recent = $derived(all.slice(0, 12));
</script>

<div class="v5-dash">
  <header><b>Workspace dashboard</b></header>
  <section>
    <h3>Recent</h3>
    {#if allQ.isLoading}
      <p class="muted">loading…</p>
    {:else}
      <ul>
        {#each recent as r (r.id)}
          <li>
            <button
              type="button"
              onclick={() =>
                onNavigate({ kind: "open-page", path: r.id, how: "replace" })}
            >
              {r.title}
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
  <section>
    <h3>Stats</h3>
    <p>{all.length} pages total</p>
  </section>
</div>

<style>
  .v5-dash {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px 14px;
    font-family: var(--v4-mono);
    font-size: 12px;
    color: var(--v4-ink2);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  header {
    color: var(--v4-ink);
    font-size: 12.5px;
  }
  h3 {
    color: var(--v4-ink);
    font-size: 11px;
    text-transform: uppercase;
    margin: 0 0 6px;
    letter-spacing: 0.5px;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  li button {
    background: transparent;
    border: 1px solid var(--v4-hair);
    border-radius: 5px;
    padding: 3px 7px;
    color: var(--v4-ink2);
    font-family: var(--v4-mono);
    font-size: 11px;
    cursor: pointer;
    text-align: left;
    width: 100%;
  }
  li button:hover {
    border-color: var(--v4-hair2);
    color: var(--v4-ink);
  }
  .muted {
    color: var(--v4-ink5);
  }
</style>
