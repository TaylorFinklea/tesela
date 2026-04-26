<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { parseBlocks, segmentText } from "$lib/block-parser";
  import {
    IconTable,
    IconLayoutGrid,
    IconList,
    IconHierarchy,
    IconLayoutKanban,
    IconArrowUp,
    IconArrowDown,
  } from "@tabler/icons-svelte";
  import type { Note } from "$lib/types/Note";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";

  type ViewMode = "table" | "cards" | "list" | "outline" | "kanban";
  const ALL_VIEWS: ViewMode[] = ["table", "cards", "list", "outline", "kanban"];

  let { block, onUpdate }: {
    block: ParsedBlock;
    onUpdate?: (newRawText: string) => void;
  } = $props();

  function navigateToBlock(noteId: string, blockId: string) {
    goto(`/p/${noteId}?block=${encodeURIComponent(blockId)}`);
  }

  /** Parsed list of block IDs from `collection::` JSON array. */
  const blockIds: string[] = $derived.by(() => {
    const raw = block.properties.collection;
    if (typeof raw !== "string" || raw.trim().length === 0) return [];
    try {
      const parsed = JSON.parse(raw);
      return Array.isArray(parsed) ? parsed.filter((s): s is string => typeof s === "string") : [];
    } catch {
      return [];
    }
  });

  const viewMode: ViewMode = $derived.by(() => {
    const v = (block.properties.view ?? "cards").trim().toLowerCase();
    return ((ALL_VIEWS as string[]).includes(v) ? v : "cards") as ViewMode;
  });

  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
  }));

  type Match = { block: ParsedBlock; noteTitle: string; noteId: string };

  /** Index every block by its ID for O(1) collection-member lookup. */
  const blockIndex = $derived.by((): Map<string, Match> => {
    const out = new Map<string, Match>();
    const notes = (allNotesQuery.data ?? []) as Note[];
    for (const n of notes) {
      for (const b of parseBlocks(n.id, n.body)) {
        out.set(b.id, { block: b, noteTitle: n.title, noteId: n.id });
      }
    }
    return out;
  });

  /** Resolved members in the order specified by collection::, dropping any
   *  IDs that no longer exist (block moved/deleted). */
  const members: Match[] = $derived.by(() =>
    blockIds.map((id) => blockIndex.get(id)).filter((m): m is Match => m !== undefined),
  );

  // Add-block search state
  let addingBlock = $state(false);
  let addQuery = $state("");
  let addResults: Match[] = $derived.by(() => {
    const q = addQuery.trim().toLowerCase();
    if (!q) return [];
    const inCollection = new Set(blockIds);
    const out: Match[] = [];
    for (const [id, m] of blockIndex) {
      if (inCollection.has(id)) continue;
      if (m.block.text.toLowerCase().includes(q) || m.noteTitle.toLowerCase().includes(q)) {
        out.push(m);
        if (out.length >= 8) break;
      }
    }
    return out;
  });

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

  function setView(next: ViewMode) {
    writeBlockProps({ view: next });
  }

  function addBlock(id: string) {
    if (blockIds.includes(id)) return;
    writeBlockProps({ collection: JSON.stringify([...blockIds, id]) });
    addQuery = "";
    addingBlock = false;
  }

  function removeBlock(id: string) {
    writeBlockProps({ collection: JSON.stringify(blockIds.filter((x) => x !== id)) });
  }

  function moveBlock(id: string, direction: -1 | 1) {
    const idx = blockIds.indexOf(id);
    const next = idx + direction;
    if (idx < 0 || next < 0 || next >= blockIds.length) return;
    const updated = [...blockIds];
    [updated[idx], updated[next]] = [updated[next], updated[idx]];
    writeBlockProps({ collection: JSON.stringify(updated) });
  }

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

  const propColumns = $derived.by(() => {
    const SYSTEM = new Set(["query", "view", "views", "active_view", "collection"]);
    const keys = new Set<string>();
    for (const m of members) {
      for (const k of Object.keys(m.block.properties)) {
        if (!SYSTEM.has(k.toLowerCase())) keys.add(k);
      }
    }
    return [...keys].sort();
  });

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

