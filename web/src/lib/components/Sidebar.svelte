<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { getRecents } from "$lib/stores/recents.svelte";
  import { getFavorites } from "$lib/stores/favorites.svelte";
  import type { Note } from "$lib/types/Note";
  import { IconSun, IconCalendarEvent, IconGraph, IconSettings, IconChevronLeft, IconChevronRight, IconClock, IconFile, IconStar } from "@tabler/icons-svelte";

  let { collapsed, onToggle }: { collapsed: boolean; onToggle: () => void } = $props();
  let selectedIndex = $state(-1);
  let sidebarFocused = $state(false);
  let sidebarEl = $state<HTMLElement | undefined>(undefined);

  const notesQuery = createQuery(() => ({ queryKey: ["notes", { limit: 200 }] as const, queryFn: () => api.listNotes({ limit: 200 }) }));
  const notes = $derived(notesQuery.data ?? [] as Note[]);
  const currentPath = $derived(page.url.pathname);
  const favoriteNotes: Note[] = $derived(
    getFavorites()
      .map((id: string) => notes.find((n: Note) => n.id === id))
      .filter((n): n is Note => n !== undefined),
  );
  const recentNotes: Note[] = $derived(
    getRecents().slice(0, 5)
      .map((id: string) => notes.find((n: Note) => n.id === id))
      .filter((n): n is Note => n !== undefined),
  );

  const iconSize = 16;
  const iconStroke = 1.5;
  const navItems = [
    { path: "/daily", label: "Today", match: (p: string) => p === "/daily" || p.startsWith("/p/20") },
    { path: "/timeline", label: "Timeline", match: (p: string) => p === "/timeline" },
    { path: "/graph", label: "Graph", match: (p: string) => p === "/graph" },
    { path: "/", label: "Pages", match: (p: string) => p === "/" },
  ];

  const allItems = $derived([
    ...navItems.map((item) => ({ path: item.path, label: item.label })),
    ...favoriteNotes.map((n: Note) => ({ path: `/p/${encodeURIComponent(n.id)}`, label: n.title })),
    ...recentNotes.map((n: Note) => ({ path: `/p/${encodeURIComponent(n.id)}`, label: n.title })),
  ]);

  function handleKeydown(e: KeyboardEvent) {
    if (!sidebarFocused) return;
    if (e.key === "j" || e.key === "ArrowDown") { e.preventDefault(); selectedIndex = Math.min(allItems.length - 1, selectedIndex + 1); }
    else if (e.key === "k" || e.key === "ArrowUp") { e.preventDefault(); selectedIndex = Math.max(0, selectedIndex - 1); }
    else if (e.key === "Enter" && allItems[selectedIndex]) { e.preventDefault(); goto(allItems[selectedIndex].path); }
    else if (e.key === "Escape") { e.preventDefault(); sidebarFocused = false; sidebarEl?.blur(); }
  }
</script>

