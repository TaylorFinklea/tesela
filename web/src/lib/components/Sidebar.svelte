<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";

  let { collapsed, onToggle }: { collapsed: boolean; onToggle: () => void } = $props();
  let filter = $state("");

  const notesQuery = createQuery(() => ({ queryKey: ["notes", { limit: 200 }] as const, queryFn: () => api.listNotes({ limit: 200 }) }));
  const notes = $derived(notesQuery.data ?? [] as Note[]);
  const filtered = $derived(
    filter ? notes.filter((n) => n.title.toLowerCase().includes(filter.toLowerCase())) : notes,
  );
  const currentPath = $derived(page.url.pathname);
</script>

{#if collapsed}
  <div class="w-10 border-r border-border flex flex-col items-center py-3">
    <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-xs" title="Expand sidebar">▶</button>
  </div>
{:else}
  <div class="w-60 border-r border-border flex flex-col shrink-0">
    <div class="flex items-center justify-between px-3 py-3 border-b border-border">
      <a href="/" class="text-sm font-medium tracking-tight">Tesela</a>
      <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-xs" title="Collapse sidebar">◀</button>
    </div>

    <div class="px-3 py-2">
      <input
        type="text"
        placeholder="Filter pages…"
        bind:value={filter}
        class="w-full text-xs bg-accent/50 rounded px-2 py-1.5 text-foreground placeholder:text-muted-foreground outline-none focus:ring-1 focus:ring-ring/30"
      />
    </div>

    <div class="px-1.5 py-1 space-y-0.5">
      <a href="/daily" class="flex items-center gap-2 rounded px-1.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors {currentPath.startsWith('/p/20') ? 'bg-accent text-accent-foreground' : ''}">
        <span>☀</span> Today
      </a>
      <a href="/timeline" class="flex items-center gap-2 rounded px-1.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors {currentPath === '/timeline' ? 'bg-accent text-accent-foreground' : ''}">
        <span>📅</span> Timeline
      </a>
      <a href="/graph" class="flex items-center gap-2 rounded px-1.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors {currentPath === '/graph' ? 'bg-accent text-accent-foreground' : ''}">
        <span>◉</span> Graph
      </a>
    </div>

    <nav class="flex-1 overflow-y-auto px-1.5 pb-2">
      <div class="text-[10px] font-medium text-muted-foreground uppercase tracking-wider px-1.5 py-1.5">Pages</div>
      {#if notesQuery.isLoading}
        <div class="px-1.5 py-1 text-xs text-muted-foreground">Loading…</div>
      {/if}
      {#each filtered as note (note.id)}
        {@const isActive = currentPath === `/p/${encodeURIComponent(note.id)}`}
        <a
          href="/p/{encodeURIComponent(note.id)}"
          class="block rounded px-1.5 py-1 text-xs truncate transition-colors {isActive ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground hover:bg-accent/50'}"
        >
          {note.title}
        </a>
      {/each}
      {#if filtered.length === 0 && !notesQuery.isLoading}
        <div class="px-1.5 py-1 text-xs text-muted-foreground">{filter ? "No matches" : "No notes"}</div>
      {/if}
    </nav>

    <div class="border-t border-border px-3 py-2">
      <div class="text-[10px] text-muted-foreground">{notes.length} notes</div>
    </div>
  </div>
{/if}