{#snippet rowControls(id: string, idx: number)}
  <div class="shrink-0 flex items-center gap-0.5 opacity-0 group-hover/row:opacity-100 transition-opacity">
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="p-0.5 rounded text-muted-foreground/50 hover:text-foreground/80 hover:bg-muted/30 disabled:opacity-30"
      disabled={idx === 0}
      onclick={(e) => { e.stopPropagation(); moveBlock(id, -1); }}
      title="Move up"
    ><IconArrowUp size={11} stroke={1.5} /></button>
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="p-0.5 rounded text-muted-foreground/50 hover:text-foreground/80 hover:bg-muted/30 disabled:opacity-30"
      disabled={idx === blockIds.length - 1}
      onclick={(e) => { e.stopPropagation(); moveBlock(id, 1); }}
      title="Move down"
    ><IconArrowDown size={11} stroke={1.5} /></button>
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="p-0.5 rounded text-muted-foreground/40 hover:text-destructive hover:bg-destructive/10"
      onclick={(e) => { e.stopPropagation(); removeBlock(id); }}
      title="Remove from collection"
    >×</button>
  </div>
{/snippet}

<div class="ml-6 mt-1 mb-3 p-3 rounded-md bg-muted/20 border border-border/40">
  <!-- Header: count + view switcher -->
  <div class="flex items-center justify-between mb-2 gap-2">
    <div class="text-[10px] text-muted-foreground/50">
      {members.length} {members.length === 1 ? "block" : "blocks"} in collection
    </div>
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

  {#if members.length === 0 && !addingBlock}
    <div class="text-[11px] text-muted-foreground/60 italic mb-2">No blocks added yet</div>

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
          <th class="w-16"></th>
        </tr>
      </thead>
      <tbody>
        {#each members as m, idx (m.block.id)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <tr class="group/row border-t border-border/30 hover:bg-muted/30 transition-colors cursor-pointer" onclick={() => navigateToBlock(m.noteId, m.block.id)}>
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
              <td class="px-2 py-1.5 text-foreground/70">{@render segmentRender(m.block.properties[col] ?? "")}</td>
            {/each}
            <td class="px-2 py-1.5">{@render rowControls(m.block.id, idx)}</td>
          </tr>
        {/each}
      </tbody>
    </table>

  {:else if viewMode === "cards"}
    <div class="space-y-1">
      {#each members as m, idx (m.block.id)}
        {@const status = m.block.properties.status}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="group/row p-3 rounded bg-surface border border-border/40 hover:border-primary/40 transition-colors cursor-pointer flex items-start gap-2"
          onclick={() => navigateToBlock(m.noteId, m.block.id)}
        >
          <span class="text-[14px] font-mono leading-none mt-1 {statusColorClass(status)}">{statusIcon(status)}</span>
          <div class="flex-1 min-w-0">
            <div class="text-[13px] text-foreground/90 font-medium">
              {@render segmentRender(m.block.text)}
              {#each m.block.tags as t}
                <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70 ml-1">{t}</span>
              {/each}
            </div>
            <div class="flex items-center flex-wrap gap-x-3 gap-y-1 mt-1.5 text-[11px] text-muted-foreground/70">
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
          {@render rowControls(m.block.id, idx)}
        </div>
      {/each}
    </div>

  {:else if viewMode === "list"}
    <div>
      {#each members as m, idx (m.block.id)}
        {@const status = m.block.properties.status}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="group/row flex items-center gap-3 py-1.5 px-2 hover:bg-muted/40 rounded transition-colors cursor-pointer"
          onclick={() => navigateToBlock(m.noteId, m.block.id)}
        >
          <span class="text-[14px] font-mono leading-none w-3 shrink-0 {statusColorClass(status)}">{statusIcon(status)}</span>
          <div class="text-[12px] text-foreground/85 flex-1 min-w-0 truncate">{@render segmentRender(m.block.text)}</div>
          <a href="/p/{m.noteId}" class="text-[10px] text-muted-foreground/60 shrink-0 hover:text-primary transition-colors" onclick={(e) => e.stopPropagation()}>{m.noteTitle}</a>
          {@render rowControls(m.block.id, idx)}
        </div>
      {/each}
    </div>

  {:else if viewMode === "outline"}
    <div>
      {#each members as m, idx (m.block.id)}
        {@const status = m.block.properties.status}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="group/row flex items-start py-1 hover:bg-accent/30 rounded transition-colors cursor-pointer" onclick={() => navigateToBlock(m.noteId, m.block.id)}>
          <span class="shrink-0 pt-[10px] pl-2 pr-1 {statusColorClass(status)}">
            <span class="block text-[12px] leading-none font-mono w-[14px] text-center">{statusIcon(status)}</span>
          </span>
          <div class="flex-1 min-w-0 py-1">
            <div class="text-[14px] text-foreground/90 font-mono">{@render segmentRender(m.block.text)}</div>
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
          {@render rowControls(m.block.id, idx)}
        </div>
      {/each}
    </div>

  {:else if viewMode === "kanban"}
    <!-- Kanban groups by status; ordering within each column follows collection order -->
    {@const buckets = (() => {
      const m = new Map<string, Match[]>();
      for (const item of members) {
        const k = item.block.properties.status ?? "—";
        const arr = m.get(k) ?? [];
        arr.push(item);
        m.set(k, arr);
      }
      return [...m.entries()].sort(([a], [b]) => a.localeCompare(b));
    })()}
    <div class="flex gap-3 overflow-x-auto">
      {#each buckets as [colName, items]}
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
                <div class="text-[12px] text-foreground/80 line-clamp-2">{@render segmentRender(m.block.text)}</div>
                <div class="flex items-center gap-1 mt-1 text-[9px] text-muted-foreground/60">
                  <span>{m.noteTitle}</span>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Add-block control -->
  {#if addingBlock}
    <div class="mt-2 relative">
      <!-- svelte-ignore a11y_autofocus -->
      <input
        autofocus
        type="text"
        placeholder="Search blocks…"
        bind:value={addQuery}
        onkeydown={(e) => { if (e.key === "Escape") { addingBlock = false; addQuery = ""; } }}
        class="w-full text-[12px] bg-surface border border-primary/30 rounded px-2 py-1 outline-none focus:border-primary"
      />
      {#if addResults.length > 0}
        <div class="absolute left-0 right-0 top-full mt-1 max-h-60 overflow-y-auto bg-popover border border-border rounded-md shadow-lg z-10">
          {#each addResults as r}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="px-3 py-1.5 hover:bg-muted/40 cursor-pointer"
              onclick={() => addBlock(r.block.id)}
            >
              <div class="text-[12px] text-foreground/80 truncate">{r.block.text || "(empty)"}</div>
              <div class="text-[10px] text-muted-foreground/60">{r.noteTitle}</div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {:else}
    <button
      class="mt-2 text-[11px] px-2 py-1 rounded text-muted-foreground/50 hover:text-primary hover:bg-muted/30 transition-colors"
      onclick={() => { addingBlock = true; }}
    >+ Add block</button>
  {/if}
</div>
