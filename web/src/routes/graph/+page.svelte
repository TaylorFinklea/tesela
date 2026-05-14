<script lang="ts">
  import { goto } from "$app/navigation";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { GraphEdge } from "$lib/types/GraphEdge";
  import GraphCanvas from "$lib/components/GraphCanvas.svelte";

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));

  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
  }));

  const allNotes: Note[] = $derived((notesQuery.data ?? []) as Note[]);
  const allEdges: GraphEdge[] = $derived((edgesQuery.data ?? []) as GraphEdge[]);

  // Filters
  let filterTag = $state("");
  let maxDepth = $state(0); // 0 = show all

  const availableTags = $derived.by(() => {
    const tagSet = new Set<string>();
    for (const n of allNotes) {
      for (const t of n.metadata.tags) tagSet.add(t);
    }
    return [...tagSet].sort();
  });

  // Filtered notes/edges based on tag + depth.
  const { notes, edges } = $derived.by(() => {
    if (!filterTag && maxDepth === 0) return { notes: allNotes, edges: allEdges };

    let matchingIds: Set<string>;
    if (filterTag) {
      matchingIds = new Set(
        allNotes
          .filter((n) => n.metadata.tags.includes(filterTag))
          .map((n) => n.id.toLowerCase()),
      );
    } else {
      matchingIds = new Set(allNotes.map((n) => n.id.toLowerCase()));
    }

    if (maxDepth > 0 && filterTag) {
      let frontier = new Set(matchingIds);
      for (let d = 0; d < maxDepth; d++) {
        const next = new Set<string>();
        for (const edge of allEdges) {
          const sl = edge.source.toLowerCase();
          const tl = edge.target.toLowerCase();
          if (frontier.has(sl) && !matchingIds.has(tl)) next.add(tl);
          if (frontier.has(tl) && !matchingIds.has(sl)) next.add(sl);
        }
        for (const id of next) matchingIds.add(id);
        frontier = next;
        if (next.size === 0) break;
      }
    }

    const filteredNotes = allNotes.filter((n) =>
      matchingIds.has(n.id.toLowerCase()),
    );
    const filteredEdges = allEdges.filter(
      (e) =>
        matchingIds.has(e.source.toLowerCase()) &&
        matchingIds.has(e.target.toLowerCase()),
    );
    return { notes: filteredNotes, edges: filteredEdges };
  });
</script>

<div class="flex-1 flex flex-col">
  <header class="border-b border-border px-6 py-2.5 flex items-center gap-4 shrink-0">
    <span class="text-xs text-muted-foreground shrink-0">Graph View</span>

    <select
      bind:value={filterTag}
      class="text-[11px] bg-muted/30 border border-border/50 rounded-md px-2 py-1 text-foreground/80 outline-none focus:border-primary/40 transition-colors"
    >
      <option value="">All tags</option>
      {#each availableTags as tag}
        <option value={tag}>{tag}</option>
      {/each}
    </select>

    {#if filterTag}
      <div class="flex items-center gap-2">
        <span class="text-[10px] text-muted-foreground/60">Depth</span>
        <input
          type="range"
          min="0"
          max="5"
          bind:value={maxDepth}
          class="w-16 accent-primary"
        />
        <span class="text-[10px] text-muted-foreground font-mono w-4">{maxDepth || "∞"}</span>
      </div>
    {/if}

    {#if filterTag}
      <button
        onclick={() => { filterTag = ""; maxDepth = 0; }}
        class="text-[10px] text-muted-foreground/50 hover:text-foreground/70 transition-colors"
      >
        Clear
      </button>
    {/if}

    <span class="flex-1"></span>
    <span class="text-xs text-muted-foreground">{notes.length} notes, {edges.length} links</span>
  </header>

  <div class="flex-1 relative">
    <GraphCanvas
      {notes}
      {edges}
      onNodePick={(id) => goto(`/p/${encodeURIComponent(id)}`)}
    />
  </div>
</div>
