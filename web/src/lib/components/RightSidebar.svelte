<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import { updateBlockProperty } from "$lib/property-update";

  let {
    noteId,
    collapsed,
    onToggle,
    focusedBlock = null,
  }: {
    noteId: string;
    collapsed: boolean;
    onToggle: () => void;
    focusedBlock?: ParsedBlock | null;
  } = $props();

  const queryClient = useQueryClient();

  // "page" = page properties panel, "block" = focused block properties
  let panelContext = $state<"page" | "block">("page");

  // Inline editing state for page properties
  let editingKey = $state<string | null>(null);
  let editingValue = $state("");

  // Inline editing state for block properties
  let editingBlockKey = $state<string | null>(null);
  let editingBlockValue = $state("");

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
        if (!props.some((p) => p.key.toLowerCase() === key.toLowerCase())) {
          props.push({ key, value: match[2] });
        }
      }
    }
    return props;
  });

  // Block properties as array
  const blockProperties = $derived.by(() => {
    if (!focusedBlock) return [];
    return Object.entries(focusedBlock.properties).map(([key, value]) => ({ key, value }));
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

  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: !collapsed,
  }));

  const backlinks: Link[] = $derived((backlinksQuery.data ?? []) as Link[]);
  const forwardLinks: Link[] = $derived((forwardLinksQuery.data ?? []) as Link[]);
  const edges: GraphEdge[] = $derived((edgesQuery.data ?? []) as GraphEdge[]);

  const incomingFromEdges = $derived(
    edges.filter((e) => e.target.toLowerCase() === noteId.toLowerCase() || e.target === noteId)
      .map((e) => e.source)
  );

  const allBacklinkSources = $derived.by(() => {
    const fromApi = new Set(backlinks.map((l) => l.target));
    const combined = new Set([...fromApi, ...incomingFromEdges]);
    return [...combined];
  });

  // Switch to block panel when a block is focused
  $effect(() => {
    if (focusedBlock) panelContext = "block";
    else panelContext = "page";
  });

  async function savePageProperty(key: string, newValue: string) {
    editingKey = null;
    if (!note || newValue.trim() === "") return;
    const lines = note.content.split("\n");
    for (let i = 0; i < lines.length; i++) {
      const trimmed = lines[i].trim();
      const match = trimmed.match(/^([A-Za-z_][A-Za-z0-9_]*):: (.+)$/);
      if (match && match[1].toLowerCase() === key.toLowerCase()) {
        const indent = lines[i].length - lines[i].trimStart().length;
        lines[i] = " ".repeat(indent) + `${match[1]}:: ${newValue.trim()}`;
        break;
      }
    }
    const updated = await api.updateNote(noteId, lines.join("\n"));
    queryClient.setQueryData(["note", noteId], updated);
  }

  async function saveBlockProperty(key: string, newValue: string) {
    editingBlockKey = null;
    if (!focusedBlock || newValue.trim() === "") return;
    await updateBlockProperty({
      block: focusedBlock,
      propKey: key,
      value: newValue.trim(),
      tagName: "",
      queryClient,
    });
  }

  function startEditPage(key: string, current: string) {
    editingKey = key;
    editingValue = current;
  }

  function startEditBlock(key: string, current: string) {
    editingBlockKey = key;
    editingBlockValue = current;
  }

  function handlePageKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") { e.preventDefault(); savePageProperty(key, editingValue); }
    else if (e.key === "Escape") { editingKey = null; }
  }

  function handleBlockKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") { e.preventDefault(); saveBlockProperty(key, editingBlockValue); }
    else if (e.key === "Escape") { editingBlockKey = null; }
  }
</script>

{#if collapsed}
  <div class="w-10 bg-surface border-l border-border flex flex-col items-center pt-4">
    <button onclick={onToggle} class="text-muted-foreground hover:text-primary text-[10px] p-1.5 rounded-md hover:bg-muted transition-all" title="Show right panel">◀</button>
  </div>
{:else}
  <div class="w-[200px] bg-surface border-l border-border flex flex-col shrink-0 overflow-y-auto">
    <div class="flex items-center justify-between px-4 h-[52px] border-b border-border shrink-0">
      <div class="flex items-center gap-2">
        <span class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em]">Details</span>
        <!-- Panel context toggle -->
        <div class="flex items-center bg-muted/40 rounded-md p-0.5">
          <button
            onclick={() => { panelContext = "page"; }}
            class="text-[9px] px-1.5 py-0.5 rounded transition-all {panelContext === 'page' ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-muted-foreground'}"
            title="Page properties"
          >pg</button>
          <button
            onclick={() => { panelContext = "block"; }}
            class="text-[9px] px-1.5 py-0.5 rounded transition-all {panelContext === 'block' ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-muted-foreground'}"
            title="Focused block properties"
          >blk</button>
        </div>
      </div>
      <button onclick={onToggle} class="text-muted-foreground hover:text-primary text-[10px] p-1 rounded-md hover:bg-muted transition-all" title="Hide right panel">▶</button>
    </div>

    {#if panelContext === "block"}
      <!-- Block context panel -->
      <div class="px-4 py-3">
        <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">Block</div>
        {#if focusedBlock}
          <div class="text-[10px] text-muted-foreground/50 mb-2 break-words line-clamp-2 italic">
            "{focusedBlock.text || "(empty)"}"
          </div>
          {#if blockProperties.length > 0}
            {#each blockProperties as prop}
              <div class="mb-1.5">
                <div class="text-[10px] text-muted-foreground/50 mb-0.5">{prop.key}</div>
                {#if editingBlockKey === prop.key}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    autofocus
                    class="w-full text-[11px] bg-muted/60 border border-primary/40 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary"
                    bind:value={editingBlockValue}
                    onblur={() => saveBlockProperty(prop.key, editingBlockValue)}
                    onkeydown={(e) => handleBlockKeydown(e, prop.key)}
                  />
                {:else}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="text-[11px] text-foreground/70 break-words px-1 -mx-1 rounded hover:bg-muted/60 cursor-text transition-colors"
                    onclick={() => startEditBlock(prop.key, prop.value)}
                    title="Click to edit"
                  >{prop.value}</div>
                {/if}
              </div>
            {/each}
          {:else}
            <div class="text-[11px] text-muted-foreground/40 italic">No properties</div>
          {/if}
        {:else}
          <div class="text-[11px] text-muted-foreground/40 italic">No block focused</div>
        {/if}
      </div>
    {:else}
      <!-- Page context panel -->
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

          <!-- Custom properties (editable) -->
          {#if customProperties.length > 0}
            {#each customProperties as prop}
              <div class="mb-1.5">
                <div class="text-[10px] text-muted-foreground/50 mb-0.5">{prop.key}</div>
                {#if editingKey === prop.key}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    autofocus
                    class="w-full text-[11px] bg-muted/60 border border-primary/40 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary"
                    bind:value={editingValue}
                    onblur={() => savePageProperty(prop.key, editingValue)}
                    onkeydown={(e) => handlePageKeydown(e, prop.key)}
                  />
                {:else}
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="text-[11px] text-foreground/70 break-words px-1 -mx-1 rounded hover:bg-muted/60 cursor-text transition-colors"
                    onclick={() => startEditPage(prop.key, prop.value)}
                    title="Click to edit"
                  >{prop.value}</div>
                {/if}
              </div>
            {/each}
          {/if}

          {#if note.metadata.tags.length === 0 && !note.metadata.note_type && customProperties.length === 0}
            <div class="text-[11px] text-muted-foreground/40 italic">No properties</div>
          {/if}
        </div>
      {/if}
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
