<script lang="ts">
  /**
   * Bottom-drawer Linked Tasks tab (Phase 9.3). Shows blocks tagged Task that
   * link to the focused page. Reuses the 9.1 /search/query endpoint with the
   * 9.3 has-link: predicate.
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";

  type Props = { noteId: string };
  let { noteId }: Props = $props();

  const tasksQuery = createQuery(() => ({
    queryKey: ["linked-tasks", noteId] as const,
    queryFn: () =>
      api.executeQuery(`kind:block tag:Task has-link:${noteId}`, "status", null),
    enabled: noteId !== "",
  }));

  const groups = $derived(tasksQuery.data?.groups ?? []);
  const total = $derived(groups.reduce((acc, g) => acc + g.items.length, 0));

  function statusColor(status: string | undefined): string {
    if (!status) return "var(--v9-ink-faint)";
    const s = status.toLowerCase();
    if (s === "doing") return "var(--v9-amber)";
    if (s === "done") return "var(--v9-sage)";
    if (s === "todo") return "var(--v9-ink-2)";
    return "var(--v9-ink-3)";
  }
</script>

{#if !noteId}
  <div class="empty">No note focused</div>
{:else if tasksQuery.isLoading}
  <div class="empty">Loading…</div>
{:else if total === 0}
  <div class="empty">No tasks linked to this page.</div>
{:else}
  {#each groups as g}
    {#if g.items.length > 0}
      <div class="grp">{g.key || "—"} <span class="n">{g.items.length}</span></div>
      {#each g.items as item}
        <button
          class="task-row"
          type="button"
          onclick={() => {
            const href = item.block_id
              ? `/p/${encodeURIComponent(item.page_id)}?block=${encodeURIComponent(item.block_id)}`
              : `/p/${encodeURIComponent(item.page_id)}`;
            goto(href);
          }}
        >
          <span class="dot" style:background={statusColor(item.properties?.status)}></span>
          <span class="text">{item.text}</span>
          <span class="src">↳ {item.parent_breadcrumb.join(" / ") || item.title}</span>
        </button>
      {/each}
    {/if}
  {/each}
{/if}

<style>
  .empty {
    color: var(--v9-ink-faint);
    font-family: var(--v9-mono);
    font-size: 11px;
    padding: 4px 0;
  }
  .grp {
    font-family: var(--v9-mono); font-size: 10px;
    text-transform: uppercase; letter-spacing: 0.12em;
    color: var(--v9-ink-faint);
    padding: 8px 0 4px;
  }
  .grp .n { color: var(--v9-ink-3); margin-left: 6px; }
  .task-row {
    display: grid;
    grid-template-columns: 12px 1fr auto;
    gap: 8px;
    align-items: center;
    width: 100%;
    padding: 4px 0;
    background: transparent;
    border: none;
    border-bottom: 1px dashed var(--v9-line);
    color: var(--v9-ink);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }
  .task-row:hover { background: var(--v9-bg-3); }
  .task-row .dot {
    width: 8px; height: 8px; border-radius: 50%;
    display: inline-block;
    justify-self: center;
  }
  .task-row .text { color: var(--v9-ink); }
  .task-row .src {
    color: var(--v9-ink-faint);
    font-family: var(--v9-mono);
    font-size: 10.5px;
  }
</style>
