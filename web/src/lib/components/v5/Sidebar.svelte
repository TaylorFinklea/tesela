<script lang="ts">
  /*
   * Prism v5 — left sidebar.
   *
   * Layout: a thin always-visible toggle strip on the far left edge
   * (just a chevron when collapsed, plus the surface icons when
   * expanded), with the active surface's content area to its right.
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
  {#if sidebar.collapsed}
    <div class="v5-sidebar-collapsed">
      <button
        type="button"
        class="toggle"
        title="expand sidebar · ⌘B"
        onclick={() => setSidebarCollapsed(false)}
      >►</button>
    </div>
  {:else}
    <div class="v5-sidebar-expanded">
      <header class="v5-sidebar-header">
        <nav class="v5-sidebar-strip">
          {#each ICONS as { surface, glyph, title }}
            <button
              type="button"
              class:active={sidebar.activeSurface === surface}
              {title}
              onclick={() => setSidebarSurface(surface)}
            >{glyph}</button>
          {/each}
        </nav>
        <button
          type="button"
          class="toggle"
          title="collapse sidebar · ⌘B"
          onclick={() => setSidebarCollapsed(true)}
        >◄</button>
      </header>
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
  .v5-sidebar-collapsed {
    width: 22px;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 6px;
  }
  .v5-sidebar-expanded {
    width: 240px;
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow: hidden;
  }
  .v5-sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 4px;
    padding: 4px 6px;
    border-bottom: 1px solid var(--v4-hair);
    flex-shrink: 0;
  }
  .v5-sidebar-strip {
    display: flex;
    flex-direction: row;
    gap: 2px;
  }
  .v5-sidebar button {
    background: transparent;
    border: 0;
    color: var(--v4-ink5);
    width: 22px;
    height: 22px;
    line-height: 22px;
    text-align: center;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    padding: 0;
  }
  .v5-sidebar button:hover {
    color: var(--v4-ink2);
    background: var(--v4-surface-lo);
  }
  .v5-sidebar-strip button.active {
    color: var(--v4-accent);
    background: color-mix(in srgb, var(--v4-accent) 14%, transparent);
  }
  .v5-sidebar-header .toggle {
    color: var(--v4-ink6);
  }
  .v5-sidebar-content {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
