<script lang="ts">
  /* Ambient workspace dashboard: shows pinned pages + recent + summary
   * counts. Phase 5 ships a simple two-column layout; Phase 7's shared
   * state stores will be wired in here once they exist. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import type { Note } from "$lib/types/Note";

  let { onNavigate }: AmbientRendererProps = $props();

  // Raised 500→5000 (tesela-sclr.1): shares the ["notes", {limit}] cache key
  // with the other "all notes" surfaces; a 500 cap silently dropped notes
  // past #500 from those.
  const allQ = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
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
    font-family: var(--theme-font-mono);
    font-size: 12px;
    color: var(--fg-muted);
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  header {
    color: var(--fg-default);
    font-size: 12.5px;
  }
  h3 {
    color: var(--fg-default);
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
    border: 1px solid var(--line-soft);
    border-radius: 5px;
    padding: 3px 7px;
    color: var(--fg-muted);
    font-family: var(--theme-font-mono);
    font-size: 11px;
    cursor: pointer;
    text-align: left;
    width: 100%;
  }
  li button:hover {
    border-color: var(--line);
    color: var(--fg-default);
  }
  .muted {
    color: var(--fg-faint);
  }
</style>
