<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { getRecents } from "$lib/stores/recents.svelte";
  import type { Note } from "$lib/types/Note";
  import { IconSun, IconCalendarEvent, IconGraph, IconSettings, IconChevronLeft, IconChevronRight, IconClock, IconFile } from "@tabler/icons-svelte";

  let { collapsed, onToggle }: { collapsed: boolean; onToggle: () => void } = $props();
  let filter = $state("");
  let selectedIndex = $state(-1);
  let sidebarFocused = $state(false);
  let filterInput = $state<HTMLInputElement | undefined>(undefined);
  let sidebarEl = $state<HTMLElement | undefined>(undefined);

  const notesQuery = createQuery(() => ({ queryKey: ["notes", { limit: 200 }] as const, queryFn: () => api.listNotes({ limit: 200 }) }));
  const notes = $derived(notesQuery.data ?? [] as Note[]);
  const filtered = $derived(filter ? notes.filter((n: Note) => n.title.toLowerCase().includes(filter.toLowerCase())) : notes);
  const currentPath = $derived(page.url.pathname);
  const recentNotes: Note[] = $derived(
    getRecents().slice(0, 5).map((id: string) => notes.find((n: Note) => n.id === id)).filter((n): n is Note => n !== undefined),
  );

  const navItems = [
    { path: "/daily", label: "Today", match: (p: string) => p === "/daily" || p.startsWith("/p/20") },
    { path: "/timeline", label: "Timeline", match: (p: string) => p === "/timeline" },
    { path: "/graph", label: "Graph", match: (p: string) => p === "/graph" },
  ];

  const allItems = $derived([
    { path: "/daily", label: "Today" }, { path: "/timeline", label: "Timeline" }, { path: "/graph", label: "Graph" },
    ...filtered.map((n: Note) => ({ path: `/p/${encodeURIComponent(n.id)}`, label: n.title })),
  ]);

  function handleKeydown(e: KeyboardEvent) {
    if (!sidebarFocused) return;
    if (e.key === "j" || e.key === "ArrowDown") { e.preventDefault(); selectedIndex = Math.min(allItems.length - 1, selectedIndex + 1); }
    else if (e.key === "k" || e.key === "ArrowUp") { e.preventDefault(); selectedIndex = Math.max(0, selectedIndex - 1); }
    else if (e.key === "Enter" && allItems[selectedIndex]) { e.preventDefault(); goto(allItems[selectedIndex].path); }
    else if (e.key === "/") { e.preventDefault(); filterInput?.focus(); }
    else if (e.key === "Escape") { e.preventDefault(); if (filter) filter = ""; else { sidebarFocused = false; sidebarEl?.blur(); } }
  }

  function handleFilterKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") { e.preventDefault(); filter = ""; filterInput?.blur(); sidebarEl?.focus(); }
    else if (e.key === "ArrowDown") { e.preventDefault(); filterInput?.blur(); sidebarEl?.focus(); selectedIndex = 0; }
    else if (e.key === "Enter" && allItems.length > 0) { e.preventDefault(); goto(allItems[0].path); filter = ""; }
  }
</script>

