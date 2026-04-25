<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { parseBlocks } from "$lib/block-parser";
  import { parseQuery, blockMatches } from "$lib/query-language";
  import type { Note } from "$lib/types/Note";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";

  let { block }: { block: ParsedBlock } = $props();

  const queryText = $derived((block.properties.query ?? "").trim());
  const viewMode = $derived((block.properties.view ?? "table").trim().toLowerCase());

  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: queryText.length > 0,
  }));

  const parsedQuery = $derived(parseQuery(queryText));

  const matches: { block: ParsedBlock; noteTitle: string; noteId: string }[] = $derived.by(() => {
    if (!queryText) return [];
    const notes = (allNotesQuery.data ?? []) as Note[];
    const out: { block: ParsedBlock; noteTitle: string; noteId: string }[] = [];
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

  // Collect property columns (union of all property keys from matched blocks)
  const propColumns = $derived.by(() => {
    const keys = new Set<string>();
    for (const m of matches) {
      for (const k of Object.keys(m.block.properties)) {
        if (k !== "query" && k !== "view") keys.add(k);
      }
    }
    return [...keys].sort();
  });

  const groupBy = $derived(viewMode === "kanban" ? (propColumns.includes("status") ? "status" : propColumns[0]) : null);
  const kanbanGroups = $derived.by(() => {
    if (!groupBy) return [];
    const buckets = new Map<string, typeof matches>();
    for (const m of matches) {
      const key = m.block.properties[groupBy] ?? "—";
      const arr = buckets.get(key) ?? [];
      arr.push(m);
      buckets.set(key, arr);
    }
    return [...buckets.entries()].sort(([a], [b]) => a.localeCompare(b));
  });
</script>

<div class="ml-6 mt-1 mb-3 p-3 rounded-md bg-muted/20 border border-border/40">
  {#if !queryText}
    <div class="text-[11px] text-muted-foreground/60 italic">Empty query — set <code>query::</code> on this block.</div>
  {:else if allNotesQuery.isLoading}
    <div class="text-[11px] text-muted-foreground">Loading…</div>
  {:else if matches.length === 0}
    <div class="text-[11px] text-muted-foreground/60 italic">No matches for <code>{queryText}</code></div>
  {:else if viewMode === "kanban"}
    <div class="flex gap-3 overflow-x-auto">
      {#each kanbanGroups as [colName, items]}
        <div class="shrink-0 w-[200px]">
          <div class="text-[10px] font-semibold text-muted-foreground/70 uppercase tracking-widest mb-2 px-1">
            {colName} <span class="text-muted-foreground/40">({items.length})</span>
          </div>
          <div class="space-y-1">
            {#each items as m}
              <a
                href="/p/{m.noteId}?block={encodeURIComponent(m.block.id)}"
                class="block p-2 rounded bg-surface border border-border/40 hover:border-primary/40 transition-colors"
              >
                <div class="text-[12px] text-foreground/80 line-clamp-2">{m.block.text || "(empty)"}</div>
                <div class="flex items-center gap-1 mt-1 text-[9px] text-muted-foreground/60">
                  <span>{m.noteTitle}</span>
                  {#if m.block.tags.length > 0}
                    <span>·</span>
                    {#each m.block.tags.slice(0, 2) as t}<span class="text-primary/60">#{t}</span>{/each}
                  {/if}
                </div>
              </a>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {:else}
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
          <tr class="border-t border-border/30 hover:bg-muted/30 transition-colors">
            <td class="px-2 py-1.5">
              <a
                href="/p/{m.noteId}?block={encodeURIComponent(m.block.id)}"
                class="text-foreground/80 hover:text-primary transition-colors"
              >{m.block.text || "(empty)"}</a>
            </td>
            <td class="px-2 py-1.5 text-muted-foreground/70">
              <a href="/p/{m.noteId}" class="hover:text-primary transition-colors">{m.noteTitle}</a>
            </td>
            <td class="px-2 py-1.5">
              <div class="flex flex-wrap gap-0.5">
                {#each m.block.tags as t}
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70">{t}</span>
                {/each}
              </div>
            </td>
            {#each propColumns as col}
              <td class="px-2 py-1.5 text-foreground/70">{m.block.properties[col] ?? ""}</td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>
