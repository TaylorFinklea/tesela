<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  let { noteId, collapsed, onToggle }: { noteId: string; collapsed: boolean; onToggle: () => void } = $props();

  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", noteId] as const,
    queryFn: () => api.getBacklinks(noteId),
    enabled: !collapsed && noteId !== "",
  }));

  const forwardLinksQuery = createQuery(() => ({
    queryKey: ["forward-links", noteId] as const,
    queryFn: () => api.getForwardLinks(noteId),
    enabled: !collapsed && noteId !== "",
  }));

  // Also get edges to find backlinks by target name (the API might use different casing)
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: !collapsed,
  }));

  const backlinks: Link[] = $derived((backlinksQuery.data ?? []) as Link[]);
  const forwardLinks: Link[] = $derived((forwardLinksQuery.data ?? []) as Link[]);
  const edges: GraphEdge[] = $derived((edgesQuery.data ?? []) as GraphEdge[]);

  // Supplement backlinks from edges (in case the direct API returns empty due to casing)
  const incomingFromEdges = $derived(
    edges.filter((e) => e.target.toLowerCase() === noteId.toLowerCase() || e.target === noteId)
      .map((e) => e.source)
  );

  const allBacklinkSources = $derived.by(() => {
    const fromApi = new Set(backlinks.map((l) => l.target));
    const combined = new Set([...fromApi, ...incomingFromEdges]);
    return [...combined];
  });
</script>

{#if collapsed}
  <div class="w-10 bg-surface border-l border-border flex flex-col items-center pt-3">
    <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-[10px] p-1.5 rounded hover:bg-accent transition-colors" title="Show right panel">◀</button>
  </div>
{:else}
  <div class="w-56 bg-surface border-l border-border flex flex-col shrink-0 overflow-y-auto">
    <div class="flex items-center justify-between px-3 h-11 border-b border-border shrink-0">
      <span class="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">Links</span>
      <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-[10px] p-1 rounded hover:bg-accent transition-colors" title="Hide right panel">▶</button>
    </div>

    <!-- Backlinks -->
    <div class="px-3 py-2">
      <div class="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-1">
        Backlinks ({allBacklinkSources.length})
      </div>
      {#if allBacklinkSources.length === 0}
        <div class="text-xs text-muted-foreground/60 italic">No pages link here</div>
      {:else}
        {#each allBacklinkSources as source}
          <a
            href="/p/{encodeURIComponent(source.toLowerCase())}"
            class="block text-xs py-0.5 text-muted-foreground hover:text-foreground hover:bg-accent/50 rounded px-1"
          >
            {source}
          </a>
        {/each}
      {/if}
    </div>

    <!-- Forward Links -->
    <div class="px-3 py-2 border-t border-border/50">
      <div class="text-[10px] font-medium text-muted-foreground uppercase tracking-wider mb-1">
        Forward Links ({forwardLinks.length})
      </div>
      {#if forwardLinks.length === 0}
        <div class="text-xs text-muted-foreground/60 italic">No outgoing links</div>
      {:else}
        {#each forwardLinks as link}
          <a
            href="/p/{encodeURIComponent(link.target.toLowerCase())}"
            class="block text-xs py-0.5 text-muted-foreground hover:text-foreground hover:bg-accent/50 rounded px-1"
          >
            {link.target}
          </a>
        {/each}
      {/if}
    </div>
  </div>
{/if}
