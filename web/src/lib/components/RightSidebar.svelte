<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import { updateBlockProperty } from "$lib/property-update";
  import {
    buildRegistry,
    buildInheritanceMap,
    resolveTagChain,
    getVisibleChoices,
    parseHiddenChoices,
    updateFrontmatterKey,
    removeFrontmatterKey,
  } from "$lib/property-registry";
  import type { PropertyDefinition, PropertyRegistry, InheritanceMap } from "$lib/property-registry";

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

  let panelContext = $state<"page" | "block">("page");

  // Text-editing state for page properties
  let editingKey = $state<string | null>(null);
  let editingValue = $state("");

  // Text-editing state for block properties (text/number/url/etc types)
  let editingBlockKey = $state<string | null>(null);
  let editingBlockValue = $state("");

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: !collapsed && noteId !== "",
  }));
  const note: Note | undefined = $derived(noteQuery.data as Note | undefined);

  // All notes — used to build the property registry
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: !collapsed,
  }));

  const propertyRegistry: PropertyRegistry = $derived.by(() => {
    const notes = (allNotesQuery.data ?? []) as Note[];
    return buildRegistry(notes);
  });

  const inheritanceMap: InheritanceMap = $derived.by(() => {
    const notes = (allNotesQuery.data ?? []) as Note[];
    return buildInheritanceMap(notes);
  });

  // Resolve hidden choices from a list of tag names, walking extends chains
  function hiddenChoicesForTags(tags: string[]): Record<string, string[]> {
    const allNotes = (allNotesQuery.data ?? []) as Note[];
    const merged: Record<string, string[]> = {};
    const resolved = new Set<string>();
    for (const tag of tags) {
      for (const t of resolveTagChain(tag, inheritanceMap)) {
        resolved.add(t);
      }
    }
    for (const tagName of resolved) {
      const tagPage = allNotes.find(
        (n) => n.title.toLowerCase() === tagName && n.metadata.note_type === "Tag",
      );
      if (tagPage) {
        const tagHidden = parseHiddenChoices(tagPage.metadata.custom);
        for (const [key, vals] of Object.entries(tagHidden)) {
          merged[key] = [...(merged[key] ?? []), ...vals];
        }
      }
    }
    return merged;
  }

  // Page panel: hidden choices from the note's own tag pages
  const hiddenChoices = $derived.by(() => {
    if (!note) return {};
    if (note.metadata.note_type === "Tag") return parseHiddenChoices(note.metadata.custom);
    return hiddenChoicesForTags(note.metadata.tags);
  });

  // Block panel: hidden choices from block's own tags + inherited tags, falling back to note's tags
  const blockHiddenChoices = $derived.by(() => {
    if (!focusedBlock) return {};
    const direct = focusedBlock.tags;
    const inherited = focusedBlock.inherited_tags ?? [];
    const allBlockTags = [...new Set([...direct, ...inherited])];
    const tags = allBlockTags.length > 0 ? allBlockTags : (note?.metadata.tags ?? []);
    return hiddenChoicesForTags(tags);
  });

  /**
   * System keys that are managed via dedicated UI on Tag/Property pages
   * (TagPropertyConfig, PropertyTypeConfig). They live in frontmatter but
   * shouldn't render as generic edit chips here.
   */
  const HIDDEN_PAGE_KEYS = new Set([
    "extends",
    "tag_properties",
    "value_type",
    "choices",
    "default",
    "hide_by_default",
    "hide_empty",
    "icon",
    "color",
    "title", // standard field, also stored separately
  ]);

  // Page custom properties — drawn from frontmatter `metadata.custom`. We
  // explicitly DON'T body-scan, so block-level `key:: value` continuation
  // lines (query::, view::, etc.) don't pollute the page property pane.
  const customProperties = $derived.by(() => {
    if (!note) return [];
    const out: { key: string; value: string }[] = [];
    for (const [key, value] of Object.entries(note.metadata.custom)) {
      const lower = key.toLowerCase();
      if (HIDDEN_PAGE_KEYS.has(lower)) continue;
      if (lower.startsWith("hidden_")) continue; // per-tag hidden choice maps
      // Only render scalar values; arrays/objects don't have a sensible inline
      // edit affordance and typically have their own UI.
      if (typeof value === "string" || typeof value === "number" || typeof value === "boolean") {
        out.push({ key, value: String(value) });
      }
    }
    return out;
  });

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
    edges
      .filter((e) => e.target.toLowerCase() === noteId.toLowerCase() || e.target === noteId)
      .map((e) => e.source),
  );
  const allBacklinkSources = $derived.by(() => {
    const fromApi = new Set(backlinks.map((l) => l.target));
    return [...new Set([...fromApi, ...incomingFromEdges])];
  });

  // Page property save — writes to frontmatter via the canonical helper.
  async function savePageProperty(key: string, newValue: string) {
    editingKey = null;
    if (!note || newValue.trim() === "") return;
    // Quote the value so it serializes safely (handles spaces, colons, etc.).
    const serialized = `"${newValue.trim().replace(/"/g, '\\"')}"`;
    const updated = await api.updateNote(
      noteId,
      updateFrontmatterKey(note.content, key, serialized),
    );
    queryClient.setQueryData(["note", noteId], updated);
  }

  // Block property save
  async function saveBlockProperty(key: string, newValue: string) {
    editingBlockKey = null;
    if (!focusedBlock || newValue.trim() === "") return;
    await updateBlockProperty({
      block: focusedBlock,
      propKey: key,
      value: newValue.trim(),
      tagName: note?.metadata.note_type === "Tag" ? (note.title ?? "") : "",
      queryClient,
    });
  }

  function handlePageKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") { e.preventDefault(); savePageProperty(key, editingValue); }
    else if (e.key === "Escape") { editingKey = null; }
  }

  function handleBlockKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") { e.preventDefault(); saveBlockProperty(key, editingBlockValue); }
    else if (e.key === "Escape") { editingBlockKey = null; }
  }

  // Type-appropriate input component selector
  function isSelectType(def: PropertyDefinition | undefined): boolean {
    return def?.value_type === "select" || def?.value_type === "multi-select";
  }

  function isAlwaysInteractive(def: PropertyDefinition | undefined): boolean {
    return def?.value_type === "checkbox" || isSelectType(def) || def?.value_type === "date";
  }

  function inputTypeFor(def: PropertyDefinition | undefined): string {
    switch (def?.value_type) {
      case "number": return "number";
      case "url": return "url";
      case "email": return "email";
      case "phone": return "tel";
      case "date": return "date";
      default: return "text";
    }
  }

  async function convertPageType(newType: "" | "Tag" | "Property") {
    if (!note) return;
    let content = note.content;
    if (!content.startsWith("---")) {
      content = `---\ntitle: "${note.title}"\n---\n${content}`;
    }
    if (newType === "") {
      content = removeFrontmatterKey(content, "type");
    } else {
      content = updateFrontmatterKey(content, "type", `"${newType}"`);
      if (newType === "Tag" && !content.includes("tag_properties:")) {
        content = updateFrontmatterKey(content, "tag_properties", "[]");
      }
    }
    const updated = await api.updateNote(noteId, content);
    queryClient.setQueryData(["note", noteId], updated);
    queryClient.invalidateQueries({ queryKey: ["notes"] });
  }
