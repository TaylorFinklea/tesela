<script lang="ts">
  /*
   * Prism v4 — fullscreen overlay shell.
   *
   * Phase 5 ships exactly one overlay: the graph (`g` opens it). The
   * shell is generic so future overlays (zen-mode editor, presentation
   * view) can slot in via the `OverlayKind` union without rebuilding
   * the keymap or backdrop.
   *
   * Graph uses the same `GraphCanvas` that the in-pane `graph` kind
   * mounts; here we just give it the full viewport.
   */
  import { onMount } from "svelte";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { GraphEdge } from "$lib/types/GraphEdge";
  import GraphCanvas from "$lib/components/GraphCanvas.svelte";
  import {
    closeOverlay,
    getActiveOverlay,
    isOverlayOpen,
  } from "$lib/stores/fullscreen-overlay.svelte";
  import { jumpToTile } from "$lib/stores/pane-tree.svelte";

  const open = $derived(isOverlayOpen());
  const kind = $derived(getActiveOverlay());

  // Notes + edges for the graph kind. Same query keys as PaneShell so
  // they share cache + WS invalidation.
  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: open && kind === "graph",
  }));
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: open && kind === "graph",
  }));
  const notes = $derived((notesQuery.data ?? []) as Note[]);
  const edges = $derived((edgesQuery.data ?? []) as GraphEdge[]);

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      closeOverlay();
    }
  }

  onMount(() => {
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  });
</script>

{#if open && kind === "graph"}
  <div class="overlay">
    <header class="overlay-head">
      <span class="overlay-label">graph</span>
      <span class="overlay-hint">esc closes · click a node to open it</span>
      <button class="overlay-close" type="button" onclick={closeOverlay} title="close · Esc">×</button>
    </header>
    <div class="overlay-body">
      <GraphCanvas
        {notes}
        {edges}
        onNodePick={(noteId) => {
          jumpToTile(noteId, "graph");
          closeOverlay();
        }}
      />
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 95;
    background: var(--v4-bg);
    display: flex;
    flex-direction: column;
  }
  .overlay-head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--v4-hair);
    flex-shrink: 0;
  }
  .overlay-label {
    font-family: var(--v4-mono);
    font-size: 10px;
    letter-spacing: 1.4px;
    text-transform: uppercase;
    color: var(--v4-accent);
  }
  .overlay-hint {
    flex: 1;
    font-family: var(--v4-mono);
    font-size: 10.5px;
    color: var(--v4-ink5);
  }
  .overlay-close {
    background: transparent;
    border: 0;
    color: var(--v4-ink4);
    font-size: 16px;
    line-height: 1;
    padding: 2px 8px;
    border-radius: 5px;
    cursor: pointer;
  }
  .overlay-close:hover { color: var(--v4-ink2); background: var(--v4-surface-lo); }
  .overlay-body {
    flex: 1;
    min-height: 0;
    position: relative;
  }
</style>
