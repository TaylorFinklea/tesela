<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import {
    getActiveRegion,
    setActiveRegion,
    getBottomTab,
    setBottomTab,
    type BottomTab,
  } from "$lib/stores/pane-state.svelte";
  import { getFocusedBlock } from "$lib/stores/current-block.svelte";
  import { parseBlocks } from "$lib/block-parser";
  import { updateBlockProperty } from "$lib/property-update";
  import {
    buildRegistry,
    buildInheritanceMap,
    resolveTagChain,
    getVisibleChoices,
    parseHiddenChoices,
    updateFrontmatterKey,
  } from "$lib/property-registry";
  import type { PropertyDefinition, PropertyRegistry, InheritanceMap } from "$lib/property-registry";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  const queryClient = useQueryClient();

  const path = $derived(page.url.pathname);
  const noteId = $derived(path.startsWith("/p/") ? decodeURIComponent(path.slice(3)) : "");

  const focused = $derived(getActiveRegion() === "bottom");
  let rootEl = $state<HTMLElement | undefined>();
  let selectedNavIndex = $state(0);
  let panelContext = $state<"page" | "block">("page");

  const focusedBlock = $derived(getFocusedBlock());
  const tab = $derived(getBottomTab());

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: noteId !== "",
  }));
  const note: Note | undefined = $derived(noteQuery.data as Note | undefined);

  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const allNotes = $derived((allNotesQuery.data ?? []) as Note[]);
  const propertyRegistry: PropertyRegistry = $derived.by(() => buildRegistry(allNotes));
  const inheritanceMap: InheritanceMap = $derived.by(() => buildInheritanceMap(allNotes));

  function hiddenChoicesForTags(tags: string[]): Record<string, string[]> {
    const merged: Record<string, string[]> = {};
    const resolved = new Set<string>();
    for (const tag of tags) {
      for (const t of resolveTagChain(tag, inheritanceMap)) resolved.add(t);
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

  const hiddenChoices = $derived.by(() => {
    if (!note) return {};
    if (note.metadata.note_type === "Tag") return parseHiddenChoices(note.metadata.custom);
    return hiddenChoicesForTags(note.metadata.tags);
  });
  const blockHiddenChoices = $derived.by(() => {
    if (!focusedBlock) return {};
    const direct = focusedBlock.tags;
    const inherited = focusedBlock.inherited_tags ?? [];
    const allBlockTags = [...new Set([...direct, ...inherited])];
    const tags = allBlockTags.length > 0 ? allBlockTags : (note?.metadata.tags ?? []);
    return hiddenChoicesForTags(tags);
  });

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
    "title",
  ]);

  const customProperties = $derived.by(() => {
    if (!note) return [];
    const out: { key: string; value: string }[] = [];
    for (const [key, value] of Object.entries(note.metadata.custom)) {
      const lower = key.toLowerCase();
      if (HIDDEN_PAGE_KEYS.has(lower)) continue;
      if (lower.startsWith("hidden_")) continue;
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

  // Backlinks
  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", noteId] as const,
    queryFn: () => api.getBacklinks(noteId),
    enabled: noteId !== "",
  }));
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: noteId !== "",
  }));
  const backlinks: Link[] = $derived((backlinksQuery.data ?? []) as Link[]);
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

  // Outline = top-level blocks of focused note (or the drilled subtree)
  const noteBody = $derived.by(() => {
    if (!note) return "";
    const c = note.content;
    if (!c.startsWith("---")) return c;
    const end = c.indexOf("---", 3);
    if (end === -1) return c;
    const after = c.slice(end + 3);
    return after.startsWith("\n") ? after.slice(1) : after;
  });
  const outlineBlocks = $derived(note ? parseBlocks(note.id, noteBody) : []);

  // Edit state for properties tab
  let editingKey = $state<string | null>(null);
  let editingValue = $state("");
  let editingBlockKey = $state<string | null>(null);
  let editingBlockValue = $state("");

  async function savePageProperty(key: string, newValue: string) {
    editingKey = null;
    if (!note || newValue.trim() === "") return;
    const serialized = `"${newValue.trim().replace(/"/g, '\\"')}"`;
    const updated = await api.updateNote(noteId, updateFrontmatterKey(note.content, key, serialized));
    queryClient.setQueryData(["note", noteId], updated);
  }
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
  function isSelectType(def: PropertyDefinition | undefined): boolean {
    return def?.value_type === "select" || def?.value_type === "multi-select";
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
  function handlePageKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") { e.preventDefault(); savePageProperty(key, editingValue); }
    else if (e.key === "Escape") { editingKey = null; }
  }
  function handleBlockKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") { e.preventDefault(); saveBlockProperty(key, editingBlockValue); }
    else if (e.key === "Escape") { editingBlockKey = null; }
  }

  // Force pg when no focused block
  $effect(() => {
    if (!focusedBlock && panelContext === "block") panelContext = "page";
  });

  type TabSpec = { id: BottomTab; label: string; n: number };
  const tabSpecs = $derived<TabSpec[]>([
    { id: "backlinks", label: "Backlinks", n: allBacklinkSources.length },
    { id: "properties", label: "Properties", n: customProperties.length + blockProperties.length },
    { id: "outline", label: "Outline", n: outlineBlocks.length },
    { id: "history", label: "History", n: 0 },
    { id: "linkedTasks", label: "Linked tasks", n: 0 },
  ]);

  function cycleTab(direction: 1 | -1) {
    const idx = tabSpecs.findIndex((t) => t.id === tab);
    const next = (idx + direction + tabSpecs.length) % tabSpecs.length;
    setBottomTab(tabSpecs[next].id);
  }

  $effect(() => {
    if (focused) {
      if (rootEl && document.activeElement !== rootEl) rootEl.focus();
    } else if (rootEl && document.activeElement === rootEl) {
      rootEl.blur();
    }
  });

  $effect(() => {
    if (selectedNavIndex >= allBacklinkSources.length) {
      selectedNavIndex = Math.max(0, allBacklinkSources.length - 1);
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!focused) return;
    if (e.key === "Tab") {
      e.preventDefault();
      cycleTab(e.shiftKey ? -1 : 1);
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      setActiveRegion("focus");
      return;
    }
    if (tab === "backlinks") {
      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        selectedNavIndex = Math.min(allBacklinkSources.length - 1, selectedNavIndex + 1);
      } else if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        selectedNavIndex = Math.max(0, selectedNavIndex - 1);
      } else if (e.key === "Enter" && allBacklinkSources[selectedNavIndex]) {
        e.preventDefault();
        const src = allBacklinkSources[selectedNavIndex];
        goto(`/p/${encodeURIComponent(src.toLowerCase())}`);
        setActiveRegion("focus");
      }
    }
  }

  function clickOutline(blockId: string) {
    if (!note) return;
    goto(`/p/${encodeURIComponent(note.id)}?block=${encodeURIComponent(blockId)}`);
    setActiveRegion("focus");
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={rootEl}
  class="v9-bottom"
  tabindex="0"
  onfocus={() => setActiveRegion("bottom")}
  onclick={() => setActiveRegion("bottom")}
  onkeydown={handleKeydown}
  style="outline: none;"
>
  <div class="tabs">
    {#each tabSpecs as t}
      <span
        class="tab {t.id === tab ? 'active' : ''}"
        onclick={(e) => { e.stopPropagation(); setBottomTab(t.id); setActiveRegion("bottom"); }}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setBottomTab(t.id); } }}
        role="tab"
        tabindex="-1"
      >
        {t.label} <span class="n">{t.n}</span>
      </span>
    {/each}
  </div>
  <div class="body">
    {#if !noteId}
      <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No note focused</div>
    {:else if tab === "backlinks"}
      {#if allBacklinkSources.length === 0}
        <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No pages link here</div>
      {:else}
        {#each allBacklinkSources as src, bi}
          {@const sel = focused && selectedNavIndex === bi}
          <div
            class="v9-bl-card"
            style="cursor: pointer; {sel ? 'background: var(--v9-bg-3);' : ''}"
            onclick={() => { goto(`/p/${encodeURIComponent(src.toLowerCase())}`); setActiveRegion("focus"); }}
            role="button"
            tabindex="-1"
          >
            <span class="src"><span class="lbl">{src}</span></span>
          </div>
        {/each}
      {/if}
    {:else if tab === "properties"}
      <!-- pg/blk segmented -->
      <div style="display: flex; gap: 6px; margin-bottom: 10px; font-family: var(--v9-mono); font-size: 10.5px;">
        <button
          class="pchip"
          style="cursor: pointer; {panelContext === 'page' ? 'color: var(--v9-amber); border-color: var(--v9-amber);' : ''}"
          onclick={(e) => { e.stopPropagation(); panelContext = 'page'; }}
        >
          <span class="k">view</span><span class="v">page</span>
        </button>
        <button
          class="pchip"
          style="cursor: pointer; {panelContext === 'block' ? 'color: var(--v9-amber); border-color: var(--v9-amber);' : ''} {!focusedBlock ? 'opacity: 0.4; cursor: not-allowed;' : ''}"
          onclick={(e) => { e.stopPropagation(); if (focusedBlock) panelContext = 'block'; }}
          disabled={!focusedBlock}
        >
          <span class="k">view</span><span class="v">block</span>
        </button>
      </div>

      {#if panelContext === "block"}
        {#if focusedBlock}
          {#if blockProperties.length > 0}
            <div style="display: flex; flex-wrap: wrap; gap: 6px;">
              {#each blockProperties as prop}
                {@const def = propertyRegistry.get(prop.key.toLowerCase())}
                {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, blockHiddenChoices) : []}
                <span class="pchip">
                  <span class="k">{prop.key}</span>
                  {#if def?.value_type === "checkbox"}
                    <input
                      type="checkbox"
                      checked={prop.value === "true" || prop.value === "yes"}
                      onchange={(e) => saveBlockProperty(prop.key, (e.target as HTMLInputElement).checked ? "true" : "false")}
                    />
                  {:else if isSelectType(def)}
                    <select
                      value={prop.value}
                      onchange={(e) => saveBlockProperty(prop.key, (e.target as HTMLSelectElement).value)}
                      style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-line); font-family: var(--v9-mono); font-size: 11px;"
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
                      onchange={(e) => saveBlockProperty(prop.key, (e.target as HTMLInputElement).value)}
                      style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-line); font-family: var(--v9-mono); font-size: 11px;"
                    />
                  {:else if editingBlockKey === prop.key}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      autofocus
                      type={inputTypeFor(def)}
                      bind:value={editingBlockValue}
                      onblur={() => saveBlockProperty(prop.key, editingBlockValue)}
                      onkeydown={(e) => handleBlockKeydown(e, prop.key)}
                      style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-amber); font-family: var(--v9-mono); font-size: 11px;"
                    />
                  {:else}
                    <span
                      class="v"
                      style="cursor: text;"
                      onclick={(e) => { e.stopPropagation(); editingBlockKey = prop.key; editingBlockValue = prop.value; }}
                    >{prop.value}</span>
                  {/if}
                </span>
              {/each}
            </div>
          {:else}
            <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No block properties</div>
          {/if}
        {:else}
          <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">Focus a block to see its properties</div>
        {/if}
      {:else}
        {#if note}
          <div style="display: flex; flex-wrap: wrap; gap: 6px;">
            {#if note.metadata.tags.length > 0}
              {#each note.metadata.tags as tagName}
                <a class="pchip" href="/p/{encodeURIComponent(tagName)}">
                  <span class="k">tag</span><span class="v">{tagName}</span>
                </a>
              {/each}
            {/if}
            {#each customProperties as prop}
              {@const def = propertyRegistry.get(prop.key.toLowerCase())}
              {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, hiddenChoices) : []}
              <span class="pchip">
                <span class="k">{prop.key}</span>
                {#if def?.value_type === "checkbox"}
                  <input
                    type="checkbox"
                    checked={prop.value === "true" || prop.value === "yes"}
                    onchange={(e) => savePageProperty(prop.key, (e.target as HTMLInputElement).checked ? "true" : "false")}
                  />
                {:else if isSelectType(def)}
                  <select
                    value={prop.value}
                    onchange={(e) => savePageProperty(prop.key, (e.target as HTMLSelectElement).value)}
                    style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-line); font-family: var(--v9-mono); font-size: 11px;"
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
                    style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-line); font-family: var(--v9-mono); font-size: 11px;"
                  />
                {:else if editingKey === prop.key}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    autofocus
                    type={inputTypeFor(def)}
                    bind:value={editingValue}
                    onblur={() => savePageProperty(prop.key, editingValue)}
                    onkeydown={(e) => handlePageKeydown(e, prop.key)}
                    style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-amber); font-family: var(--v9-mono); font-size: 11px;"
                  />
                {:else}
                  <span
                    class="v"
                    style="cursor: text;"
                    onclick={(e) => { e.stopPropagation(); editingKey = prop.key; editingValue = prop.value; }}
                  >{prop.value}</span>
                {/if}
              </span>
            {/each}
            {#if note.metadata.tags.length === 0 && customProperties.length === 0}
              <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No page properties</div>
            {/if}
          </div>
        {:else}
          <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">Loading…</div>
        {/if}
      {/if}
    {:else if tab === "outline"}
      {#if outlineBlocks.length === 0}
        <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">No outline</div>
      {:else}
        {#each outlineBlocks as b}
          <div
            style="padding-left: {b.indent_level * 14}px; font-size: 12px; color: var(--v9-ink-2); padding-top: 3px; padding-bottom: 3px; cursor: pointer;"
            onclick={() => clickOutline(b.id)}
            role="button"
            tabindex="-1"
          >· {b.text || "(empty)"}</div>
        {/each}
      {/if}
    {:else if tab === "history"}
      <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">Coming in 9.x</div>
    {:else if tab === "linkedTasks"}
      <div style="color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">Coming in 9.x</div>
    {/if}
  </div>
</div>
