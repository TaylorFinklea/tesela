<script lang="ts">
  /* Compact cascade member for local-graph-of-page. Renders just a count
   * chip when the host doesn't have room for the full force-directed
   * graph. */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  let {
    reference,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();

  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
  }));
  const edges = $derived((edgesQuery.data ?? []) as GraphEdge[]);
  const neighborCount = $derived.by(() => {
    const p = reference.path?.toLowerCase();
    if (!p) return 0;
    const ns = new Set<string>();
    for (const e of edges) {
      if (e.source.toLowerCase() === p) ns.add(e.target.toLowerCase());
      if (e.target.toLowerCase() === p) ns.add(e.source.toLowerCase());
    }
    return ns.size;
  });
</script>

<div class="compact">
  <p class="label">local graph</p>
  <p class="value">{neighborCount} neighbor{neighborCount === 1 ? "" : "s"}</p>
  <p class="hint">resize pane for full graph</p>
</div>

<style>
  .compact {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    font-family: var(--v4-mono);
    color: var(--v4-ink2);
  }
  .label {
    font-size: 9.5px;
    color: var(--v4-ink5);
    text-transform: uppercase;
    letter-spacing: 0.7px;
    margin: 0;
  }
  .value {
    font-size: 22px;
    color: var(--v4-accent);
    margin: 0;
  }
  .hint {
    color: var(--v4-ink6);
    font-size: 10px;
    margin: 0;
  }
</style>
