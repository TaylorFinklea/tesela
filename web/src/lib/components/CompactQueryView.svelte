<script lang="ts">
  /* Compact cascade member for `note_type: Query` pages — renders the
   * query results as a simple list of titles instead of QueryWidgetView's
   * wider table. Used when the host pane is narrower than ~50 cols.
   *
   * Reuses widgetFromNote + the same backend execution path; only the
   * presentation diverges. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import { widgetFromNote } from "$lib/widget-registry.svelte";

  let {
    note,
    onOpenRow,
  }: {
    note: Note;
    onOpenRow?: (pageId: string) => void;
  } = $props();

  const widget = $derived(widgetFromNote(note));

  // QueryWidgetView delegates to `/api/search/query`; we hit the same
  // endpoint for the compact list.
  const resultsQ = createQuery(() => ({
    queryKey: ["compact-query-results", note.id, widget?.query] as const,
    queryFn: () =>
      api.executeQuery(widget!.query, widget!.group, widget!.sort),
    enabled: !!widget?.query,
  }));

  const rows = $derived.by(() => {
    const data = resultsQ.data as unknown;
    if (!data || typeof data !== "object") return [];
    // executeQuery returns { rows: [...] } where each row has
    // {id, title, ...} for kind:page or {note_id, ...} for kind:block.
    const r = (data as { rows?: Array<Record<string, unknown>> }).rows ?? [];
    return r.slice(0, 50);
  });
</script>

<div class="compact-q">
  <header>{note.title}</header>
  {#if !widget}
    <p class="muted">not a query note</p>
  {:else if resultsQ.isLoading}
    <p class="muted">loading…</p>
  {:else if rows.length === 0}
    <p class="muted">no results</p>
  {:else}
    <ul>
      {#each rows as r (String((r.id ?? r.note_id) ?? Math.random()))}
        {@const id = String(r.id ?? r.note_id ?? "")}
        {@const title = String(r.title ?? r.text ?? id)}
        <li>
          <button type="button" onclick={() => id && onOpenRow?.(id)}>
            {title}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .compact-q {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 12px;
    font-family: var(--theme-font-mono);
    color: var(--fg-muted);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  header {
    color: var(--fg-default);
    font-size: 11.5px;
    text-transform: uppercase;
    letter-spacing: 0.6px;
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
    color: var(--fg-muted);
    text-align: left;
    padding: 3px 6px;
    border-radius: 4px;
    cursor: pointer;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--theme-font-mono);
    font-size: 11px;
  }
  li button:hover {
    border-color: var(--line-soft);
    color: var(--fg-default);
  }
  .muted {
    color: var(--fg-faint);
    font-size: 11px;
  }
</style>