{#if collapsed}
  <div class="w-10 bg-surface border-r border-border flex flex-col items-center pt-4">
    <button onclick={onToggle} class="text-muted-foreground hover:text-primary p-1.5 rounded-md hover:bg-muted transition-all" title="Expand (1)">
      <IconChevronRight size={14} stroke={1.5} />
    </button>
  </div>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    bind:this={sidebarEl}
    class="w-[220px] bg-surface border-r border-border flex flex-col shrink-0 select-none outline-none"
    tabindex="0"
    onfocus={() => { sidebarFocused = true; if (selectedIndex < 0) selectedIndex = 0; }}
    onblur={() => { sidebarFocused = false; }}
    onkeydown={handleKeydown}
  >
    <!-- Brand -->
    <div class="flex items-center justify-between px-4 h-[52px] shrink-0">
      <a href="/" class="font-display text-lg font-semibold tracking-tight text-foreground">Tesela</a>
      <button onclick={onToggle} class="text-muted-foreground hover:text-primary p-1 rounded-md hover:bg-muted transition-all" title="Collapse (1)">
        <IconChevronLeft size={14} stroke={1.5} />
      </button>
    </div>

    <!-- Nav -->
    <div class="px-2 pb-2 space-y-px shrink-0">
      {#each navItems as item, qi}
        {@const isSelected = sidebarFocused && selectedIndex === qi}
        {@const isActive = item.match(currentPath)}
        <a
          href={item.path}
          class="flex items-center gap-2.5 rounded-lg px-3 py-[7px] text-[12px] transition-all
            {isSelected ? 'bg-primary/10 text-primary ring-1 ring-primary/20' : ''}
            {isActive && !isSelected ? 'bg-muted/60 text-foreground/90' : ''}
            {!isActive && !isSelected ? 'text-muted-foreground hover:text-foreground/80 hover:bg-muted/40' : ''}"
        >
          <span class="w-4 text-primary/60">
            {#if qi === 0}<IconSun size={iconSize} stroke={iconStroke} />
            {:else if qi === 1}<IconCalendarEvent size={iconSize} stroke={iconStroke} />
            {:else if qi === 2}<IconGraph size={iconSize} stroke={iconStroke} />
            {:else}<IconFile size={iconSize} stroke={iconStroke} />
            {/if}
          </span>
          <span>{item.label}</span>
        </a>
      {/each}
    </div>

    <!-- Scrollable area: favorites + recents -->
    <nav class="flex-1 overflow-y-auto px-2 pb-2">
      {#if favoriteNotes.length > 0}
        <div class="flex items-center gap-1.5 text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] px-3 pt-2 pb-1.5">
          <IconStar size={11} stroke={1.5} class="text-primary/30" /> Favorites
        </div>
        {#each favoriteNotes as note, fi (note.id)}
          {@const itemIndex = navItems.length + fi}
          {@const isSelected = sidebarFocused && selectedIndex === itemIndex}
          {@const isActive = currentPath === `/p/${encodeURIComponent(note.id)}`}
          <a
            href="/p/{encodeURIComponent(note.id)}"
            class="block rounded-lg px-3 py-[5px] text-[12px] truncate transition-all
              {isSelected ? 'bg-primary/10 text-primary ring-1 ring-primary/15 font-medium' : ''}
              {isActive && !isSelected ? 'bg-muted/60 text-foreground/90 font-medium' : ''}
              {!isActive && !isSelected ? 'text-muted-foreground/80 hover:text-foreground/70 hover:bg-muted/30' : ''}"
          >{note.title}</a>
        {/each}
      {/if}

      {#if recentNotes.length > 0}
        <div class="flex items-center gap-1.5 text-[10px] font-semibold text-muted-foreground/60 uppercase tracking-[0.12em] px-3 pt-2 pb-1.5">
          <IconClock size={11} stroke={1.5} class="text-primary/30" /> Recent
        </div>
        {#each recentNotes as note, ni (note.id)}
          {@const itemIndex = navItems.length + favoriteNotes.length + ni}
          {@const isSelected = sidebarFocused && selectedIndex === itemIndex}
          {@const isActive = currentPath === `/p/${encodeURIComponent(note.id)}`}
          <a
            href="/p/{encodeURIComponent(note.id)}"
            class="block rounded-lg px-3 py-[5px] text-[12px] truncate transition-all
              {isSelected ? 'bg-primary/10 text-primary ring-1 ring-primary/15 font-medium' : ''}
              {isActive && !isSelected ? 'bg-muted/60 text-foreground/90 font-medium' : ''}
              {!isActive && !isSelected ? 'text-muted-foreground/80 hover:text-foreground/70 hover:bg-muted/30' : ''}"
          >{note.title}</a>
        {/each}
      {/if}
    </nav>

    <!-- Footer -->
    <div class="border-t border-border/50 px-2 py-1.5 shrink-0 space-y-px">
      <a
        href="/settings"
        class="flex items-center gap-2.5 rounded-lg px-3 py-[6px] text-[11px] text-muted-foreground/60 hover:text-foreground/70 hover:bg-muted/30 transition-all {currentPath === '/settings' ? 'bg-muted/50 text-foreground/70' : ''}"
      >
        <span class="w-4 text-primary/30"><IconSettings size={14} stroke={1.5} /></span> Settings
      </a>
      <div class="text-[10px] text-muted-foreground/50 px-3 py-0.5">{notes.length} notes</div>
    </div>
  </div>
{/if}
