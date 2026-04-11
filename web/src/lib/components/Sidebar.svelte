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
    filter ? notes.filter((n: Note) => n.title.toLowerCase().includes(filter.toLowerCase())) : notes,
  );
  const currentPath = $derived(page.url.pathname);
</script>

{#if collapsed}
  <div class="w-10 bg-surface border-r border-border flex flex-col items-center pt-3">
    <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-[10px] p-1.5 rounded hover:bg-accent transition-colors" title="Expand sidebar">
      ▶
    </button>
  </div>
{:else}
  <div class="w-56 bg-surface border-r border-border flex flex-col shrink-0 select-none">
    <!-- Header -->
    <div class="flex items-center justify-between px-3 h-11 border-b border-border shrink-0">
      <a href="/" class="text-[13px] font-semibold tracking-tight text-foreground/90">Tesela</a>
      <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-[10px] p-1 rounded hover:bg-accent transition-colors" title="Collapse sidebar">
        ◀
      </button>
    </div>

    <!-- Search -->
    <div class="px-2 py-2 shrink-0">
      <input
        type="text"
        placeholder="Filter…"
        bind:value={filter}
        class="w-full text-[12px] bg-muted/50 rounded-md px-2.5 py-1.5 text-foreground/90 placeholder:text-muted-foreground/60 outline-none border border-transparent focus:border-ring/30 transition-colors"
      />
    </div>

    <!-- Quick nav -->
    <div class="px-2 pb-1 space-y-px shrink-0">
      <a href="/daily" class="flex items-center gap-2 rounded-md px-2 py-1 text-[12px] transition-colors {currentPath.startsWith('/p/20') ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground hover:bg-accent/60'}">
        <span class="w-4 text-center text-[11px]">☀</span>
        <span>Today</span>
      </a>
      <a href="/timeline" class="flex items-center gap-2 rounded-md px-2 py-1 text-[12px] transition-colors {currentPath === '/timeline' ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground hover:bg-accent/60'}">
        <span class="w-4 text-center text-[11px]">📅</span>
        <span>Timeline</span>
      </a>
      <a href="/graph" class="flex items-center gap-2 rounded-md px-2 py-1 text-[12px] transition-colors {currentPath === '/graph' ? 'bg-accent text-accent-foreground' : 'text-muted-foreground hover:text-foreground hover:bg-accent/60'}">
        <span class="w-4 text-center text-[11px]">◉</span>
        <span>Graph</span>
      </a>
    </div>

    <!-- Pages list -->
    <nav class="flex-1 overflow-y-auto px-2 pt-1 pb-2">
      <div class="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest px-2 py-1.5">
        Pages
      </div>
      {#if notesQuery.isLoading}
        <div class="px-2 py-1 text-[12px] text-muted-foreground/50">Loading…</div>
      {/if}
      {#each filtered as note (note.id)}
        {@const isActive = currentPath === `/p/${encodeURIComponent(note.id)}`}
        <a
          href="/p/{encodeURIComponent(note.id)}"
          class="block rounded-md px-2 py-[5px] text-[12px] truncate transition-colors {isActive ? 'bg-accent text-accent-foreground font-medium' : 'text-muted-foreground hover:text-foreground hover:bg-accent/60'}"
        >
          {note.title}
        </a>
      {/each}
      {#if filtered.length === 0 && !notesQuery.isLoading}
        <div class="px-2 py-1 text-[12px] text-muted-foreground/50">{filter ? "No matches" : "No notes"}</div>
      {/if}
    </nav>

    <!-- Footer -->
    <div class="border-t border-border px-3 py-2 shrink-0">
      <div class="text-[10px] text-muted-foreground/40">{notes.length} notes</div>
    </div>
  </div>
{/if}
