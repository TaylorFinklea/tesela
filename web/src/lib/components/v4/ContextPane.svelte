<script lang="ts">
  /*
   * Prism v4 `context` pane body — a tabbed panel of context for the
   * note a v4 editor pane is showing. Replaces the legacy BottomDrawer's
   * fixed tabs. The pane shell decides *which* note this follows (the
   * tab's most-recently-focused editor pane, unless the context pane was
   * explicitly pinned to a tile) and passes it in as `noteId`.
   *
   * Backlinks / Outline / Properties are v4-native lightweight tabs;
   * History / LinkedTasks reuse the existing standalone components
   * (they already take a `noteId` prop). The legacy editable property
   * panel is not ported — Properties here is read-only for now.
   */
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import BacklinksTab from "./BacklinksTab.svelte";
  import OutlineTab from "./OutlineTab.svelte";
  import PropertiesView from "./PropertiesView.svelte";
  import HistoryTab from "$lib/components/HistoryTab.svelte";
  import LinkedTasksTab from "$lib/components/LinkedTasksTab.svelte";

  let {
    noteId,
    focusedBlock,
    onOpenNote,
  }: {
    noteId: string | undefined;
    focusedBlock: ParsedBlock | null;
    onOpenNote: (noteId: string) => void;
  } = $props();

  type TabKey = "backlinks" | "properties" | "outline" | "history" | "tasks";
  const TABS: { key: TabKey; label: string }[] = [
    { key: "backlinks", label: "Backlinks" },
    { key: "properties", label: "Properties" },
    { key: "outline", label: "Outline" },
    { key: "history", label: "History" },
    { key: "tasks", label: "Tasks" },
  ];
  let activeTab = $state<TabKey>("backlinks");
</script>

<div class="v4-ctx">
  <div class="v4-ctx-tabs">
    {#each TABS as t (t.key)}
      <button
        type="button"
        class="v4-ctx-tab"
        class:active={activeTab === t.key}
        onclick={() => (activeTab = t.key)}
      >
        {t.label}
      </button>
    {/each}
  </div>

  <div class="v4-ctx-body">
    {#if !noteId}
      <p class="v4-ctx-empty">no note focused</p>
    {:else if activeTab === "backlinks"}
      <BacklinksTab {noteId} {onOpenNote} />
    {:else if activeTab === "properties"}
      <PropertiesView {noteId} {focusedBlock} />
    {:else if activeTab === "outline"}
      <OutlineTab {noteId} {onOpenNote} />
    {:else if activeTab === "history"}
      <HistoryTab {noteId} />
    {:else if activeTab === "tasks"}
      <LinkedTasksTab {noteId} />
    {/if}
  </div>
</div>

<style>
  .v4-ctx {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .v4-ctx-tabs {
    display: flex;
    gap: 2px;
    padding: 4px 8px 0;
    border-bottom: 1px solid var(--v4-hair);
    flex-shrink: 0;
  }
  .v4-ctx-tab {
    font-family: var(--v4-mono);
    font-size: 10.5px;
    letter-spacing: 0.3px;
    color: var(--v4-ink5);
    background: transparent;
    border: 0;
    border-bottom: 1px solid transparent;
    padding: 4px 8px 5px;
    cursor: pointer;
  }
  .v4-ctx-tab:hover {
    color: var(--v4-ink3);
  }
  .v4-ctx-tab.active {
    color: var(--v4-ink);
    border-bottom-color: var(--v4-accent);
  }
  .v4-ctx-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 8px;
  }

  /* Shared styles for the v4-native tabs (Backlinks / Outline /
     Properties). Global because those child components live in their
     own files but are only ever mounted inside a ContextPane. */
  :global(.v4-ctx-empty) {
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 11px;
    margin: 4px 6px;
  }
  :global(.v4-ctx-list) {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  :global(.v4-ctx-row) {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    border: 0;
    color: var(--v4-ink2);
    font-family: var(--v4-sans);
    font-size: 12px;
    padding: 3px 6px;
    border-radius: 4px;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  :global(.v4-ctx-row:hover) {
    background: var(--v4-surface);
    color: var(--v4-ink);
  }
  :global(.v4-ctx-outline-row) {
    color: var(--v4-ink3);
  }
  :global(.v4-ctx-bullet) {
    color: var(--v4-ink5);
    margin-right: 4px;
  }
</style>
