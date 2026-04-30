<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { goto } from "$app/navigation";
  import { api } from "$lib/api-client";
  import { getRecents } from "$lib/stores/recents.svelte";
  import { getFavorites } from "$lib/stores/favorites.svelte";
  import { getActiveRegion, setActiveRegion } from "$lib/stores/pane-state.svelte";
  import type { Note } from "$lib/types/Note";

  type RailItem = {
    label: string;
    href: string;
    icon: string; // glyph data-icon key
    badge?: string;
    match?: (p: string) => boolean;
  };

  const railFocused = $derived(getActiveRegion() === "rail");
  let rootEl = $state<HTMLElement | undefined>();
  let selectedIndex = $state(-1);

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
  }));
  const notes = $derived((notesQuery.data ?? []) as Note[]);
  const currentPath = $derived(page.url.pathname);

  const favoriteNotes: Note[] = $derived(
    getFavorites()
      .map((id: string) => notes.find((n) => n.id === id))
      .filter((n): n is Note => n !== undefined),
  );
  const recentNotes: Note[] = $derived(
    getRecents().slice(0, 5)
      .map((id: string) => notes.find((n) => n.id === id))
      .filter((n): n is Note => n !== undefined),
  );

  const pinned: RailItem[] = [
    { label: "Today", href: "/daily", icon: "calendar", match: (p) => p === "/daily" },
    { label: "Pages", href: "/", icon: "cal", match: (p) => p === "/" },
  ];
  const browse: RailItem[] = [
    { label: "Timeline", href: "/timeline", icon: "clock", match: (p) => p === "/timeline" },
    { label: "Graph", href: "/graph", icon: "query", match: (p) => p === "/graph" },
    { label: "Properties", href: "/properties", icon: "project", match: (p) => p === "/properties" },
  ];
  const saved: RailItem[] = $derived([
    ...favoriteNotes.map((n): RailItem => ({
      label: n.title,
      href: `/p/${encodeURIComponent(n.id)}`,
      icon: "pin",
      match: (p) => p === `/p/${encodeURIComponent(n.id)}`,
    })),
    ...recentNotes.map((n): RailItem => ({
      label: n.title,
      href: `/p/${encodeURIComponent(n.id)}`,
      icon: "clock",
      match: (p) => p === `/p/${encodeURIComponent(n.id)}`,
    })),
  ]);

  const allItems = $derived<RailItem[]>([...pinned, ...browse, ...saved]);

  function isActive(item: RailItem): boolean {
    return item.match ? item.match(currentPath) : item.href === currentPath;
  }

  $effect(() => {
    if (railFocused) {
      if (rootEl && document.activeElement !== rootEl) rootEl.focus();
      if (selectedIndex < 0) selectedIndex = 0;
    } else if (rootEl && document.activeElement === rootEl) {
      rootEl.blur();
    }
  });

  function handleKeydown(e: KeyboardEvent) {
    if (!railFocused) return;
    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      selectedIndex = Math.min(allItems.length - 1, selectedIndex + 1);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      selectedIndex = Math.max(0, selectedIndex - 1);
    } else if (e.key === "Enter" && allItems[selectedIndex]) {
      e.preventDefault();
      goto(allItems[selectedIndex].href);
      setActiveRegion("focus");
    } else if (e.key === "Escape") {
      e.preventDefault();
      setActiveRegion("focus");
    }
  }

  function rowClass(item: RailItem, idx: number): string {
    const sel = railFocused && selectedIndex === idx;
    const active = isActive(item);
    return `w ${active || sel ? "active" : ""}`;
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={rootEl}
  class="v9-rail"
  tabindex="0"
  onfocus={() => { setActiveRegion("rail"); if (selectedIndex < 0) selectedIndex = 0; }}
  onclick={() => setActiveRegion("rail")}
  onkeydown={handleKeydown}
  style="outline: none;"
>
  <div class="v9-rail-scroll">
    <div class="group">Pinned</div>
    {#each pinned as item, qi}
      {@const idx = qi}
      <a href={item.href} class={rowClass(item, idx)} data-icon={item.icon}>
        <span class="gl">{item.label[0]}</span>
        <span>{item.label}</span>
        <span class="badge">{item.badge ?? ""}</span>
        <span class="caret"></span>
      </a>
    {/each}

    <div class="group">Browse</div>
    {#each browse as item, qi}
      {@const idx = pinned.length + qi}
      <a href={item.href} class={rowClass(item, idx)} data-icon={item.icon}>
        <span class="gl">{item.label[0]}</span>
        <span>{item.label}</span>
        <span class="badge">{item.badge ?? ""}</span>
        <span class="caret"></span>
      </a>
    {/each}

    {#if saved.length > 0}
      <div class="group">Saved</div>
      {#each saved as item, qi}
        {@const idx = pinned.length + browse.length + qi}
        <a href={item.href} class={rowClass(item, idx)} data-icon={item.icon}>
          <span class="gl">{item.label[0]}</span>
          <span>{item.label}</span>
          <span class="badge">{item.badge ?? ""}</span>
          <span class="caret"></span>
        </a>
      {/each}
    {/if}
  </div>

  <!-- Settings footer (kept simple — full mini-cal is deferred per spec) -->
  <div style="border-top: 1px solid var(--v9-line); padding: 6px 6px;">
    <a
      href="/settings"
      class="w {currentPath === '/settings' ? 'active' : ''}"
      data-icon="project"
    >
      <span class="gl">S</span>
      <span>Settings</span>
      <span class="badge">{notes.length}</span>
      <span class="caret"></span>
    </a>
  </div>
</div>
