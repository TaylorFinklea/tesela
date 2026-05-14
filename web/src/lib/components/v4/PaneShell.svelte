<script lang="ts">
  /*
   * Prism v4 — one cell of the pane grid. Phase 2a implements `editor`
   * and `widget` kinds; `context` / `graph` / `dashboard` still render a
   * labelled placeholder until 2b/2c.
   *
   * Each editor pane owns its own debounced save timer, so concurrent
   * edits across panes never collide on the PUT. The save path mirrors
   * `routes/p/[id]/+page.svelte`; Phase 2d extracts a shared
   * `<NoteRenderer>` and this duplication goes away.
   */
  import { onDestroy } from "svelte";
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { Pane } from "$lib/stores/pane-tree";
  import {
    focusPane,
    stackNext,
    swapKind,
    setPaneWidget,
    jumpToTile,
  } from "$lib/stores/pane-tree.svelte";
  import {
    setFocusedBlockForPane,
    clearPaneFocusedBlock,
  } from "$lib/stores/current-block.svelte";
  import { widgetFromNote, parseWidgets } from "$lib/widget-registry.svelte";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import QueryWidgetView from "$lib/components/QueryWidgetView.svelte";
  import PaneKindMenu from "$lib/components/v4/PaneKindMenu.svelte";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";

  let {
    pane,
    row,
    col,
    focused,
  }: {
    pane: Pane;
    row: number;
    col: number;
    focused: boolean;
  } = $props();

  let shellEl = $state<HTMLElement | undefined>();

  // Active tile id for editor panes (undefined when the stack is empty).
  const activeTileId = $derived(
    pane.kind === "editor" ? pane.tiles[pane.activeIdx] : undefined,
  );

  const queryClient = useQueryClient();

  const noteQuery = createQuery(() => ({
    queryKey: ["note", activeTileId] as const,
    queryFn: () => api.getNote(activeTileId as string),
    enabled: !!activeTileId,
  }));

  // ── widget kind ───────────────────────────────────────────────────────────
  // A widget pane points at a Query-type note by id; `widgetFromNote`
  // turns that note into the config `QueryWidgetView` consumes.
  const widgetNoteId = $derived(
    pane.kind === "widget" ? pane.widget : undefined,
  );
  const widgetNoteQuery = createQuery(() => ({
    queryKey: ["note", widgetNoteId] as const,
    queryFn: () => api.getNote(widgetNoteId as string),
    enabled: !!widgetNoteId,
  }));
  const widgetConfig = $derived.by(() => {
    const n = widgetNoteQuery.data as Note | undefined;
    if (!n || n.metadata.note_type !== "Query") return null;
    return widgetFromNote(n);
  });

  // All Query notes — the widget-picker dropdown's options.
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: pane.kind === "widget",
  }));
  const widgetChoices = $derived(
    parseWidgets((allNotesQuery.data ?? []) as Note[]),
  );

  const note = $derived(noteQuery.data as Note | undefined);
  const split = $derived(splitContent(note?.content ?? ""));

  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---")) return { frontmatter: "", body: content };
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return { frontmatter: "", body: content };
    const fmEnd = endIdx + 3;
    const afterFm = content.slice(fmEnd);
    const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
    return { frontmatter: content.slice(0, fmEnd) + "\n", body: afterFm.slice(bodyStart) };
  }

  // ── per-pane debounced save ───────────────────────────────────────────────
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let inFlight: AbortController | null = null;
  let pending: string | null = null;

  function handleContentChange(fullContent: string) {
    pending = fullContent;
    if (saveTimer) clearTimeout(saveTimer);
    setSaving();
    saveTimer = setTimeout(() => void flushSave(), 500);
  }

  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (pending === null || !activeTileId) return;
    const content = pending;
    pending = null;
    if (inFlight) inFlight.abort();
    const controller = new AbortController();
    inFlight = controller;
    const id = activeTileId;
    if (note) queryClient.setQueryData(["note", id], { ...note, content });
    try {
      const updated = await api.updateNote(id, content, controller.signal);
      if (controller.signal.aborted) return;
      queryClient.setQueryData(["note", id], updated);
      setSaved();
    } catch (e) {
      if ((e as { name?: string })?.name === "AbortError") return;
      setSaveError(e instanceof Error ? e.message : "Unknown error");
      console.error("v4 pane save failed:", e);
    } finally {
      if (inFlight === controller) inFlight = null;
    }
  }

  // Cancel-and-flush for undo/redo races (BlockOutliner calls this).
  async function cancelAndFlush(fullContent: string) {
    pending = fullContent;
    await flushSave();
  }

  // Drop this pane's focused-block entry when the pane unmounts (e.g.
  // closePane) so a stale block can't linger in the shared map.
  onDestroy(() => clearPaneFocusedBlock(pane.id));

  // Clicking anywhere in the pane that isn't the editor focuses the
  // shell itself, so pane-nav keys (hjkl) land here rather than in a
  // cm-editor. Clicking into the editor lets cm own focus.
  function onShellClick(e: MouseEvent) {
    focusPane(row, col);
    const target = e.target as HTMLElement;
    if (!target.closest(".cm-editor")) {
      shellEl?.focus();
    }
  }

  const KIND_LABEL: Record<Pane["kind"], string> = {
    editor: "editor",
    widget: "widget",
    context: "context",
    graph: "graph",
    dashboard: "dashboard",
  };
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<section
  bind:this={shellEl}
  class="v4-pane"
  class:focused
  data-pane-id={pane.id}
  tabindex={0}
  onclick={onShellClick}
