<script lang="ts">
  /* Local graph of focused page: 1-hop neighborhood. For Phase 4 this
   * renders the full GraphCanvas with all notes/edges loaded; a true
   * 1-hop filter lands in Phase 10 when cascade sizing comes online. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import GraphCanvas from "$lib/components/GraphCanvas.svelte";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";
  import type { Note } from "$lib/types/Note";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  let {
    onNavigate,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();

  // Raised 500→5000 (tesela-sclr.1): renders the whole graph, so 500
  // silently hid notes/nodes past #500 from the canvas.
  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
  }));
  const notes = $derived((notesQuery.data ?? []) as Note[]);
  const edges = $derived((edgesQuery.data ?? []) as GraphEdge[]);
</script>

<div class="v5-graph-host">
  <GraphCanvas
    {notes}
    {edges}
    onNodePick={(id) =>
      onNavigate({ kind: "open-page", path: id, how: "replace" })}
  />
</div>

<style>
  .v5-graph-host {
    flex: 1;
    min-height: 0;
    position: relative;
    overflow: hidden;
  }
</style>
