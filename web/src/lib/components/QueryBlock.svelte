<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { parseBlocks, segmentText, type TextSegment } from "$lib/block-parser";
  import { parseQuery, blockMatches } from "$lib/query-language";
  import {
    IconTable,
    IconLayoutGrid,
    IconList,
    IconHierarchy,
    IconLayoutKanban,
  } from "@tabler/icons-svelte";
  import type { Note } from "$lib/types/Note";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";

  type ViewMode = "table" | "cards" | "list" | "outline" | "kanban";
  const ALL_VIEWS: ViewMode[] = ["table", "cards", "list", "outline", "kanban"];

  /** A saved view ("tab") — same data source, different lens. */
  type Tab = { name: string; view: ViewMode; filter?: string };

  let { block, onUpdate }: {
    block: ParsedBlock;
    onUpdate?: (newRawText: string) => void;
  } = $props();

  function navigateToBlock(noteId: string, blockId: string) {
    goto(`/p/${noteId}?block=${encodeURIComponent(blockId)}`);
  }

  const queryText = $derived((block.properties.query ?? "").trim());

  /**
   * Parsed tab list. Reads `views::` JSON if present, otherwise synthesizes a
   * single "All" tab from the legacy `view::` property (or default "table").
   */
  const tabs: Tab[] = $derived.by(() => {
    const viewsRaw = block.properties.views;
    if (typeof viewsRaw === "string" && viewsRaw.trim().length > 0) {
      try {
        const parsed = JSON.parse(viewsRaw);
        if (Array.isArray(parsed)) {
          return parsed.map((t, i): Tab => ({
            name: typeof t?.name === "string" ? t.name : `View ${i + 1}`,
            view: (ALL_VIEWS as string[]).includes(t?.view) ? t.view : "table",
            filter: typeof t?.filter === "string" ? t.filter : undefined,
          }));
        }
      } catch { /* fall through to legacy */ }
    }
    const legacyView = (block.properties.view ?? "table").trim().toLowerCase();
    return [{
      name: "All",
      view: ((ALL_VIEWS as string[]).includes(legacyView) ? legacyView : "table") as ViewMode,
    }];
  });

  const activeIdx = $derived.by(() => {
    const raw = block.properties.active_view ?? "0";
    const n = parseInt(raw, 10);
    if (Number.isNaN(n) || n < 0 || n >= tabs.length) return 0;
    return n;
  });
  const activeTab = $derived(tabs[activeIdx] ?? { name: "All", view: "table" as ViewMode });
  const viewMode: ViewMode = $derived(activeTab.view);

  /**
   * Write a multi-property update to the block. Each entry replaces the line
   * with that key (case-insensitive); pass `null` to remove the line entirely.
   */
  function writeBlockProps(updates: Record<string, string | null>) {
    if (!onUpdate) return;
    const lines = block.raw_text.split("\n");
    for (const [key, value] of Object.entries(updates)) {
      const re = new RegExp(`^${key}::`, "i");
      const idx = lines.findIndex((l) => re.test(l));
      if (value === null) {
        if (idx >= 0) lines.splice(idx, 1);
      } else if (idx >= 0) {
        lines[idx] = `${key}:: ${value}`;
      } else {
        lines.push(`${key}:: ${value}`);
      }
    }
    onUpdate(lines.join("\n"));
  }

  function persistTabs(next: Tab[], nextActive: number) {
    writeBlockProps({
      views: JSON.stringify(next),
      active_view: String(nextActive),
      view: null, // legacy field becomes redundant once views:: is set
    });
  }

  function setView(next: ViewMode) {
    const updated = tabs.map((t, i) => i === activeIdx ? { ...t, view: next } : t);
    persistTabs(updated, activeIdx);
  }

  function setActiveTab(idx: number) {
    if (idx < 0 || idx >= tabs.length) return;
    writeBlockProps({ active_view: String(idx) });
  }

  function addTab() {
    const next: Tab[] = [...tabs, { name: `View ${tabs.length + 1}`, view: "table" }];
    persistTabs(next, next.length - 1);
  }

  function deleteTab(idx: number) {
    if (tabs.length <= 1) return; // keep at least one tab
    const next = tabs.filter((_, i) => i !== idx);
    const newActive = idx <= activeIdx && activeIdx > 0 ? activeIdx - 1 : Math.min(activeIdx, next.length - 1);
    persistTabs(next, newActive);
  }

  function renameTab(idx: number, name: string) {
    const trimmed = name.trim();
    if (!trimmed) return;
    const updated = tabs.map((t, i) => i === idx ? { ...t, name: trimmed } : t);
    persistTabs(updated, activeIdx);
  }

  let editingTabIdx = $state<number | null>(null);
  let editingTabName = $state("");

  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: queryText.length > 0,
  }));

  // Combine the base query with the active tab's optional filter (intersection).
  const combinedQueryText = $derived.by(() => {
    const filter = activeTab.filter?.trim() ?? "";
    return [queryText, filter].filter((s) => s.length > 0).join(" ");
  });
  const parsedQuery = $derived(parseQuery(combinedQueryText));

  type Match = { block: ParsedBlock; noteTitle: string; noteId: string };
  const matches: Match[] = $derived.by(() => {
    if (!queryText) return [];
    const notes = (allNotesQuery.data ?? []) as Note[];
    const out: Match[] = [];
    for (const n of notes) {
      const noteBlocks = parseBlocks(n.id, n.body);
      for (const b of noteBlocks) {
        if (blockMatches(b, parsedQuery)) {
          out.push({ block: b, noteTitle: n.title, noteId: n.id });
        }
      }
    }
    return out;
  });

  // Query block system keys — never expose as table columns.
  const SYSTEM_KEYS = new Set(["query", "view", "views", "active_view", "collection"]);

  // Union of all property keys from matched blocks (excluding system keys).
  const propColumns = $derived.by(() => {
    const keys = new Set<string>();
    for (const m of matches) {
      for (const k of Object.keys(m.block.properties)) {
        if (!SYSTEM_KEYS.has(k.toLowerCase())) keys.add(k);
      }
    }
    return [...keys].sort();
  });

  const groupBy = $derived(
    viewMode === "kanban"
      ? propColumns.includes("status") ? "status" : (propColumns[0] ?? null)
      : null,
  );
  const kanbanGroups = $derived.by(() => {
    if (!groupBy) return [];
    const buckets = new Map<string, Match[]>();
    for (const m of matches) {
      const key = m.block.properties[groupBy] ?? "—";
      const arr = buckets.get(key) ?? [];
      arr.push(m);
      buckets.set(key, arr);
    }
    return [...buckets.entries()].sort(([a], [b]) => a.localeCompare(b));
  });

  function statusIcon(s: string | undefined): string {
    if (s === "done") return "●";
    if (s === "doing" || s === "in-review") return "◐";
    return "○";
  }
  function statusColorClass(s: string | undefined): string {
    if (s === "done") return "text-emerald-400/80";
    if (s === "doing" || s === "in-review") return "text-blue-400/80";
    if (s === "todo") return "text-amber-400/80";
    return "text-muted-foreground/60";
  }

  const VIEW_META: { id: ViewMode; label: string; Icon: typeof IconTable }[] = [
    { id: "table", label: "Table", Icon: IconTable },
    { id: "cards", label: "Cards", Icon: IconLayoutGrid },
    { id: "list", label: "List", Icon: IconList },
    { id: "outline", label: "Outline", Icon: IconHierarchy },
    { id: "kanban", label: "Kanban", Icon: IconLayoutKanban },
  ];