</script>

{#if collapsed}
  <div class="w-10 bg-surface border-l border-border flex flex-col items-center pt-4">
    <button
      onclick={onToggle}
      class="text-muted-foreground hover:text-primary text-[10px] p-1.5 rounded-md hover:bg-muted transition-all"
      title="Show right panel"
    >◀</button>
  </div>
{:else}
  <div class="w-[200px] bg-surface border-l border-border flex flex-col shrink-0 overflow-y-auto">
    <!-- Header with pg/blk toggle -->
    <div class="flex items-center justify-between px-4 h-[52px] border-b border-border shrink-0">
      <div class="flex items-center gap-2">
        <span class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em]">Details</span>
        <div class="flex items-center bg-muted/40 rounded-md p-0.5">
          <button
            onclick={() => { panelContext = "page"; }}
            class="text-[9px] px-1.5 py-0.5 rounded transition-all {panelContext === 'page' ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-muted-foreground'}"
            title="Page properties"
          >pg</button>
          <button
            onclick={() => { panelContext = "block"; }}
            class="text-[9px] px-1.5 py-0.5 rounded transition-all {panelContext === 'block' ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-muted-foreground'}"
            title="Block properties"
          >blk</button>
        </div>
      </div>
      <button
        onclick={onToggle}
        class="text-muted-foreground hover:text-primary text-[10px] p-1 rounded-md hover:bg-muted transition-all"
        title="Hide right panel"
      >▶</button>
    </div>

    {#if panelContext === "block"}
      <!-- Block panel -->
      <div class="px-4 py-3 flex-1">
        <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">Block</div>
        {#if focusedBlock}
          <div class="text-[10px] text-muted-foreground/50 mb-3 break-words line-clamp-2 italic">
            "{focusedBlock.text || "(empty)"}"
          </div>
          {#if blockProperties.length > 0}
            {#each blockProperties as prop}
              {@const def = propertyRegistry.get(prop.key.toLowerCase())}
              {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, blockHiddenChoices) : []}
              <div class="mb-2">
                <div class="text-[10px] text-muted-foreground/50 mb-0.5 flex items-center gap-1">
                  {prop.key}
                  {#if def}
                    <span class="text-muted-foreground/30 text-[9px]">{def.value_type}</span>
                  {/if}
                </div>

                {#if def?.value_type === "checkbox"}
                  <!-- Checkbox toggle -->
                  <label class="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={prop.value === "true" || prop.value === "yes"}
                      onchange={(e) => saveBlockProperty(prop.key, (e.target as HTMLInputElement).checked ? "true" : "false")}
                      class="rounded border-border"
                    />
                    <span class="text-[11px] text-foreground/70">{prop.value}</span>
                  </label>

                {:else if isSelectType(def)}
                  <!-- Select dropdown — always interactive -->
                  <select
                    class="w-full text-[11px] bg-muted/60 border border-border/60 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary/60 cursor-pointer"
                    value={prop.value}
                    onchange={(e) => saveBlockProperty(prop.key, (e.target as HTMLSelectElement).value)}
                  >
                    {#if !visibleChoices.includes(prop.value)}
                      <option value={prop.value}>{prop.value}</option>
                    {/if}
                    {#each visibleChoices as choice}
                      <option value={choice}>{choice}</option>
                    {/each}
                  </select>

                {:else if def?.value_type === "date"}
                  <!-- Date input — always interactive -->
                  <input
                    type="date"
                    value={prop.value}
                    onchange={(e) => saveBlockProperty(prop.key, (e.target as HTMLInputElement).value)}
                    class="w-full text-[11px] bg-muted/60 border border-border/60 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary/60"
                  />

                {:else if editingBlockKey === prop.key}
                  <!-- Text/number/url/etc — edit mode -->
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    autofocus
                    type={inputTypeFor(def)}
                    class="w-full text-[11px] bg-muted/60 border border-primary/40 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary"
                    bind:value={editingBlockValue}
                    onblur={() => saveBlockProperty(prop.key, editingBlockValue)}
                    onkeydown={(e) => handleBlockKeydown(e, prop.key)}
                  />

                {:else}
                  <!-- Text display — click to edit -->
                  <!-- svelte-ignore a11y_click_events_have_key_events -->
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="text-[11px] text-foreground/70 break-words px-1 -mx-1 rounded hover:bg-muted/60 cursor-text transition-colors"
                    onclick={() => { editingBlockKey = prop.key; editingBlockValue = prop.value; }}
                    title="Click to edit"
                  >{prop.value}</div>
                {/if}
              </div>
            {/each}
          {:else}
            <div class="text-[11px] text-muted-foreground/40 italic">No properties</div>
          {/if}
        {:else}
          <div class="text-[11px] text-muted-foreground/40 italic">Focus a block to see its properties</div>
        {/if}
      </div>

    {:else}
      <!-- Page panel -->
      {#if note}
        <div class="px-4 py-3">
          <div class="text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] mb-2">Properties</div>

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

          <div class="mb-2">
            <div class="text-[10px] text-muted-foreground/50 mb-0.5">Type</div>
            <select
              class="w-full text-[11px] bg-muted/60 border border-border/60 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary/60 cursor-pointer"
              value={note.metadata.note_type ?? ""}
              onchange={(e) => convertPageType((e.target as HTMLSelectElement).value as "" | "Tag" | "Property")}
            >
              <option value="">Page</option>
              <option value="Tag">Tag</option>
              <option value="Property">Property</option>
            </select>
          </div>

          {#each customProperties as prop}
            {@const def = propertyRegistry.get(prop.key.toLowerCase())}
            {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, hiddenChoices) : []}
            <div class="mb-1.5">
              <div class="text-[10px] text-muted-foreground/50 mb-0.5 flex items-center gap-1">
                {prop.key}
                {#if def}
                  <span class="text-muted-foreground/30 text-[9px]">{def.value_type}</span>
                {/if}
              </div>

              {#if def?.value_type === "checkbox"}
                <label class="flex items-center gap-2 cursor-pointer">
                  <input
                    type="checkbox"
                    checked={prop.value === "true" || prop.value === "yes"}
                    onchange={(e) => savePageProperty(prop.key, (e.target as HTMLInputElement).checked ? "true" : "false")}
                    class="rounded border-border"
                  />
                  <span class="text-[11px] text-foreground/70">{prop.value}</span>
                </label>

              {:else if isSelectType(def)}
                <select
                  class="w-full text-[11px] bg-muted/60 border border-border/60 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary/60 cursor-pointer"
                  value={prop.value}
                  onchange={(e) => savePageProperty(prop.key, (e.target as HTMLSelectElement).value)}
                >
                  {#if !visibleChoices.includes(prop.value)}
                    <option value={prop.value}>{prop.value}</option>
                  {/if}
                  {#each visibleChoices as choice}
                    <option value={choice}>{choice}</option>
                  {/each}
                </select>

              {:else if def?.value_type === "date"}
                <input
                  type="date"
                  value={prop.value}
                  onchange={(e) => savePageProperty(prop.key, (e.target as HTMLInputElement).value)}
                  class="w-full text-[11px] bg-muted/60 border border-border/60 rounded px-1.5 py-0.5 text-foreground outline-none focus:border-primary/60"
                />

              {:else if editingKey === prop.key}
                <!-- svelte-ignore a11y_autofocus -->
                <input
                  autofocus
                  type={inputTypeFor(def)}
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
                  onclick={() => { editingKey = prop.key; editingValue = prop.value; }}
                  title="Click to edit"
                >{prop.value}</div>
              {/if}
            </div>
          {/each}

          {#if note.metadata.tags.length === 0 && customProperties.length === 0}
            <div class="text-[11px] text-muted-foreground/40 italic">No properties</div>
          {/if}
        </div>
      {/if}

      <!-- Backlinks -->
      <div class="px-4 py-3 border-t border-border/30">
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
            >{source}</a>
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
            >{link.target}</a>
          {/each}
        {/if}
      </div>
    {/if}
  </div>
{/if}
