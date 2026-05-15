<script lang="ts">
  /*
   * Prism v4 context-pane tab — pages that link to the followed note.
   * v4-native (the legacy version is inline in BottomDrawer.svelte and
   * coupled to the drawer's region/keyboard model). Union of the
   * backlinks API + the link graph's incoming edges, matching the
   * legacy `allBacklinkSources` derivation.
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  let {
    noteId,
    onOpenNote,
  }: {
    noteId: string | undefined;
    onOpenNote: (noteId: string) => void;
  } = $props();

  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", noteId] as const,
    queryFn: () => api.getBacklinks(noteId as string),
    enabled: !!noteId,
  }));
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: !!noteId,
  }));

  const sources = $derived.by(() => {
    if (!noteId) return [];
    const fromApi = ((backlinksQuery.data ?? []) as Link[]).map((l) => l.target);
    const fromEdges = ((edgesQuery.data ?? []) as GraphEdge[])
      .filter((e) => e.target.toLowerCase() === noteId.toLowerCase())
      .map((e) => e.source);
    return [...new Set([...fromApi, ...fromEdges])];
  });
</script>

{#if !noteId}
  <p class="v4-ctx-empty">no note focused</p>
{:else if backlinksQuery.isLoading}
  <p class="v4-ctx-empty">loading…</p>
{:else if sources.length === 0}
  <p class="v4-ctx-empty">no pages link here</p>
{:else}
  <ul class="v4-ctx-list">
    {#each sources as src (src)}
      <li>
        <button type="button" class="v4-ctx-row" onclick={() => onOpenNote(src)}>
          {src}
        </button>
      </li>
    {/each}
  </ul>
{/if}
