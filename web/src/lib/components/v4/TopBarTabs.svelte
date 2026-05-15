<script lang="ts">
  /*
   * Prism v4 — the tab strip in the top bar. Tabs are tmux-style
   * windows: each holds its own pane tree + focus. Reads the pane-tree
   * store directly (it's inherently store-bound) and drives it through
   * the tab mutations. Keybindings (⌘T / ⌘⇧W / ⌥1–9) live in the v4
   * layout's keydown handler.
   */
  import {
    getState,
    switchTab,
    closeTab,
    newTab,
  } from "$lib/stores/pane-tree.svelte";
  import type { Tab } from "$lib/stores/pane-tree";

  const state = $derived(getState());

  // Compact "3e·1w·c" kind-count pill for a tab.
  function kindCounts(tab: Tab): string {
    const counts: Record<string, number> = {};
    for (const row of tab.layout) {
      for (const pane of row) counts[pane.kind] = (counts[pane.kind] ?? 0) + 1;
    }
    const parts: string[] = [];
    if (counts.editor) parts.push(`${counts.editor}e`);
    if (counts.widget) parts.push(`${counts.widget}w`);
    if (counts.context) parts.push("c");
    if (counts.graph) parts.push("g");
    if (counts.dashboard) parts.push("d");
    return parts.join("·");
  }
</script>

<div class="v4-tabs">
  {#each state.tabs as tab, i (tab.id)}
    {@const active = tab.id === state.activeTabId}
    <button
      type="button"
      class="v4-tab"
      class:active
      onclick={() => switchTab(tab.id)}
      title={tab.name}
    >
      <span class="v4-tab-idx">{i + 1}</span>
      <span class="v4-tab-name">{tab.name}</span>
      <span class="v4-tab-counts">{kindCounts(tab)}</span>
      {#if active && state.tabs.length > 1}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <span
          class="v4-tab-close"
          onclick={(e) => {
            e.stopPropagation();
            closeTab(tab.id);
          }}
          title="close tab">×</span
        >
      {/if}
    </button>
  {/each}
  <button
    type="button"
    class="v4-tab-new"
    onclick={() => newTab()}
    title="new tab · ⌘T">+</button
  >
</div>

<style>
  .v4-tabs {
    display: flex;
    align-items: stretch;
    gap: 2px;
    min-width: 0;
    overflow: hidden;
  }
  .v4-tab {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 10px;
    max-width: 200px;
    min-width: 0;
    background: transparent;
    border: 0;
    border-top: 1px solid transparent;
    color: var(--v4-ink4);
    font-family: var(--v4-sans);
    font-size: 12px;
    cursor: pointer;
    transition: background 140ms;
  }
  .v4-tab:hover {
    background: var(--v4-surface-lo);
  }
  .v4-tab.active {
    background: color-mix(in srgb, var(--v4-accent) 8%, transparent);
    border-top-color: var(--v4-accent);
    color: var(--v4-ink);
  }
  .v4-tab-idx {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink5);
    flex-shrink: 0;
  }
  .v4-tab.active .v4-tab-idx {
    color: var(--v4-accent);
  }
  .v4-tab-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v4-tab-counts {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink5);
    flex-shrink: 0;
  }
  .v4-tab-close {
    color: var(--v4-ink5);
    font-size: 13px;
    line-height: 1;
    flex-shrink: 0;
  }
  .v4-tab-close:hover {
    color: var(--v4-ink2);
  }
  .v4-tab-new {
    padding: 0 10px;
    background: transparent;
    border: 0;
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 13px;
    cursor: pointer;
  }
  .v4-tab-new:hover {
    color: var(--v4-ink2);
  }
</style>