>
  <header class="v4-pane-header">
    <div class="v4-pane-header-left">
      <span class="v4-focus-dot">{focused ? "●" : "○"}</span>
      {#if pane.kind === "editor"}
        {#if pane.tiles.length > 1}
          <div class="v4-stack-bar">
            {#each pane.tiles as tileId, i (tileId)}
              <button
                class="v4-stack-chip"
                class:active={i === pane.activeIdx}
                onclick={(e) => {
                  e.stopPropagation();
                  focusPane(row, col);
                  // step the stack to this index
                  const delta = i - pane.activeIdx;
                  if (delta !== 0) stackNext(delta > 0 ? 1 : -1);
                }}
                title={tileId}
              >
                {tileId}
              </button>
            {/each}
          </div>
        {:else}
          <span class="v4-pane-title">{activeTileId ?? "empty"}</span>
        {/if}
      {:else if pane.kind === "widget"}
        <select
          class="v4-widget-picker"
          value={pane.widget}
          onclick={(e) => e.stopPropagation()}
          onchange={(e) => setPaneWidget(pane.id, e.currentTarget.value)}
          title="pick widget"
        >
          {#if widgetChoices.length === 0}
            <option value={pane.widget}>{pane.widget}</option>
          {:else}
            {#each widgetChoices as w (w.id)}
              <option value={w.id}>{w.title}</option>
            {/each}
          {/if}
        </select>
      {:else}
        <span class="v4-pane-title">{KIND_LABEL[pane.kind]}</span>
      {/if}
    </div>
    <div class="v4-pane-header-right">
      <PaneKindMenu {pane} onSwapKind={(k) => swapKind(pane.id, k)} />
    </div>
  </header>

  <div class="v4-pane-body">
    {#if pane.kind === "editor"}
      {#if !activeTileId}
        <div class="v4-pane-empty">
          <p>empty pane</p>
          <p class="v4-pane-empty-hint">jump to a tile to fill it</p>
        </div>
      {:else if noteQuery.isLoading}
        <div class="v4-pane-empty"><p>loading…</p></div>
      {:else if noteQuery.isError}
        <div class="v4-pane-empty"><p>could not load {activeTileId}</p></div>
      {:else if note}
        {#key activeTileId}
          <div class="v4-pane-scroll">
            <BlockOutliner
              noteId={note.id}
              body={split.body}
              frontmatter={split.frontmatter}
              paneId={pane.id}
              onContentChange={handleContentChange}
              onCancelAndFlush={cancelAndFlush}
              onfocusedblockchange={(b) => setFocusedBlockForPane(pane.id, b)}
            />
          </div>
        {/key}
      {/if}
    {:else if pane.kind === "widget"}
      {#if widgetNoteQuery.isLoading}
        <div class="v4-pane-empty"><p>loading…</p></div>
      {:else if widgetConfig}
        {#key pane.widget}
          <div class="v4-pane-scroll">
            <QueryWidgetView
              widget={widgetConfig}
              onOpenRow={(pageId) => {
                focusPane(row, col);
                jumpToTile(pageId);
              }}
            />
          </div>
        {/key}
      {:else if widgetNoteQuery.isError}
        <div class="v4-pane-empty">
          <p>could not load widget "{pane.widget}"</p>
        </div>
      {:else}
        <div class="v4-pane-empty">
          <p>"{pane.widget}" is not a Query note</p>
        </div>
      {/if}
    {:else}
      <div class="v4-pane-empty">
        <p>{KIND_LABEL[pane.kind]} pane</p>
        <p class="v4-pane-empty-hint">wired up in Phase 2b–2c</p>
      </div>
    {/if}
  </div>
</section>

<style>
  .v4-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--v4-bg);
    border-top: 2px solid transparent;
    transition: border-color 200ms, background 200ms;
    outline: none;
  }
  .v4-pane.focused {
    border-top-color: var(--v4-accent);
    background: linear-gradient(
      180deg,
      rgba(123, 140, 255, 0.04),
      transparent 30%
    );
  }

  .v4-pane-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 4px 10px;
    border-bottom: 1px solid var(--v4-hair);
    min-height: 30px;
    flex-shrink: 0;
  }
  .v4-pane-header-left {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .v4-pane-header-right {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .v4-focus-dot {
    font-family: var(--v4-mono);
    font-size: 9px;
    color: var(--v4-ink5);
    flex-shrink: 0;
  }
  .v4-pane.focused .v4-focus-dot {
    color: var(--v4-accent);
  }
  .v4-pane-title {
    font-family: var(--v4-sans);
    font-size: 12px;
    color: var(--v4-ink2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v4-widget-picker {
    font-family: var(--v4-sans);
    font-size: 12px;
    color: var(--v4-ink2);
    background: transparent;
    border: 1px solid var(--v4-hair);
    border-radius: 5px;
    padding: 1px 6px;
    max-width: 220px;
    cursor: pointer;
  }
  .v4-widget-picker:hover {
    border-color: var(--v4-hair2);
  }

  .v4-stack-bar {
    display: flex;
    align-items: center;
    gap: 3px;
    overflow: hidden;
  }
  .v4-stack-chip {
    font-family: var(--v4-mono);
    font-size: 10.5px;
    color: var(--v4-ink4);
    background: transparent;
    border: 1px solid var(--v4-hair);
    border-radius: 4px;
    padding: 1px 6px;
    cursor: pointer;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v4-stack-chip.active {
    color: var(--v4-ink);
    border-color: var(--v4-accent-dim);
    background: rgba(123, 140, 255, 0.08);
  }

  .v4-pane-body {
    flex: 1;
    min-height: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
  .v4-pane-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 14px 18px;
  }

  .v4-pane-empty {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 12px;
  }
  .v4-pane-empty-hint {
    color: var(--v4-ink6);
    font-size: 10.5px;
  }
</style>
