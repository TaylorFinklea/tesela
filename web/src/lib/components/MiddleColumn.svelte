<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { getActiveRegion, setActiveRegion } from "$lib/stores/pane-state.svelte";
  import { parseBlocks } from "$lib/block-parser";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";

  type Row = { id: string; label: string; href?: string };

  const middleFocused = $derived(getActiveRegion() === "middle");
  let rootEl = $state<HTMLElement | undefined>();
  let selectedIndex = $state(0);

  const path = $derived(page.url.pathname);

  // ----- /  (Pages) -----
  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
    enabled: path === "/",
  }));
  const notesList = $derived((notesQuery.data ?? []) as Note[]);

  // ----- /daily — today's daily note's blocks -----
  const dailyQuery = createQuery(() => ({
    queryKey: ["daily-note"] as const,
    queryFn: () => api.getDailyNote(),
    enabled: path === "/daily",
  }));
  const dailyNote: Note | undefined = $derived(dailyQuery.data as Note | undefined);

  function bodyOf(note: Note | undefined): string {
    if (!note) return "";
    const c = note.content;
    if (!c.startsWith("---")) return c;
    const end = c.indexOf("---", 3);
    if (end === -1) return c;
    const after = c.slice(end + 3);
    return after.startsWith("\n") ? after.slice(1) : after;
  }
  const dailyBlocks = $derived(dailyNote ? parseBlocks(dailyNote.id, bodyOf(dailyNote)) : []);
  const dailyTopBlocks = $derived(dailyBlocks.filter((b) => b.indent_level === 0));

  // ----- /p/[id] — backlinks of focused note -----
  const noteId = $derived(path.startsWith("/p/") ? decodeURIComponent(path.slice(3)) : "");
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

  // Title + subtitle + rows for the active context.
  const view = $derived.by((): { title: string; subtitle: string; rows: Row[]; placeholder?: string } => {
    if (path === "/") {
      return {
        title: "Pages",
        subtitle: `${notesList.length} notes`,
        rows: notesList.map((n) => ({
          id: n.id,
          label: n.title,
          href: `/p/${encodeURIComponent(n.id)}`,
        })),
      };
    }
    if (path === "/daily") {
      return {
        title: "Today",
        subtitle: dailyNote?.title ?? "loading…",
        rows: dailyTopBlocks.map((b) => ({
          id: b.id,
          label: b.text || "(empty)",
          href: dailyNote ? `/p/${encodeURIComponent(dailyNote.id)}?block=${encodeURIComponent(b.id)}` : undefined,
        })),
      };
    }
    if (path.startsWith("/p/")) {
      return {
        title: "Backlinks",
        subtitle: `${allBacklinkSources.length} pages link here`,
        rows: allBacklinkSources.map((src) => ({
          id: src,
          label: src,
          href: `/p/${encodeURIComponent(src.toLowerCase())}`,
        })),
      };
    }
    return {
      title: path.replace(/^\//, "") || "—",
      subtitle: "",
      rows: [],
      placeholder: "No list view in 9.0",
    };
  });

  $effect(() => {
    if (selectedIndex >= view.rows.length) selectedIndex = Math.max(0, view.rows.length - 1);
  });

  $effect(() => {
    if (middleFocused) {
      if (rootEl && document.activeElement !== rootEl) rootEl.focus();
      if (selectedIndex < 0) selectedIndex = 0;
    } else if (rootEl && document.activeElement === rootEl) {
      rootEl.blur();
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!middleFocused) return;
    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(view.rows.length - 1, selectedIndex + 1);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && view.rows[selectedIndex]?.href) {
      e.preventDefault();
      goto(view.rows[selectedIndex].href!);
      setActiveRegion("focus");
    } else if (e.key === "Escape") {
      e.preventDefault();
      setActiveRegion("focus");
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={rootEl}
  class="v9-middle"
  tabindex="0"
  onfocus={() => { setActiveRegion("middle"); if (selectedIndex < 0) selectedIndex = 0; }}
  onclick={() => setActiveRegion("middle")}
  onkeydown={handleKeydown}
  style="outline: none;"
>
  <div class="v9-pane-head">
    <span class="t">{view.title}</span>
    <span class="s">{view.subtitle}</span>
  </div>
  <div class="v9-pane-body">
    {#if view.placeholder}
      <div style="padding: 14px; color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">
        {view.placeholder}
      </div>
    {:else if view.rows.length === 0}
      <div style="padding: 14px; color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">
        — empty —
      </div>
    {:else}
      {#each view.rows as row, ri}
        {@const sel = middleFocused && selectedIndex === ri}
        <a
          class="v9-row {sel ? 'selected' : ''}"
          href={row.href ?? "#"}
          onclick={(e) => { if (row.href) { e.preventDefault(); selectedIndex = ri; goto(row.href); setActiveRegion("focus"); } }}
        >
          <span class="marker">{sel ? "▸" : ""}</span>
          <span class="text">{row.label}</span>
        </a>
      {/each}
    {/if}
  </div>
</div>
