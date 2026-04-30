<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { getActiveRegion, setActiveRegion } from "$lib/stores/pane-state.svelte";
  import { parseBlocks } from "$lib/block-parser";
  import { widgetFromNote } from "$lib/widget-registry.svelte";
  import { applyTriage, attachToProject, triageActionForKey } from "$lib/triage.svelte";
  import { useQueryClient } from "@tanstack/svelte-query";
  import ProjectPicker from "./ProjectPicker.svelte";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { GraphEdge } from "$lib/types/GraphEdge";
  import type { QueryItem } from "$lib/types/QueryItem";

  type Row = {
    id: string;
    label: string;
    href?: string;
    breadcrumb?: string[];
    primaryTag?: string;
    /** Block + page IDs for triage / drill actions. */
    blockId?: string;
    pageId?: string;
  };

  const middleFocused = $derived(getActiveRegion() === "middle");
  let rootEl = $state<HTMLElement | undefined>();
  let selectedIndex = $state(0);
  const queryClient = useQueryClient();

  // Phase 9.4 — Project picker for `p` triage key.
  let projectPickerRow = $state<Row | null>(null);

  const path = $derived(page.url.pathname);
  const noteId = $derived(path.startsWith("/p/") ? decodeURIComponent(path.slice(3)) : "");

  // ----- The focused note (we need its metadata to detect Query widgets) -----
  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: noteId !== "",
  }));
  const note: Note | undefined = $derived(noteQuery.data as Note | undefined);
  const isQueryWidget = $derived(note?.metadata.note_type === "Query");
  const widget = $derived(note && isQueryWidget ? widgetFromNote(note) : null);

  // ----- Query widget execution -----
  const widgetResultQuery = createQuery(() => ({
    queryKey: ["widget", noteId, widget?.query, widget?.group, widget?.sort] as const,
    queryFn: () =>
      widget && widget.query.trim().length > 0
        ? api.executeQuery(widget.query, widget.group, widget.sort)
        : Promise.resolve({ groups: [] }),
    enabled: !!widget && widget.query.trim().length > 0,
  }));

  // Phase 9.4 — Project list for `p` triage picker. Only fetched when the
  // active widget is `inbox` (the only triage surface today).
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", "all-for-picker"] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: widget?.id === "inbox",
  }));
  const allNotes = $derived((allNotesQuery.data ?? []) as Note[]);

  async function handleProjectSelect(project: Note) {
    const row = projectPickerRow;
    projectPickerRow = null;
    if (!row || !row.blockId || !row.pageId) return;
    try {
      const ok = await attachToProject(row.pageId, row.blockId, project.id);
      if (ok) queryClient.invalidateQueries({ queryKey: ["widget", noteId] });
    } catch (e) {
      console.error("Attach to project failed:", e);
    }
  }

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

  // ----- Backlinks fallback for non-Query notes -----
  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", noteId] as const,
    queryFn: () => api.getBacklinks(noteId),
    enabled: noteId !== "" && !isQueryWidget,
  }));
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: noteId !== "" && !isQueryWidget,
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

  // Convert a backend QueryItem into a row.
  function itemToRow(item: QueryItem): Row {
    const href =
      item.kind === "block"
        ? `/p/${encodeURIComponent(item.page_id)}?block=${encodeURIComponent(item.block_id ?? "")}`
        : `/p/${encodeURIComponent(item.page_id)}`;
    return {
      id: item.block_id ?? item.page_id,
      label: item.text || item.title,
      href,
      breadcrumb: item.parent_breadcrumb,
      primaryTag: item.primary_tag ?? undefined,
      blockId: item.block_id ?? undefined,
      pageId: item.page_id,
    };
  }

  // Inbox special-case (Phase 9.2): pure DSL can't express "page is type=page"
  // negation across multiple non-page note_types. Post-filter here using the
  // `page_note_type` field that the backend populates for block-kind items.
  const TRIAGED_PAGE_TYPES = new Set(["Tag", "Property", "Query", "Template"]);
  function isInboxableRow(item: QueryItem): boolean {
    if (item.kind !== "block") return false;
    // Daily-note pages have IDs of the form YYYY-MM-DD.
    if (/^\d{4}-\d{2}-\d{2}$/.test(item.page_id)) return false;
    if (item.page_note_type && TRIAGED_PAGE_TYPES.has(item.page_note_type)) return false;
    return true;
  }

  type View = {
    title: string;
    subtitle: string;
    /** Either flat rows (legacy nav) OR grouped query result */
    rows: Row[];
    groups?: { key: string; rows: Row[] }[];
    placeholder?: string;
    error?: string;
  };

  const view = $derived.by((): View => {
    if (isQueryWidget && widget) {
      const result = widgetResultQuery.data;
      const isInbox = widget.id === "inbox";
      const filteredGroups = (result?.groups ?? []).map((g) => ({
        ...g,
        items: isInbox ? g.items.filter(isInboxableRow) : g.items,
      }));
      const total = filteredGroups.reduce((acc, g) => acc + g.items.length, 0);
      const groups = filteredGroups.map((g) => ({
        key: g.key || "—",
        rows: g.items.map(itemToRow),
      }));
      // Single-group with empty key → flatten so the rendering branch chooses
      // the flat path (no group headers shown).
      const useGroups = !(groups.length === 1 && groups[0].key === "—");
      return {
        title: widget.title,
        subtitle: widget.query
          ? `${total} ${widget.query.includes("kind:page") ? "pages" : "blocks"}`
          : "(empty query — edit `query::` in the focus pane)",
        rows: useGroups ? [] : groups[0]?.rows ?? [],
        groups: useGroups ? groups : undefined,
        error: widgetResultQuery.error
          ? (widgetResultQuery.error as Error).message
          : undefined,
      };
    }
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

  // Flat row list for keynav, accounting for grouped views.
  const flatRows = $derived<Row[]>(
    view.groups ? view.groups.flatMap((g) => g.rows) : view.rows,
  );

  $effect(() => {
    if (selectedIndex >= flatRows.length) selectedIndex = Math.max(0, flatRows.length - 1);
  });

  $effect(() => {
    if (middleFocused) {
      if (rootEl && document.activeElement !== rootEl) rootEl.focus();
      if (selectedIndex < 0) selectedIndex = 0;
    } else if (rootEl && document.activeElement === rootEl) {
      rootEl.blur();
    }
  });

  async function triageRow(row: Row, key: string): Promise<boolean> {
    const action = triageActionForKey(key);
    if (!action || !row.blockId || !row.pageId) return false;
    try {
      const ok = await applyTriage(row.pageId, row.blockId, action);
      if (ok) {
        // Re-run the query so the row drops out of the inbox list.
        queryClient.invalidateQueries({ queryKey: ["widget", noteId] });
      }
      return ok;
    } catch (e) {
      console.error("Triage failed:", e);
      return false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (!middleFocused) return;
    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(flatRows.length - 1, selectedIndex + 1);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && flatRows[selectedIndex]?.href) {
      e.preventDefault();
      goto(flatRows[selectedIndex].href!);
      setActiveRegion("focus");
    } else if (e.key === "Escape") {
      e.preventDefault();
      setActiveRegion("focus");
    } else if (
      widget?.id === "inbox" &&
      flatRows[selectedIndex] &&
      triageActionForKey(e.key) !== null
    ) {
      e.preventDefault();
      void triageRow(flatRows[selectedIndex], e.key);
    } else if (widget?.id === "inbox" && e.key === "p" && flatRows[selectedIndex]) {
      e.preventDefault();
      projectPickerRow = flatRows[selectedIndex];
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
    {#if view.error}
      <div style="padding: 14px; color: var(--v9-rose); font-family: var(--v9-mono); font-size: 11px;">
        Query error: {view.error}
      </div>
    {:else if view.placeholder}
      <div style="padding: 14px; color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">
        {view.placeholder}
      </div>
    {:else if view.groups}
      {#each view.groups as g}
        {#if g.rows.length > 0}
          <div class="v9-grp">{g.key} <span style="color:var(--v9-ink-faint); margin-left: 6px;">{g.rows.length}</span></div>
          {#each g.rows as row}
            {@const ri = flatRows.indexOf(row)}
            {@const sel = middleFocused && selectedIndex === ri}
            <a
              class="v9-row {sel ? 'selected' : ''}"
              href={row.href ?? "#"}
              onclick={(e) => { if (row.href) { e.preventDefault(); selectedIndex = ri; goto(row.href); setActiveRegion("focus"); } }}
            >
              <span class="marker">{sel ? "▸" : ""}</span>
              <span class="text">
                {#if row.primaryTag}
                  <span class="kind-badge kind-{row.primaryTag.toLowerCase()}">{row.primaryTag}</span>
                {/if}
                {row.label}
              </span>
            </a>
            {#if row.breadcrumb && row.breadcrumb.length > 0}
              <div class="src">↳ {row.breadcrumb.join(" / ")}</div>
            {/if}
          {/each}
        {/if}
      {/each}
    {:else if flatRows.length === 0}
      <div style="padding: 14px; color: var(--v9-ink-faint); font-family: var(--v9-mono); font-size: 11px;">
        — empty —
      </div>
    {:else}
      {#each flatRows as row, ri}
        {@const sel = middleFocused && selectedIndex === ri}
        <a
          class="v9-row {sel ? 'selected' : ''}"
          href={row.href ?? "#"}
          onclick={(e) => { if (row.href) { e.preventDefault(); selectedIndex = ri; goto(row.href); setActiveRegion("focus"); } }}
        >
          <span class="marker">{sel ? "▸" : ""}</span>
          <span class="text">
            {#if row.primaryTag}
              <span class="kind-badge kind-{row.primaryTag.toLowerCase()}">{row.primaryTag}</span>
            {/if}
            {row.label}
          </span>
        </a>
        {#if row.breadcrumb && row.breadcrumb.length > 0}
          <div class="src">↳ {row.breadcrumb.join(" / ")}</div>
        {/if}
      {/each}
    {/if}
  </div>
</div>

{#if projectPickerRow}
  <ProjectPicker
    notes={allNotes}
    onselect={handleProjectSelect}
    onclose={() => (projectPickerRow = null)}
  />
{/if}
