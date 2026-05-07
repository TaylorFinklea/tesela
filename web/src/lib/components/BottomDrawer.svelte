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
  import { updateBlockProperty, clearBlockProperty } from "$lib/property-update";
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
  import HistoryTab from "./HistoryTab.svelte";
  import LinkedTasksTab from "./LinkedTasksTab.svelte";

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

  // When a block from a different source note is focused (e.g., viewing /p/tasks
  // query page but focusing a block from phase3gqa.md), fetch the block's source
  // note separately. If focusedBlock.note_id === noteId (block from the current page),
  // reuse the main note to avoid double-fetching.
  const blockSourceNoteQuery = createQuery(() => ({
    queryKey: ["note", focusedBlock?.note_id ?? ""] as const,
    queryFn: () => api.getNote(focusedBlock!.note_id),
    enabled: !!focusedBlock && focusedBlock.note_id !== noteId,
  }));
  const blockSourceNote: Note | undefined = $derived(
    focusedBlock && focusedBlock.note_id === noteId
      ? note
      : (blockSourceNoteQuery.data as Note | undefined),
  );

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
  function extractBody(content: string): string {
    if (!content.startsWith("---")) return content;
    const end = content.indexOf("---", 3);
    if (end === -1) return content;
    const after = content.slice(end + 3);
    return after.startsWith("\n") ? after.slice(1) : after;
  }

  // Re-derive block properties from the block's SOURCE note content, not from the
  // focused-block store snapshot. The store is set when the user focuses a
  // block in the editor; if we read its `properties` directly, an
  // updateBlockProperty save (which only writes the note content + cache)
  // doesn't refresh the chip values until the user re-focuses the block.
  // Looking the block up by id in the freshly-parsed source note body fixes that.
  // blockSourceNote is fetched from focusedBlock.note_id and handles the case
  // where a Query page is viewed but a block from a different source note is focused.
  const blockProperties = $derived.by(() => {
    if (!focusedBlock) return [];
    const sourceNote = blockSourceNote;
    if (!sourceNote) {
      // Source note hasn't loaded yet — fall back to the snapshot. This is rare
      // (only the first render before the query resolves) and matches the
      // pre-fix behavior.
      return Object.entries(focusedBlock.properties).map(([key, value]) => ({ key, value }));
    }
    const live = parseBlocks(sourceNote.id, extractBody(sourceNote.content)).find(
      (b) => b.id === focusedBlock.id,
    );
    const source = live ?? focusedBlock;
    return Object.entries(source.properties).map(([key, value]) => ({ key, value }));
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

  // Phase 9.7 — keyboard nav for the Properties tab. j/k cycles through
  // `flatProperties` (the current panel's list — block or page), Enter opens
  // edit mode for the selected chip, Tab commits + advances. Tag chips on
  // the page panel are display-only and not part of the navigation.
  let selectedPropertyIndex = $state(0);
  const flatProperties = $derived(
    panelContext === "block" ? blockProperties : customProperties,
  );
  $effect(() => {
    if (selectedPropertyIndex >= flatProperties.length) {
      selectedPropertyIndex = Math.max(0, flatProperties.length - 1);
    }
  });

  async function savePageProperty(key: string, newValue: string) {
    editingKey = null;
    if (note && newValue.trim() !== "") {
      const serialized = `"${newValue.trim().replace(/"/g, '\\"')}"`;
      const updated = await api.updateNote(noteId, updateFrontmatterKey(note.content, key, serialized));
      queryClient.setQueryData(["note", noteId], updated);
    }
    requestAnimationFrame(() => rootEl?.focus());
  }
  async function saveBlockProperty(key: string, newValue: string) {
    editingBlockKey = null;
    if (focusedBlock && newValue.trim() !== "") {
      await updateBlockProperty({
        block: focusedBlock,
        propKey: key,
        value: newValue.trim(),
        tagName: note?.metadata.note_type === "Tag" ? (note.title ?? "") : "",
        queryClient,
      });
    }
    requestAnimationFrame(() => rootEl?.focus());
  }

  function enterEditOnCurrent() {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (
      def?.value_type === "select" ||
      def?.value_type === "multi-select" ||
      def?.value_type === "date" ||
      def?.value_type === "checkbox"
    ) {
      const chip = rootEl?.querySelector(
        `[data-prop-index="${selectedPropertyIndex}"][data-prop-context="${panelContext}"]`,
      );
      const ctrl = chip?.querySelector("select, input") as HTMLElement | null;
      ctrl?.focus();
      return;
    }
    if (panelContext === "block") {
      editingBlockKey = prop.key;
      editingBlockValue = prop.value;
    } else {
      editingKey = prop.key;
      editingValue = prop.value;
    }
    requestAnimationFrame(() => {
      const chip = rootEl?.querySelector(
        `[data-prop-index="${selectedPropertyIndex}"][data-prop-context="${panelContext}"]`,
      );
      const input = chip?.querySelector("input");
      if (input instanceof HTMLInputElement) {
        input.focus();
        input.select();
      }
    });
  }
  // Vim-like one-keystroke editors. `Space` (or h/l) cycles a select's
  // value in NAV mode; `x` clears a block property; `Space` toggles a
  // checkbox. Keeps the drawer keyboard-only — no need to focus the inner
  // <select> or open a popup just to flip a status.
  function cycleSelectValue(direction: 1 | -1) {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (!def || (def.value_type !== "select" && def.value_type !== "multi-select")) return;
    const hidden = panelContext === "block" ? blockHiddenChoices : hiddenChoices;
    const choices = getVisibleChoices(def, hidden);
    if (choices.length === 0) return;
    const currentIdx = choices.indexOf(prop.value);
    const nextIdx = ((currentIdx + direction) % choices.length + choices.length) % choices.length;
    const nextVal = choices[nextIdx];
    if (panelContext === "block") void saveBlockProperty(prop.key, nextVal);
    else void savePageProperty(prop.key, nextVal);
  }

  function toggleCheckboxValue() {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    const def = propertyRegistry.get(prop.key.toLowerCase());
    if (def?.value_type !== "checkbox") return;
    const next = prop.value === "true" ? "false" : "true";
    if (panelContext === "block") void saveBlockProperty(prop.key, next);
    else void savePageProperty(prop.key, next);
  }

  function clearCurrentProperty() {
    const prop = flatProperties[selectedPropertyIndex];
    if (!prop) return;
    if (panelContext !== "block" || !focusedBlock) return;
    void clearBlockProperty({
      block: focusedBlock,
      propKey: prop.key,
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
  /**
   * Phase 10.5 — date-property values are persisted as `[[YYYY-MM-DD]]`
   * wiki-links (so they show in the daily-page backlink calendar). HTML
   * `<input type="date">` only accepts the bare `YYYY-MM-DD` form, so the
   * drawer needs to strip the brackets when reading and re-wrap when
   * writing. This pair was the missing piece behind "drawer date input
   * was empty even though the chip showed Apr 15."
   */
  function stripDateBrackets(v: string): string {
    const m = v.trim().match(/^\[\[(\d{4}-\d{2}-\d{2})\]\]$/);
    return m ? m[1] : v.trim();
  }
  function wrapDateBrackets(v: string): string {
    if (!v) return "";
    return /^\d{4}-\d{2}-\d{2}$/.test(v) ? `[[${v}]]` : v;
  }
  // Inline-input keydown contract:
  //   Enter  → commit + close edit mode, focus drawer (j/k navigates again)
  //   Esc    → bail (no save), close edit mode, focus drawer
  //   Tab    → bail too (don't move-and-advance — the user prefers explicit
  //            j/k navigation between chips)
  //   stopPropagation everywhere so the drawer's own Tab-cycles-tabs and
  //   Enter-enters-edit handlers don't double-fire on the same bubble.
  function handlePageKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      savePageProperty(key, editingValue);
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      editingKey = null;
      requestAnimationFrame(() => rootEl?.focus());
    }
  }
  function handleBlockKeydown(e: KeyboardEvent, key: string) {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      saveBlockProperty(key, editingBlockValue);
    } else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      editingBlockKey = null;
      requestAnimationFrame(() => rootEl?.focus());
    }
  }

  // Auto-track focusedBlock: when a block is focused, default to the block
  // panel (otherwise the user has to click the "view: block" segment to see
  // their block's status/priority/etc.). When no block is focused, fall
  // back to page. The user can still click the segment to override.
  let lastFocusedId = $state<string | null>(null);
  $effect(() => {
    const id = focusedBlock?.id ?? null;
    if (id !== lastFocusedId) {
      lastFocusedId = id;
      panelContext = id ? "block" : "page";
    }
  });

  // Lightweight count-only fetches for the tab badges. Cheap and reactive.
  const versionsCountQuery = createQuery(() => ({
    queryKey: ["note-versions", noteId, "count"] as const,
    queryFn: () => api.listNoteVersions(noteId, 200),
    enabled: noteId !== "",
  }));
  const versionsCount = $derived(versionsCountQuery.data?.length ?? 0);

  const linkedTasksCountQuery = createQuery(() => ({
    queryKey: ["linked-tasks", noteId, "count"] as const,
    queryFn: () =>
      api.executeQuery(`kind:block tag:Task has-link:${noteId}`, null, null),
    enabled: noteId !== "",
  }));
  const linkedTasksCount = $derived(
    linkedTasksCountQuery.data?.groups?.reduce((acc, g) => acc + g.items.length, 0) ?? 0,
  );

  type TabSpec = { id: BottomTab; label: string; n: number };
  const tabSpecs = $derived<TabSpec[]>([
    { id: "backlinks", label: "Backlinks", n: allBacklinkSources.length },
    { id: "properties", label: "Properties", n: customProperties.length + blockProperties.length },
    { id: "outline", label: "Outline", n: outlineBlocks.length },
    { id: "history", label: "History", n: versionsCount },
    { id: "linkedTasks", label: "Linked tasks", n: linkedTasksCount },
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
      // While editing a property inline, Tab is "commit + advance" — let the
      // input's onkeydown handle it (handleBlockKeydown / handlePageKeydown).
      if (editingKey !== null || editingBlockKey !== null) return;
      e.preventDefault();
      cycleTab(e.shiftKey ? -1 : 1);
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      // Blur whatever is focused inside the drawer (e.g. a `<select>` the
      // user opened to edit a property). Without this, document focus stays
      // on the drawer element and continues consuming keys via native
      // typeahead even though the active region has flipped back to "focus".
      (document.activeElement as HTMLElement | null)?.blur();
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
    } else if (tab === "properties") {
      // While editing an inline input, that input owns its keys (handlePage/BlockKeydown).
      if (editingKey !== null || editingBlockKey !== null) return;
      const prop = flatProperties[selectedPropertyIndex];
      const def = prop ? propertyRegistry.get(prop.key.toLowerCase()) : undefined;
      const isSelect = def?.value_type === "select" || def?.value_type === "multi-select";
      const isCheckbox = def?.value_type === "checkbox";

      if (e.key === "j" || e.key === "ArrowDown") {
        e.preventDefault();
        if (flatProperties.length > 0) {
          selectedPropertyIndex = Math.min(flatProperties.length - 1, selectedPropertyIndex + 1);
        }
      } else if (e.key === "k" || e.key === "ArrowUp") {
        e.preventDefault();
        if (flatProperties.length > 0) {
          selectedPropertyIndex = Math.max(0, selectedPropertyIndex - 1);
        }
      } else if (e.key === "g") {
        e.preventDefault();
        selectedPropertyIndex = 0;
      } else if (e.key === "G") {
        e.preventDefault();
        selectedPropertyIndex = Math.max(0, flatProperties.length - 1);
      } else if (e.key === " ") {
        // Space: cycle for select, toggle for checkbox, fall through to inline-edit otherwise.
        e.preventDefault();
        if (isSelect) cycleSelectValue(e.shiftKey ? -1 : 1);
        else if (isCheckbox) toggleCheckboxValue();
        else if (prop) enterEditOnCurrent();
      } else if (e.key === "l" && isSelect) {
        e.preventDefault();
        cycleSelectValue(1);
      } else if (e.key === "h" && isSelect) {
        e.preventDefault();
        cycleSelectValue(-1);
      } else if (e.key === "x") {
        e.preventDefault();
        clearCurrentProperty();
      } else if (e.key === "Enter" && prop) {
        e.preventDefault();
        enterEditOnCurrent();
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
            <div class="props-list">
              {#each blockProperties as prop, pi}
                {@const def = propertyRegistry.get(prop.key.toLowerCase())}
                {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, blockHiddenChoices) : []}
                {@const propSelected = focused && tab === "properties" && panelContext === "block" && selectedPropertyIndex === pi}
                <span
                  class="pchip {propSelected ? 'selected' : ''}"
                  data-prop-index={pi}
                  data-prop-context="block"
                >
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
                      value={stripDateBrackets(prop.value)}
                      onchange={(e) => saveBlockProperty(prop.key, wrapDateBrackets((e.target as HTMLInputElement).value))}
                      style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-line); font-family: var(--v9-mono); font-size: 11px;"
                    />
                  {:else if editingBlockKey === prop.key}
                    <!-- svelte-ignore a11y_autofocus -->
                    <input
                      autofocus
                      type={inputTypeFor(def)}
                      bind:value={editingBlockValue}
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
            {#each customProperties as prop, pi}
              {@const def = propertyRegistry.get(prop.key.toLowerCase())}
              {@const visibleChoices = def && isSelectType(def) ? getVisibleChoices(def, hiddenChoices) : []}
              {@const propSelected = focused && tab === "properties" && panelContext === "page" && selectedPropertyIndex === pi}
              <span
                class="pchip {propSelected ? 'selected' : ''}"
                data-prop-index={pi}
                data-prop-context="page"
              >
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
                    value={stripDateBrackets(prop.value)}
                    onchange={(e) => savePageProperty(prop.key, wrapDateBrackets((e.target as HTMLInputElement).value))}
                    style="background: var(--v9-bg-3); color: var(--v9-ink); border: 1px solid var(--v9-line); font-family: var(--v9-mono); font-size: 11px;"
                  />
                {:else if editingKey === prop.key}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    autofocus
                    type={inputTypeFor(def)}
                    bind:value={editingValue}
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
      <HistoryTab {noteId} />
    {:else if tab === "linkedTasks"}
      <LinkedTasksTab {noteId} />
    {/if}
  </div>
</div>
