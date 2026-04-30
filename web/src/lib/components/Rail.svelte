<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { getActiveRegion, setActiveRegion } from "$lib/stores/pane-state.svelte";
  import { parseWidgets, widgetsBySection } from "$lib/widget-registry.svelte";
  import MiniCalendar from "./MiniCalendar.svelte";
  import type { Note } from "$lib/types/Note";
  import type { Widget, WidgetSection } from "$lib/types/Widget";

  const railFocused = $derived(getActiveRegion() === "rail");
  let rootEl = $state<HTMLElement | undefined>();
  let selectedIndex = $state(-1);

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));
  const notes = $derived((notesQuery.data ?? []) as Note[]);
  const widgets = $derived(parseWidgets(notes));
  const sections = $derived(widgetsBySection(widgets));
  const flat = $derived<Widget[]>([
    ...sections.pinned,
    ...sections.browse,
    ...sections.saved,
  ]);
  const currentPath = $derived(page.url.pathname);

  function isActive(w: Widget): boolean {
    return currentPath === `/p/${encodeURIComponent(w.id)}`;
  }

  $effect(() => {
    if (railFocused) {
      if (rootEl && document.activeElement !== rootEl) rootEl.focus();
      if (selectedIndex < 0) selectedIndex = 0;
    } else if (rootEl && document.activeElement === rootEl) {
      rootEl.blur();
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!railFocused) return;
    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(flat.length - 1, selectedIndex + 1);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && flat[selectedIndex]) {
      e.preventDefault();
      goto(`/p/${encodeURIComponent(flat[selectedIndex].id)}`);
      setActiveRegion("focus");
    } else if (e.key === "Escape") {
      e.preventDefault();
      setActiveRegion("focus");
    }
  }

  function rowClass(w: Widget, idx: number): string {
    const sel = railFocused && selectedIndex === idx;
    const active = isActive(w);
    return `w ${active || sel ? "active" : ""}`;
  }

  // Default kind glyph — lifted from v9-styles.css's `.v9-rail .w[data-icon=...] .gl`
  // class hooks. Keep the data-icon attribute on the row so the existing CSS
  // colors the glyph automatically.
  function dataIcon(w: Widget): string {
    return w.icon ?? "cal";
  }

  function glyphChar(w: Widget): string {
    if (w.title.length === 0) return "?";
    return w.title[0].toUpperCase();
  }

  async function newQueryWidget() {
    // Prompt for a title; create a Query note with empty DSL and navigate.
    const title = window.prompt("New query name:");
    if (!title) return;
    const trimmed = title.trim();
    if (!trimmed) return;
    const content = [
      "---",
      `title: "${trimmed}"`,
      `type: "Query"`,
      "tags: []",
      "---",
      "query::",
      "section:: saved",
      "",
    ].join("\n");
    try {
      const created = await api.createNote(trimmed, content);
      goto(`/p/${encodeURIComponent(created.id)}`);
    } catch (e) {
      console.error("Failed to create query:", e);
    }
  }

  function sectionTitle(name: WidgetSection): string {
    return name.toUpperCase();
  }

  // Used by the inline iteration to assign a flat index for keynav.
  function flatIndexOf(w: Widget): number {
    return flat.indexOf(w);
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={rootEl}
  class="v9-rail"
  tabindex="0"
  onfocus={() => { setActiveRegion("rail"); if (selectedIndex < 0) selectedIndex = 0; }}
  onclick={() => setActiveRegion("rail")}
  onkeydown={handleKeydown}
  style="outline: none;"
>
  <div class="v9-rail-scroll">
    {#each ["pinned", "browse", "saved"] as const as sectionName}
      {#if sections[sectionName].length > 0}
        <div class="group">{sectionTitle(sectionName)}</div>
        {#each sections[sectionName] as w (w.id)}
          {@const idx = flatIndexOf(w)}
          <a href={`/p/${encodeURIComponent(w.id)}`} class={rowClass(w, idx)} data-icon={dataIcon(w)}>
            <span class="gl">{glyphChar(w)}</span>
            <span>{w.title}</span>
            <span class="badge"></span>
            <span class="caret"></span>
          </a>
        {/each}
      {/if}
    {/each}

    <!-- New widget button -->
    <button class="add" onclick={newQueryWidget} type="button">+ New widget</button>
  </div>

  <!-- Mini calendar (Phase 9.2). Pinned just above the Settings footer. -->
  <MiniCalendar />

  <!-- Settings footer -->
  <div style="border-top: 1px solid var(--v9-line); padding: 6px 6px;">
    <a
      href="/settings"
      class="w {currentPath === '/settings' ? 'active' : ''}"
      data-icon="project"
    >
      <span class="gl">S</span>
      <span>Settings</span>
      <span class="badge">{notes.length}</span>
      <span class="caret"></span>
    </a>
  </div>
</div>