{#if collapsed}
  <div class="w-12 bg-surface border-r border-border flex flex-col items-center pt-4">
    <button onclick={onToggle} class="text-muted-foreground hover:text-primary p-2 rounded-lg hover:bg-accent transition-all" title="Expand (1)">
      <IconChevronRight size={16} stroke={1.5} />
    </button>
  </div>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    bind:this={sidebarEl}
    class="w-[240px] bg-surface border-r border-border flex flex-col shrink-0 select-none outline-none"
    tabindex="0"
    onfocus={() => { sidebarFocused = true; if (selectedIndex < 0) selectedIndex = 0; }}
    onblur={() => { sidebarFocused = false; }}
    onkeydown={handleKeydown}
  >
    <!-- Brand -->
    <div class="flex items-center justify-between px-5 h-14 shrink-0">
      <a href="/" class="font-display text-xl font-semibold tracking-tight text-foreground">Tesela</a>
      <button onclick={onToggle} class="text-muted-foreground hover:text-primary p-1.5 rounded-lg hover:bg-accent transition-all" title="Collapse (1)">
        <IconChevronLeft size={14} stroke={1.5} />
      </button>
    </div>

    <!-- Search -->
    <div class="px-4 pb-3 shrink-0">
      <input
        bind:this={filterInput}
        type="text"
        placeholder="Search notes…"
        bind:value={filter}
        onkeydown={handleFilterKeydown}
        onfocus={() => { sidebarFocused = true; }}
        class="w-full text-[13px] font-sans bg-accent/60 rounded-lg px-3.5 py-2.5 text-foreground placeholder:text-muted-foreground outline-none border border-border focus:border-primary/40 focus:ring-2 focus:ring-primary/10 transition-all"
      />
    </div>

    <!-- Quick nav -->
    <div class="px-3 pb-3 space-y-0.5 shrink-0">
      {#each navItems as item, qi}
        {@const isSelected = sidebarFocused && selectedIndex === qi}
        {@const isActive = item.match(currentPath)}
        <a
          href={item.path}
          class="flex items-center gap-3 rounded-lg px-3 py-2 text-[13px] font-medium transition-all
            {isSelected ? 'bg-primary/10 text-primary ring-1 ring-primary/20' : ''}
            {isActive && !isSelected ? 'bg-accent text-foreground' : ''}
            {!isActive && !isSelected ? 'text-muted-foreground hover:text-foreground hover:bg-accent/60' : ''}"
        >
          <span class="text-primary/50">
            {#if qi === 0}<IconSun size={18} stroke={1.5} />
            {:else if qi === 1}<IconCalendarEvent size={18} stroke={1.5} />
            {:else}<IconGraph size={18} stroke={1.5} />
            {/if}
          </span>
          <span>{item.label}</span>
        </a>
      {/each}
    </div>

    <!-- Scrollable: recents + pages -->
    <nav class="flex-1 overflow-y-auto px-3 pb-3">
      <!-- Recents -->
      {#if recentNotes.length > 0 && !filter}
        <div class="flex items-center gap-2 text-[10px] font-semibold text-muted-foreground uppercase tracking-[0.15em] px-3 pt-3 pb-2">
          <IconClock size={11} stroke={1.5} class="text-primary/40" />
          <span>Recent</span>
        </div>
        {#each recentNotes as note (note.id)}
          {@const isActive = currentPath === `/p/${encodeURIComponent(note.id)}`}
          <a
            href="/p/{encodeURIComponent(note.id)}"
            class="block rounded-lg px-3 py-2 text-[13px] truncate transition-all
              {isActive ? 'bg-accent text-foreground font-medium' : 'text-muted-foreground hover:text-foreground hover:bg-accent/60'}"
          >{note.title}</a>
        {/each}
      {/if}

      <!-- Pages -->
      <div class="flex items-center gap-2 text-[10px] font-semibold text-muted-foreground uppercase tracking-[0.15em] px-3 pt-4 pb-2">
        <IconFile size={11} stroke={1.5} class="text-primary/40" />
        <span>{filter ? "Results" : "Pages"}</span>
      </div>
      {#if notesQuery.isLoading}
        <div class="px-3 py-2 text-[13px] text-muted-foreground">Loading…</div>
      {/if}
      {#each filtered as note, ni (note.id)}
        {@const itemIndex = 3 + ni}
        {@const isSelected = sidebarFocused && selectedIndex === itemIndex}
        {@const isActive = currentPath === `/p/${encodeURIComponent(note.id)}`}
        <a
          href="/p/{encodeURIComponent(note.id)}"
          class="block rounded-lg px-3 py-2 text-[13px] truncate transition-all
            {isSelected ? 'bg-primary/10 text-primary ring-1 ring-primary/15 font-medium' : ''}
            {isActive && !isSelected ? 'bg-accent text-foreground font-medium' : ''}
            {!isActive && !isSelected ? 'text-muted-foreground hover:text-foreground hover:bg-accent/60' : ''}"
        >{note.title}</a>
      {/each}
      {#if filtered.length === 0 && !notesQuery.isLoading}
        <div class="px-3 py-2 text-[13px] text-muted-foreground">{filter ? "No matches" : "No notes"}</div>
      {/if}
    </nav>

    <!-- Footer -->
    <div class="border-t border-border px-3 py-2 shrink-0 space-y-0.5">
      <a
        href="/settings"
        class="flex items-center gap-3 rounded-lg px-3 py-2 text-[12px] text-muted-foreground hover:text-foreground hover:bg-accent/60 transition-all {currentPath === '/settings' ? 'bg-accent text-foreground' : ''}"
      >
        <IconSettings size={15} stroke={1.5} class="text-primary/40" /> Settings
      </a>
      <div class="text-[10px] text-muted-foreground/60 px-3 py-1">{notes.length} notes</div>
    </div>
  </div>
{/if}
