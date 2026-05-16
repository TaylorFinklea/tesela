<script lang="ts">
  /*
   * Prism v5 — left sidebar.
   *
   * NvimTree-shaped: a thin icon strip at the very left edge persists
   * when the sidebar is collapsed; clicking an icon swaps which content
   * surface is mounted. Five surfaces: tree, search, recent, pinned,
   * tags. Plus a ◄ button at the bottom of the strip to collapse.
   *
   * State (collapsed + active surface) lives on the Workspace and
   * persists alongside the pane tree.
   */
  import {
    getWorkspace,
    setSidebarCollapsed,
    setSidebarSurface,
  } from "$lib/buffer/state.svelte";
  import NotesTree from "./sidebar/NotesTree.svelte";
  import SearchSurface from "./sidebar/SearchSurface.svelte";
  import RecentSurface from "./sidebar/RecentSurface.svelte";
  import PinnedSurface from "./sidebar/PinnedSurface.svelte";
  import TagsSurface from "./sidebar/TagsSurface.svelte";
  import type { SidebarSurface } from "$lib/buffer/types";

  const sidebar = $derived(getWorkspace().sidebar);

  const ICONS: { surface: SidebarSurface; glyph: string; title: string }[] = [
    { surface: "tree", glyph: "☰", title: "notes tree" },
    { surface: "search", glyph: "⚲", title: "search" },
    { surface: "recent", glyph: "⏲", title: "recent" },
    { surface: "pinned", glyph: "☆", title: "pinned" },
    { surface: "tags", glyph: "♬", title: "tags" },
  ];
</script>

<aside class="v5-sidebar" class:collapsed={sidebar.collapsed}>
  <nav class="v5-sidebar-strip">
    {#each ICONS as { surface, glyph, title }}
      <button
        type="button"
        class:active={sidebar.activeSurface === surface && !sidebar.collapsed}
        {title}
        onclick={() => {
          if (sidebar.collapsed) {
            setSidebarCollapsed(false);
            setSidebarSurface(surface);
          } else if (sidebar.activeSurface === surface) {
            setSidebarCollapsed(true);
          } else {
            setSidebarSurface(surface);
          }
        }}
      >{glyph}</button>
    {/each}
    <button
      type="button"
      class="collapse"
      title={sidebar.collapsed ? "expand sidebar · ⌘B" : "collapse sidebar · ⌘B"}
      onclick={() => setSidebarCollapsed(!sidebar.collapsed)}
    >{sidebar.collapsed ? "►" : "◄"}</button>
  </nav>
  {#if !sidebar.collapsed}
    <div class="v5-sidebar-content">
      {#if sidebar.activeSurface === "tree"}
        <NotesTree />
      {:else if sidebar.activeSurface === "search"}
        <SearchSurface />
      {:else if sidebar.activeSurface === "recent"}
        <RecentSurface />
      {:else if sidebar.activeSurface === "pinned"}
        <PinnedSurface />
      {:else if sidebar.activeSurface === "tags"}
        <TagsSurface />
      {/if}
    </div>
  {/if}
</aside>

<style>
  .v5-sidebar {
    display: flex;
    background: var(--v4-bg);
    border-right: 1px solid var(--v4-hair);
    min-height: 0;
    overflow: hidden;
  }
  .v5-sidebar-strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    width: 36px;
    flex-shrink: 0;
    border-right: 1px solid var(--v4-hair);
    padding: 6px 0;
    gap: 2px;
  }
  .v5-sidebar-strip button {
    background: transparent;
    border: 0;
    color: var(--v4-ink5);
    width: 28px;
    height: 28px;
    line-height: 28px;
    text-align: center;
    border-radius: 5px;
    cursor: pointer;
    font-size: 14px;
    padding: 0;
  }
  .v5-sidebar-strip button:hover {
    color: var(--v4-ink2);
    background: var(--v4-surface-lo);
  }
  .v5-sidebar-strip button.active {
    color: var(--v4-accent);
    background: color-mix(in srgb, var(--v4-accent) 12%, transparent);
  }
  .v5-sidebar-strip .collapse {
    margin-top: auto;
    color: var(--v4-ink6);
    font-size: 12px;
  }
  .v5-sidebar-content {
    width: 240px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
</style>
