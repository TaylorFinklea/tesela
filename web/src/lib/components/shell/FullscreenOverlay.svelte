<script lang="ts">
  /*
   * Prism v4 — fullscreen overlay shell.
   *
   * Two overlay kinds today: graph (`g` opens it) and settings (`⚙` /
   * `:settings-<slug>` open it). The shell is generic so future
   * overlays (zen-mode editor, presentation view) can slot in via
   * the `OverlayKind` union without rebuilding the keymap or backdrop.
   *
   * Graph uses the same `GraphCanvas` that the in-pane `graph` kind
   * mounts; here we just give it the full viewport. Settings mounts
   * `<SettingsOverlay>` which composes the existing settings page
   * components inside the v4 shell.
   */
  import { onMount } from "svelte";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import type { GraphEdge } from "$lib/types/GraphEdge";
  import GraphCanvas from "$lib/components/GraphCanvas.svelte";
  import SettingsOverlay from "$lib/components/shell/SettingsOverlay.svelte";
  import ReleaseNotesOverlay from "$lib/components/shell/ReleaseNotesOverlay.svelte";
  // Uses app role tokens from app.css; Graphite bridges those roles in
  // graphite/tokens.css so the fixed overlay inherits the active chrome.
  import {
    closeOverlay,
    getActiveOverlay,
    getKeymapText,
    isOverlayOpen,
  } from "$lib/stores/fullscreen-overlay.svelte";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  const openInEditor = (id: string, _opts?: { via?: string }) =>
    openPageInFocused(asPageId(id));

  const open = $derived(isOverlayOpen());
  const kind = $derived(getActiveOverlay());

  // Notes + edges for the graph kind. Same query keys as PaneShell so
  // they share cache + WS invalidation.
  // Raised 500→5000 (tesela-sclr.1): a 500 cap silently hid notes past #500
  // from the graph.
  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
    enabled: open && kind === "graph",
  }));
  const edgesQuery = createQuery(() => ({
    queryKey: ["all-edges"] as const,
    queryFn: () => api.getAllEdges(),
    enabled: open && kind === "graph",
  }));
  const notes = $derived((notesQuery.data ?? []) as Note[]);
  const edges = $derived((edgesQuery.data ?? []) as GraphEdge[]);
  const keymapText = $derived(getKeymapText());

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
          openInEditor(noteId, { via: "graph" });
          closeOverlay();
        }}
      />
    </div>
  </div>
{:else if open && kind === "settings"}
  <div class="overlay">
    <SettingsOverlay />
  </div>
{:else if open && kind === "keymap"}
  <div class="overlay">
    <header class="overlay-head">
      <span class="overlay-label">keymap</span>
      <span class="overlay-hint">esc closes</span>
      <button class="overlay-close" type="button" onclick={closeOverlay} title="close · Esc">×</button>
    </header>
    <div class="overlay-body">
      <pre class="keymap-pre">{keymapText}</pre>
    </div>
  </div>
{:else if open && kind === "release-notes"}
  <div class="overlay">
    <ReleaseNotesOverlay />
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 95;
    background: var(--bg);
    display: flex;
    flex-direction: column;
    animation: app-fade-in var(--motion-duration-base) var(--motion-ease-overlay);
  }
  .overlay-head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line-soft);
    flex-shrink: 0;
  }
  .overlay-label {
    font-family: var(--theme-font-mono);
    font-size: 10px;
    letter-spacing: 1.4px;
    text-transform: uppercase;
    color: var(--accent-spark);
  }
  .overlay-hint {
    flex: 1;
    font-family: var(--theme-font-mono);
    font-size: 10.5px;
    color: var(--fg-faint);
  }
  .overlay-close {
    background: transparent;
    border: 0;
    color: var(--fg-subtle);
    font-size: 16px;
    line-height: 1;
    padding: 2px 8px;
    border-radius: 5px;
    cursor: pointer;
  }
  .overlay-close:hover { color: var(--fg-muted); background: var(--bg-2); }
  .overlay-body {
    flex: 1;
    min-height: 0;
    position: relative;
  }
  .keymap-pre {
    height: 100%;
    margin: 0;
    padding: 16px 20px;
    overflow: auto;
    font-family: var(--theme-font-mono);
    font-size: 12.5px;
    line-height: 1.5;
    color: var(--fg-default);
    white-space: pre;
    background: var(--bg);
  }
</style>