</script>

{#snippet segmentRender(text: string)}
  {#if text}
    {#each segmentText(text) as seg}
      {#if seg.type === "link"}
        <a href={seg.href} class="text-primary/80 underline decoration-primary/30 underline-offset-2 hover:decoration-primary" onclick={(e) => e.stopPropagation()}>{seg.value}</a>
      {:else}
        <span>{seg.value}</span>
      {/if}
    {/each}
  {:else}
    <span class="text-muted-foreground/40">(empty)</span>
  {/if}
{/snippet}

<div class="ml-6 mt-1 mb-3 p-3 rounded-md bg-muted/20 border border-border/40">
  <!-- Tab strip + view switcher -->
  <div class="flex items-center justify-between mb-2 gap-2">
    <!-- Tabs -->
    <div class="flex items-center gap-0.5 flex-wrap min-w-0">
      {#each tabs as tab, i (i)}
        {@const active = i === activeIdx}
        <div class="group/tab inline-flex items-center gap-0.5">
          {#if editingTabIdx === i}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              autofocus
              class="text-[11px] bg-surface border border-primary/40 rounded px-1.5 py-0.5 outline-none w-24"
              bind:value={editingTabName}
              onblur={() => { renameTab(i, editingTabName); editingTabIdx = null; }}
              onkeydown={(e) => {
                if (e.key === "Enter") { renameTab(i, editingTabName); editingTabIdx = null; }
                if (e.key === "Escape") { editingTabIdx = null; }
              }}
            />
          {:else}
            <button
              class="text-[11px] px-2 py-0.5 rounded transition-all {active ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-foreground/70 hover:bg-muted/30'}"
              onclick={() => setActiveTab(i)}
              ondblclick={() => { editingTabIdx = i; editingTabName = tab.name; }}
              title="Click to switch · double-click to rename"
            >{tab.name}</button>
            {#if active && tabs.length > 1}
              <!-- svelte-ignore a11y_consider_explicit_label -->
              <button
                class="opacity-0 group-hover/tab:opacity-100 leading-none text-muted-foreground/40 hover:text-destructive text-[10px] transition-opacity"
                onclick={() => deleteTab(i)}
                title="Delete tab"
              >×</button>
            {/if}
          {/if}
        </div>
      {/each}
      <!-- svelte-ignore a11y_consider_explicit_label -->
      <button
        class="text-[11px] px-1.5 py-0.5 rounded text-muted-foreground/40 hover:text-primary hover:bg-muted/30 transition-colors"
        onclick={addTab}
        title="Add new tab"
      >+</button>
    </div>

    <!-- View switcher (operates on active tab) -->
    <div class="flex items-center gap-0.5 bg-muted/40 rounded-md p-0.5 shrink-0">
      {#each VIEW_META as v}
        {@const active = v.id === viewMode}
        <!-- svelte-ignore a11y_consider_explicit_label -->
        <button
          class="p-1 rounded transition-all {active ? 'bg-surface text-primary shadow-sm' : 'text-muted-foreground/60 hover:text-foreground/70'}"
          onclick={() => setView(v.id)}
          title={v.label}
        >
          <v.Icon size={12} stroke={1.5} />
        </button>
      {/each}
    </div>
  </div>

  <!-- Match count + query info -->
  <div class="text-[10px] text-muted-foreground/50 mb-2">
    {#if queryText}
      {matches.length} {matches.length === 1 ? "match" : "matches"}
      · <code class="text-foreground/70">{queryText}</code>
      {#if activeTab.filter}
        <span class="text-muted-foreground/40">+</span> <code class="text-foreground/70">{activeTab.filter}</code>
      {/if}
    {:else}empty query{/if}
  </div>

  {#if !queryText}
    <div class="text-[11px] text-muted-foreground/60 italic">Empty query — set <code>query::</code> on this block.</div>
  {:else if allNotesQuery.isLoading}
    <div class="text-[11px] text-muted-foreground">Loading…</div>
  {:else if matches.length === 0}
    <div class="text-[11px] text-muted-foreground/60 italic">No matches</div>

  <!-- TABLE -->
  {:else if viewMode === "table"}
    <table class="w-full text-[12px]">
      <thead>
        <tr class="text-[10px] text-muted-foreground/70 uppercase tracking-wider">
          <th class="text-left font-medium pb-1.5 px-2">Block</th>
          <th class="text-left font-medium pb-1.5 px-2">Page</th>
          <th class="text-left font-medium pb-1.5 px-2">Tags</th>
          {#each propColumns as col}
            <th class="text-left font-medium pb-1.5 px-2">{col}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each matches as m}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <tr class="border-t border-border/30 hover:bg-muted/30 transition-colors cursor-pointer" onclick={() => navigateToBlock(m.noteId, m.block.id)}>
            <td class="px-2 py-1.5 text-foreground/85 font-medium">{@render segmentRender(m.block.text)}</td>
            <td class="px-2 py-1.5 text-muted-foreground/70">
              <a href="/p/{m.noteId}" class="hover:text-primary transition-colors" onclick={(e) => e.stopPropagation()}>{m.noteTitle}</a>
            </td>
            <td class="px-2 py-1.5">
              <div class="flex flex-wrap gap-0.5">
                {#each m.block.tags as t}
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70">{t}</span>
                {/each}
              </div>
            </td>
            {#each propColumns as col}
              <td class="px-2 py-1.5 text-foreground/70">
                {@render segmentRender(m.block.properties[col] ?? "")}
              </td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>

  <!-- CARDS -->
  {:else if viewMode === "cards"}
    <div class="space-y-1">
      {#each matches as m}
        {@const status = m.block.properties.status}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="p-3 rounded bg-surface border border-border/40 hover:border-primary/40 transition-colors cursor-pointer"
          onclick={() => navigateToBlock(m.noteId, m.block.id)}
        >
          <div class="flex items-center gap-2">
            <span class="text-[14px] font-mono leading-none {statusColorClass(status)}">{statusIcon(status)}</span>
            <div class="text-[13px] text-foreground/90 font-medium flex-1 min-w-0">
              {@render segmentRender(m.block.text)}
            </div>
            {#each m.block.tags as t}
              <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70">{t}</span>
            {/each}
          </div>
          <div class="flex items-center flex-wrap gap-x-3 gap-y-1 mt-1.5 ml-5 text-[11px] text-muted-foreground/70">
            <a href="/p/{m.noteId}" class="hover:text-primary transition-colors" onclick={(e) => e.stopPropagation()}>{m.noteTitle}</a>
            {#each propColumns as col}
              {#if m.block.properties[col]}
                <span class="flex items-center gap-1">
                  <span class="text-muted-foreground/50">{col}</span>
                  <span class="text-foreground/70">{@render segmentRender(m.block.properties[col])}</span>
                </span>
              {/if}
            {/each}
          </div>
        </div>
      {/each}
    </div>

  <!-- LIST -->
  {:else if viewMode === "list"}
    <div>
      {#each matches as m}
        {@const status = m.block.properties.status}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="flex items-center gap-3 py-1.5 px-2 hover:bg-muted/40 rounded transition-colors cursor-pointer"
          onclick={() => navigateToBlock(m.noteId, m.block.id)}
        >
          <span class="text-[14px] font-mono leading-none w-3 shrink-0 {statusColorClass(status)}">{statusIcon(status)}</span>
          <div class="text-[12px] text-foreground/85 flex-1 min-w-0 truncate">
            {@render segmentRender(m.block.text)}
          </div>
          <a href="/p/{m.noteId}" class="text-[10px] text-muted-foreground/60 shrink-0 hover:text-primary transition-colors" onclick={(e) => e.stopPropagation()}>{m.noteTitle}</a>
          {#each propColumns as col}
            {#if m.block.properties[col]}
              <span class="text-[10px] text-muted-foreground/40 shrink-0">·</span>
              <span class="text-[10px] text-muted-foreground/70 shrink-0">
                {@render segmentRender(m.block.properties[col])}
              </span>
            {/if}
          {/each}
        </div>
      {/each}
    </div>

  <!-- OUTLINE -->
  {:else if viewMode === "outline"}
    <div>
      {#each matches as m}
        {@const status = m.block.properties.status}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="group flex items-start py-1 hover:bg-accent/30 rounded transition-colors cursor-pointer" onclick={() => navigateToBlock(m.noteId, m.block.id)}>
          <span class="shrink-0 pt-[10px] pl-2 pr-1 {statusColorClass(status)}">
            <span class="block text-[12px] leading-none font-mono w-[14px] text-center">{statusIcon(status)}</span>
          </span>
          <div class="flex-1 min-w-0 py-1">
            <div class="text-[14px] text-foreground/90 font-mono">
              {@render segmentRender(m.block.text)}
            </div>
            <div class="text-[11px] text-muted-foreground/60 mt-0.5 flex items-center flex-wrap gap-x-2">
              <span class="text-muted-foreground/40">↳</span>
              <a href="/p/{m.noteId}" class="hover:text-primary transition-colors" onclick={(e) => e.stopPropagation()}>{m.noteTitle}</a>
              {#each propColumns as col}
                {#if m.block.properties[col]}
                  <span class="text-muted-foreground/30">·</span>
                  <span><span class="text-muted-foreground/40">{col}</span> {@render segmentRender(m.block.properties[col])}</span>
                {/if}
              {/each}
            </div>
          </div>
          <div class="shrink-0 flex items-center gap-1 self-center pr-2 py-1">
            {#each m.block.tags as t}
              <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70 font-medium">{t}</span>
            {/each}
          </div>
        </div>
      {/each}
    </div>

  <!-- KANBAN -->
  {:else if viewMode === "kanban"}
    {#if !groupBy}
      <div class="text-[11px] text-muted-foreground/60 italic">No groupable property — kanban needs a `status` (or any property) on results.</div>
    {:else}
      <div class="flex gap-3 overflow-x-auto">
        {#each kanbanGroups as [colName, items]}
          <div class="shrink-0 w-[220px]">
            <div class="text-[10px] font-semibold text-muted-foreground/70 uppercase tracking-widest mb-2 px-1">
              {colName} <span class="text-muted-foreground/40">({items.length})</span>
            </div>
            <div class="space-y-1">
              {#each items as m}
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div
                  class="block p-2 rounded bg-surface border border-border/40 hover:border-primary/40 transition-colors cursor-pointer"
                  onclick={() => navigateToBlock(m.noteId, m.block.id)}
                >
                  <div class="text-[12px] text-foreground/80 line-clamp-2">
                    {@render segmentRender(m.block.text)}
                  </div>
                  <div class="flex items-center gap-1 mt-1 text-[9px] text-muted-foreground/60">
                    <span>{m.noteTitle}</span>
                    {#if m.block.tags.length > 0}
                      <span>·</span>
                      {#each m.block.tags.slice(0, 2) as t}<span class="text-primary/60">#{t}</span>{/each}
                    {/if}
                  </div>
                </div>
              {/each}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  {/if}
</div>
