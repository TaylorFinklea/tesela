<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  let { noteId, collapsed, onToggle }: { noteId: string; collapsed: boolean; onToggle: () => void } = $props();

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: !collapsed && noteId !== "",
  }));

  const note: Note | undefined = $derived(noteQuery.data as Note | undefined);

  // Extract custom properties from content (key:: value lines)
  const customProperties = $derived.by(() => {
    if (!note) return [];
    const props: { key: string; value: string }[] = [];
    const lines = note.content.split("\n");
    for (const line of lines) {
      const match = line.trim().match(/^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/);
      if (match) {
        const key = match[1];
        // Skip duplicates (take first occurrence)
        if (!props.some((p) => p.key.toLowerCase() === key.toLowerCase())) {
          props.push({ key, value: match[2] });
        }
      }
    }
    return props;
  });

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
  <div class="w-10 bg-surface border-l border-border flex flex-col items-center pt-4">
    <button onclick={onToggle} class="text-muted-foreground hover:text-primary text-[10px] p-1.5 rounded-md hover:bg-muted transition-all" title="Show right panel">◀</button>
  </div>
{:else}
  <div class="w-[200px] bg-surface border-l border-border flex flex-col shrink-0 overflow-y-auto">
    <div class="flex items-center justify-between px-4 h-[52px] border-b border-border shrink-0">
      <span class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em]">Details</span>
      <button onclick={onToggle} class="text-muted-foreground hover:text-primary text-[10px] p-1 rounded-md hover:bg-muted transition-all" title="Hide right panel">▶</button>
    </div>

    <!-- Properties -->
    {#if note}
      <div class="px-4 py-3">
        <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">Properties</div>

        <!-- Tags -->
        {#if note.metadata.tags.length > 0}
          <div class="mb-2">
            <div class="text-[10px] text-muted-foreground/50 mb-1">Tags</div>
            <div class="flex flex-wrap gap-1">
              {#each note.metadata.tags as tag}
                <a
                  href="/p/{encodeURIComponent(tag)}"
                  class="text-[10px] px-1.5 py-px rounded-full bg-primary/10 text-primary/80 hover:text-primary transition-colors"
                >{tag}</a>
              {/each}
            </div>
          </div>
        {/if}

        <!-- Type -->
        {#if note.metadata.note_type}
          <div class="mb-2">
            <div class="text-[10px] text-muted-foreground/50 mb-0.5">Type</div>
            <div class="text-[11px] text-foreground/70">{note.metadata.note_type}</div>
          </div>
        {/if}

        <!-- Custom properties -->
        {#if customProperties.length > 0}
          {#each customProperties as prop}
            <div class="mb-1.5">
              <div class="text-[10px] text-muted-foreground/50 mb-0.5">{prop.key}</div>
              <div class="text-[11px] text-foreground/70 break-words">{prop.value}</div>
            </div>
          {/each}
        {/if}

        {#if note.metadata.tags.length === 0 && !note.metadata.note_type && customProperties.length === 0}
          <div class="text-[11px] text-muted-foreground/40 italic">No properties</div>
        {/if}
      </div>
    {/if}

    <!-- Backlinks -->
    <div class="px-4 py-3">
      <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">
        Backlinks ({allBacklinkSources.length})
      </div>
      {#if allBacklinkSources.length === 0}
        <div class="text-[11px] text-muted-foreground/50 italic">No pages link here</div>
      {:else}
        {#each allBacklinkSources as source}
          <a
            href="/p/{encodeURIComponent(source.toLowerCase())}"
            class="block text-[12px] py-1 text-primary/60 hover:text-primary rounded-md px-1 transition-colors"
          >
            {source}
          </a>
        {/each}
      {/if}
    </div>

    <!-- Forward Links -->
    <div class="px-4 py-3 border-t border-border/30">
      <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">
        Forward ({forwardLinks.length})
      </div>
      {#if forwardLinks.length === 0}
        <div class="text-[11px] text-muted-foreground/50 italic">No outgoing links</div>
      {:else}
        {#each forwardLinks as link}
          <a
            href="/p/{encodeURIComponent(link.target.toLowerCase())}"
            class="block text-[12px] py-1 text-primary/60 hover:text-primary rounded-md px-1 transition-colors"
          >
            {link.target}
          </a>
        {/each}
      {/if}
    </div>
  </div>
{/if}
