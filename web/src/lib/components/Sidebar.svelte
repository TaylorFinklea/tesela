<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";

  let { collapsed, onToggle }: { collapsed: boolean; onToggle: () => void } = $props();
  let filter = $state("");
  let selectedIndex = $state(-1);
  let sidebarFocused = $state(false);
  let filterInput: HTMLInputElement;
  let sidebarEl: HTMLElement;

  const notesQuery = createQuery(() => ({ queryKey: ["notes", { limit: 200 }] as const, queryFn: () => api.listNotes({ limit: 200 }) }));
  const notes = $derived(notesQuery.data ?? [] as Note[]);
  const filtered = $derived(
    filter ? notes.filter((n: Note) => n.title.toLowerCase().includes(filter.toLowerCase())) : notes,
  );
  const currentPath = $derived(page.url.pathname);

  // Quick nav items that appear before pages
  const quickNav = [
    { path: "/daily", label: "Today", icon: "☀" },
    { path: "/timeline", label: "Timeline", icon: "📅" },
    { path: "/graph", label: "Graph", icon: "◉" },
  ];

  // All navigable items for j/k
  const allItems = $derived([
    ...quickNav.map((q) => ({ path: q.path, label: q.label })),
    ...filtered.map((n: Note) => ({ path: `/p/${encodeURIComponent(n.id)}`, label: n.title })),
  ]);

  function handleKeydown(e: KeyboardEvent) {
    if (!sidebarFocused) return;

    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(allItems.length - 1, selectedIndex + 1);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && allItems[selectedIndex]) {
      e.preventDefault();
      goto(allItems[selectedIndex].path);
    } else if (e.key === "/") {
      e.preventDefault();
      filterInput?.focus();
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (filter) {
        filter = "";
      } else {
        sidebarFocused = false;
        sidebarEl?.blur();
      }
    }
  }

  function handleFilterKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      filter = "";
      filterInput?.blur();
      sidebarEl?.focus();
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      filterInput?.blur();
      sidebarEl?.focus();
      selectedIndex = 0;
    } else if (e.key === "Enter" && allItems.length > 0) {
      e.preventDefault();
      goto(allItems[0].path);
      filter = "";
    }
  }
</script>

{#if collapsed}
  <div class="w-10 bg-surface border-r border-border flex flex-col items-center pt-3">
    <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-[10px] p-1.5 rounded hover:bg-accent transition-colors" title="Expand sidebar (1)">
      ▶
    </button>
  </div>
{:else}
  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    bind:this={sidebarEl}
    class="w-56 bg-surface border-r border-border flex flex-col shrink-0 select-none outline-none"
    tabindex="0"
    onfocus={() => { sidebarFocused = true; if (selectedIndex < 0) selectedIndex = 0; }}
    onblur={() => { sidebarFocused = false; }}
    onkeydown={handleKeydown}
  >
    <!-- Header -->
    <div class="flex items-center justify-between px-3 h-11 border-b border-border shrink-0">
      <a href="/" class="text-[13px] font-semibold tracking-tight text-foreground/90">Tesela</a>
      <button onclick={onToggle} class="text-muted-foreground hover:text-foreground text-[10px] p-1 rounded hover:bg-accent transition-colors" title="Collapse sidebar (1)">
        ◀
      </button>
    </div>

    <!-- Search -->
    <div class="px-2 py-2 shrink-0">
      <input
        bind:this={filterInput}
        type="text"
        placeholder="Filter… (/)"
        bind:value={filter}
        onkeydown={handleFilterKeydown}
        onfocus={() => { sidebarFocused = true; }}
        class="w-full text-[12px] bg-muted/50 rounded-md px-2.5 py-1.5 text-foreground/90 placeholder:text-muted-foreground/60 outline-none border border-transparent focus:border-ring/30 transition-colors"
      />
    </div>

    <!-- Quick nav + pages -->
    <nav class="flex-1 overflow-y-auto px-2 pb-2">
      <!-- Quick nav -->
      <div class="pb-1 space-y-px">
        {#each quickNav as item, qi}
          {@const itemIndex = qi}
          {@const isSelected = sidebarFocused && selectedIndex === itemIndex}
          {@const isActive = currentPath === item.path || (item.path === "/daily" && currentPath.startsWith("/p/20"))}
          <a
            href={item.path}
            class="flex items-center gap-2 rounded-md px-2 py-1 text-[12px] transition-colors
              {isSelected ? 'bg-accent text-accent-foreground ring-1 ring-ring/20' : ''}
              {isActive && !isSelected ? 'bg-accent/60 text-accent-foreground' : ''}
              {!isActive && !isSelected ? 'text-muted-foreground hover:text-foreground hover:bg-accent/60' : ''}"
          >
            <span class="w-4 text-center text-[11px]">{item.icon}</span>
            <span>{item.label}</span>
          </a>
        {/each}
      </div>

      <!-- Pages section -->
      <div class="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest px-2 py-1.5">
        Pages
      </div>
      {#if notesQuery.isLoading}
        <div class="px-2 py-1 text-[12px] text-muted-foreground/50">Loading…</div>
      {/if}
      {#each filtered as note, ni (note.id)}
        {@const itemIndex = quickNav.length + ni}
        {@const isSelected = sidebarFocused && selectedIndex === itemIndex}
        {@const isActive = currentPath === `/p/${encodeURIComponent(note.id)}`}
        <a
          href="/p/{encodeURIComponent(note.id)}"
          class="block rounded-md px-2 py-[5px] text-[12px] truncate transition-colors
            {isSelected ? 'bg-accent text-accent-foreground ring-1 ring-ring/20 font-medium' : ''}
            {isActive && !isSelected ? 'bg-accent/60 text-accent-foreground font-medium' : ''}
            {!isActive && !isSelected ? 'text-muted-foreground hover:text-foreground hover:bg-accent/60' : ''}"
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
      <div class="text-[10px] text-muted-foreground/40">{notes.length} notes · j/k to navigate</div>
    </div>
  </div>
{/if}
