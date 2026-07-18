<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { commandRegistry } from "$lib/command-registry.svelte";
  import GrRow from "$lib/graphite/GrRow.svelte";
  import {
    flattenRailQueryRows,
    RAIL_PROJECTION_QUERY_KEY,
    type RailQueryProjection,
  } from "$lib/graphite/rail-widget-layout";

  let { projection }: { projection: RailQueryProjection } = $props();

  const resultsQuery = createQuery(() => ({
    queryKey: [
      RAIL_PROJECTION_QUERY_KEY,
      projection.id,
      projection.definitionRevision,
      projection.dsl,
      projection.group,
      projection.sort,
    ] as const,
    queryFn: () => api.executeQuery(projection.dsl, projection.group, projection.sort),
    enabled: projection.dsl.trim().length > 0,
  }));
  const rows = $derived(flattenRailQueryRows(resultsQuery.data));
  const total = $derived(
    resultsQuery.data?.groups.reduce((sum, group) => sum + group.items.length, 0) ?? 0,
  );

  function run(id: string, arg?: string) {
    void commandRegistry.get(id)?.run(arg);
  }

  function errorText(): string {
    const error = resultsQuery.error;
    return error instanceof Error ? error.message : "Query failed";
  }
</script>

{#if projection.dsl.trim().length === 0}
  <div class="state empty">No query configured</div>
{:else if resultsQuery.isLoading}
  <div class="state">Loading…</div>
{:else if resultsQuery.isError && !resultsQuery.data}
  <div class="state error" role="alert">{errorText()}</div>
  <button
    type="button"
    class="retry"
    data-rail-action=""
    data-command-id="rail-refresh-widget"
    onclick={() => run("rail-refresh-widget", projection.id)}
  >Retry</button>
{:else if rows.length === 0}
  <div class="state empty">No matches</div>
{:else}
  {#if resultsQuery.isFetching}<div class="freshness">Refreshing…</div>{/if}
  {#if resultsQuery.isError}<div class="freshness error">Showing saved results · refresh failed</div>{/if}
  {#each rows as row (row.block_id ?? row.page_id)}
    <GrRow
      icon={row.kind === "page" ? "file-text" : "circle-dot"}
      label={row.text || row.title || row.page_id}
      meta={row.block_id ? row.title : undefined}
      data-rail-action=""
      data-command-id="jump"
      onclick={() => run("jump", row.page_id)}
      aria-label={`Open ${row.text || row.title || row.page_id}`}
    />
  {/each}
  {#if total > rows.length}<div class="freshness">{rows.length} of {total}</div>{/if}
{/if}

<style>
  .state { padding: 6px 8px; font-size: 12px; color: var(--faint); overflow-wrap: anywhere; }
  .state.error, .freshness.error { color: var(--coral); }
  .freshness { padding: 2px 8px 4px; font: 9.5px var(--mono); color: var(--faint); }
  .retry { margin: 0 8px 5px; border: 1px solid var(--line); border-radius: 5px; background: var(--bg); color: var(--subtle); font: 10px var(--mono); cursor: pointer; }
</style>
