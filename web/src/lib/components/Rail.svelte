<script lang="ts">
  import { browser } from "$app/environment";
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
  // Phase 9.4 — apply user-rearranged order from localStorage. Widgets not
  // present in the saved order keep alphabetical (pre-order from parseWidgets).
  const ORDER_KEY = "tesela:railOrder";
  let savedOrder = $state<Record<WidgetSection, string[]>>(loadOrder());

  function loadOrder(): Record<WidgetSection, string[]> {
    if (!browser) return { pinned: [], browse: [], saved: [] };
    try {
      const raw = localStorage.getItem(ORDER_KEY);
      if (!raw) return { pinned: [], browse: [], saved: [] };
      const parsed = JSON.parse(raw);
      return {
        pinned: Array.isArray(parsed.pinned) ? parsed.pinned : [],
        browse: Array.isArray(parsed.browse) ? parsed.browse : [],
        saved: Array.isArray(parsed.saved) ? parsed.saved : [],
      };
    } catch {
      return { pinned: [], browse: [], saved: [] };
    }
  }
  function persistOrder(o: Record<WidgetSection, string[]>) {
    if (!browser) return;
    try {
      localStorage.setItem(ORDER_KEY, JSON.stringify(o));
    } catch {
      // ignore
    }
  }
  function applyOrder(items: Widget[], order: string[]): Widget[] {
    if (order.length === 0) return items;
    const byId = new Map(items.map((w) => [w.id, w]));
    const out: Widget[] = [];
    for (const id of order) {
      const w = byId.get(id);
      if (w) {
        out.push(w);
        byId.delete(id);
      }
    }
    // Append any new widgets that aren't in the saved order (newly-created since last reorder)
    for (const w of byId.values()) out.push(w);
    return out;
  }
  const orderedSections = $derived<Record<WidgetSection, Widget[]>>({
    pinned: applyOrder(sections.pinned, savedOrder.pinned),
    browse: applyOrder(sections.browse, savedOrder.browse),
    saved: applyOrder(sections.saved, savedOrder.saved),
  });
  const flat = $derived<Widget[]>([
    ...orderedSections.pinned,
    ...orderedSections.browse,
    ...orderedSections.saved,
  ]);
  const currentPath = $derived(page.url.pathname);

  // Drag-drop state
  let draggingId = $state<string | null>(null);

  function onDragStart(e: DragEvent, w: Widget) {
    if (!e.dataTransfer) return;
    draggingId = w.id;
    e.dataTransfer.effectAllowed = "move";
    e.dataTransfer.setData("text/plain", w.id);
  }
  function onDragOver(e: DragEvent) {
    if (draggingId === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
  }
  function onDrop(e: DragEvent, targetSection: WidgetSection, targetWidget: Widget) {
    e.preventDefault();
    if (!draggingId || draggingId === targetWidget.id) {
      draggingId = null;
      return;
    }
    // Determine source section by inspecting the dragged widget.
    const dragged = widgets.find((w) => w.id === draggingId);
    if (!dragged) {
      draggingId = null;
      return;
    }
    // Build new ordered list for the target section by inserting dragged widget
    // at targetWidget's position. If sections differ, the dragged widget moves
    // section.
    const currentTargetOrder = orderedSections[targetSection].map((w) => w.id);
    const filtered = currentTargetOrder.filter((id) => id !== dragged.id);
    const targetIdx = filtered.indexOf(targetWidget.id);
    const insertAt = targetIdx >= 0 ? targetIdx : filtered.length;
    filtered.splice(insertAt, 0, dragged.id);
    const next: Record<WidgetSection, string[]> = {
      pinned: [...savedOrder.pinned],
      browse: [...savedOrder.browse],
      saved: [...savedOrder.saved],
    };
    // Remove dragged from any other section's order
    for (const s of ["pinned", "browse", "saved"] as const) {
      if (s !== targetSection) {
        next[s] = next[s].filter((id) => id !== dragged.id);
      }
    }
    next[targetSection] = filtered;
    savedOrder = next;
    persistOrder(next);
    draggingId = null;
  }
  function onDragEnd() {
    draggingId = null;
  }

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
      {#if orderedSections[sectionName].length > 0}
        <div class="group">{sectionTitle(sectionName)}</div>
        {#each orderedSections[sectionName] as w (w.id)}
          {@const idx = flatIndexOf(w)}
          <a
            href={`/p/${encodeURIComponent(w.id)}`}
            class="{rowClass(w, idx)} {draggingId === w.id ? 'dragging' : ''}"
            data-icon={dataIcon(w)}
            draggable="true"
            ondragstart={(e) => onDragStart(e, w)}
            ondragover={onDragOver}
            ondrop={(e) => onDrop(e, sectionName, w)}
            ondragend={onDragEnd}
          >
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
