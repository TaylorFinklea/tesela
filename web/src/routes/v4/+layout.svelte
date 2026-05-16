<script lang="ts">
  /*
   * Prism v4 shell. Top bar (logo · tabs · command bar · graph/settings
   * icons), slim Journey bar, the recursive pane tree, and a status
   * line. The QueryClientProvider + WebSocket connection are inherited
   * from the root layout (Phase 6 collapsed that into a thin shell).
   *
   * Keybindings here mount on `document` for the layout's lifetime so
   * they only apply while the user is in v4. App-level shortcuts are
   * ⌘-only — every Ctrl-prefixed combination passes through to cm-vim.
   */
  import { onMount } from "svelte";
  import "$lib/v4/tokens.css";
  import {
    getState,
    getFocusedTab,
    getFocusedPane,
    getFocusedPaneId,
    vsplit,
    hsplit,
    closePane,
    moveFocus,
    movePane,
    stackNext,
    stackAdd,
    jumpToTile,
    newTab,
    closeTab,
    switchTabByIndex,
    openInEditor,
  } from "$lib/stores/pane-tree.svelte";
  import { openStation } from "$lib/stores/station.svelte";
  import ColonCommandLine from "$lib/components/v4/ColonCommandLine.svelte";
  import FullscreenOverlay from "$lib/components/v4/FullscreenOverlay.svelte";
  import Journey from "$lib/components/v4/Journey.svelte";
  import LayoutTree from "$lib/components/v4/LayoutTree.svelte";
  import PeekPopover from "$lib/components/v4/PeekPopover.svelte";
  import Station from "$lib/components/v4/Station.svelte";
  import TopBarTabs from "$lib/components/v4/TopBarTabs.svelte";
  import {
    canGoBackInJourney,
    canGoForwardInJourney,
    goBackInJourney,
    goForwardInJourney,
  } from "$lib/stores/journey.svelte";
  import { openColonMode } from "$lib/stores/colon-mode.svelte";
  import {
    openFullscreenGraph,
    openSettingsOverlay,
  } from "$lib/stores/fullscreen-overlay.svelte";
  import { togglePeek } from "$lib/stores/peek.svelte";

  let { children } = $props();

  const tab = $derived(getFocusedTab());
  const focusedPane = $derived(getFocusedPane());
  const focusedId = $derived(getFocusedPaneId());
  const activeTileId = $derived(
    focusedPane?.kind === "editor"
      ? focusedPane.tiles[focusedPane.activeIdx]
      : undefined,
  );

  // Reactive flag shared with `<LayoutTree>` — flipped true while any
  // resizer drag is in progress, used by the layout to disable
  // pointer-events on cm-editors so they don't fight the drag cursor.
  const dragRef = $state({ value: false });

  function isTextEntry(target: EventTarget | null): boolean {
    const el = target as HTMLElement | null;
    if (!el) return false;
    return (
      el.tagName === "INPUT" ||
      el.tagName === "TEXTAREA" ||
      el.isContentEditable ||
      !!el.closest?.(".cm-editor")
    );
  }

  function openCommandStation(opts?: { query?: string }) {
    openStation({
      tab: "palette",
      query: opts?.query,
      priorPaneId: getFocusedPaneId(),
    });
  }

  // Ctrl-W prefix state machine. Vim users expect `<C-w>h/j/k/l` to move
  // between panes. Without intercepting it here, cm-vim swallows the
  // chord and tries its own window-motion submode, which knows nothing
  // about our pane tree. Tradeoff: insert-mode `<C-w>` (delete previous
  // word) is no longer available inside the editor. Listen on the
  // *capture* phase below so this fires before cm-vim's keymap.
  let awaitingCtrlW = false;
  let ctrlWTimer: ReturnType<typeof setTimeout> | null = null;
  function clearCtrlW() {
    awaitingCtrlW = false;
    if (ctrlWTimer) {
      clearTimeout(ctrlWTimer);
      ctrlWTimer = null;
    }
  }

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      // App-level shortcuts use ⌘ only — Ctrl is reserved for vim.
      const mod = e.metaKey;

      // Ctrl-W h/j/k/l — pane focus motions, vim window-motion convention.
      if (
        !e.metaKey &&
        !e.altKey &&
        !e.shiftKey &&
        e.ctrlKey &&
        (e.key === "w" || e.key === "W")
      ) {
        e.preventDefault();
        e.stopPropagation();
        awaitingCtrlW = true;
        if (ctrlWTimer) clearTimeout(ctrlWTimer);
        ctrlWTimer = setTimeout(clearCtrlW, 1500);
        return;
      }
      if (awaitingCtrlW) {
        const k = e.key.toLowerCase();
        // Only consume the follow-up if it's an h/j/k/l motion. Any
        // other key cancels the prefix and falls through normally.
        if (k === "h" || k === "j" || k === "k" || k === "l") {
          clearCtrlW();
          e.preventDefault();
          e.stopPropagation();
          const dir =
            k === "h" ? "left" : k === "l" ? "right" : k === "j" ? "down" : "up";
          moveFocus(dir as "left" | "right" | "up" | "down");
          return;
        }
        clearCtrlW();
      }

      // ⌘K opens the Station. Station's own keydown handler intercepts
      // ⌘K while open (toggle-close).
      if (mod && !e.shiftKey && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        openCommandStation();
        return;
      }

      // Splits + tab/pane management — fire regardless of focus.
      if (mod && e.key === "\\") {
        e.preventDefault();
        vsplit("editor");
        return;
      }
      if (mod && e.key === "-") {
        e.preventDefault();
        hsplit("editor");
        return;
      }
      if (mod && e.shiftKey && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
        closeTab(getState().activeTabId);
        return;
      }
      if (mod && !e.shiftKey && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
        closePane();
        return;
      }
      if (mod && (e.key === "t" || e.key === "T")) {
        e.preventDefault();
        newTab();
        return;
      }
      // ⌥1–9 jumps to the Nth tab.
      if (e.altKey && /^[1-9]$/.test(e.key)) {
        e.preventDefault();
        switchTabByIndex(Number(e.key) - 1);
        return;
      }

      // ⌘⇧H/J/K/L move the focused pane in that direction — Aerospace /
      // tmux-style "send this pane to the left edge", etc.
      if (mod && e.shiftKey && /^[hjklHJKL]$/.test(e.key)) {
        const key = e.key.toLowerCase();
        const dir =
          key === "h" ? "left" : key === "l" ? "right" : key === "j" ? "down" : "up";
        e.preventDefault();
        movePane(dir as "left" | "right" | "up" | "down");
        return;
      }

      // ⌘I / ⌘G — peek + fullscreen graph. Fire from anywhere (including
      // inside cm-editor) so the user doesn't need to escape vim.
      if (mod && !e.shiftKey && (e.key === "i" || e.key === "I")) {
        e.preventDefault();
        togglePeek(getFocusedPaneId());
        return;
      }
      if (mod && !e.shiftKey && (e.key === "g" || e.key === "G")) {
        e.preventDefault();
        openFullscreenGraph();
        return;
      }

      // ⌘[ / ⌘] walk the Journey breadcrumb.
      if (mod && e.key === "[") {
        e.preventDefault();
        if (canGoBackInJourney()) {
          const t = goBackInJourney();
          if (t) openInEditor(t, { via: "back" });
        }
        return;
      }
      if (mod && e.key === "]") {
        e.preventDefault();
        if (canGoForwardInJourney()) {
          const t = goForwardInJourney();
          if (t) openInEditor(t, { via: "forward" });
        }
        return;
      }

      // The rest only fire when a cm-editor / input doesn't own focus.
      if (isTextEntry(e.target) || mod || e.altKey) return;

      switch (e.key) {
        case "h":
        case "ArrowLeft":
          e.preventDefault();
          moveFocus("left");
          break;
        case "l":
        case "ArrowRight":
          e.preventDefault();
          moveFocus("right");
          break;
        case "j":
        case "ArrowDown":
          e.preventDefault();
          moveFocus("down");
          break;
        case "k":
        case "ArrowUp":
          e.preventDefault();
          moveFocus("up");
          break;
        case "[":
          e.preventDefault();
          stackNext(-1);
          break;
        case "]":
          e.preventDefault();
          stackNext(1);
          break;
        case "S":
          e.preventDefault();
          promptStackAdd();
          break;
        case "o":
          e.preventDefault();
          promptJump();
          break;
        case ":":
          e.preventDefault();
          openColonMode();
          break;
        case "K":
          // `K` (no modifier) is a fallback for peek when outside the
          // editor — ⌘I works everywhere else.
          e.preventDefault();
          togglePeek(getFocusedPaneId());
          break;
        case "g":
          // Same fallback for the graph overlay.
          e.preventDefault();
          openFullscreenGraph();
          break;
        default:
          break;
      }
    };

    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  });

  // Phase 1 scaffolding — the Command Station replaces these.
  function promptJump() {
    const id = window.prompt("jump to tile (note slug)")?.trim();
    if (id) jumpToTile(id);
  }
  function promptStackAdd() {
    const id = window.prompt("stack tile (note slug)")?.trim();
    if (id) stackAdd(id);
  }
</script>

<svelte:head>
  <title>Tesela · v4</title>
</svelte:head>

<div class="v4-root">
  <!-- Top bar -->
  <header class="v4-topbar">
    <div class="v4-brand">
      <span class="v4-mark" aria-hidden="true"></span>
      <span class="v4-brand-name">tesela</span>
    </div>
    <TopBarTabs />
    <button
      class="v4-command-bar"
      type="button"
      onclick={() => openCommandStation()}
      title="open the Command Station · ⌘K"
    >
      <span class="v4-command-bar-kbd">⌘K</span>
      <span class="v4-command-bar-hint">Command Station — verbs, dashboard…</span>
    </button>
    <div class="v4-topbar-icons">
      <button
        type="button"
        title="fullscreen graph · ⌘G"
        onclick={() => openFullscreenGraph()}
      >✦</button>
      <button
        type="button"
        title="settings — devices, sync, mosaic…"
        onclick={() => openSettingsOverlay("general")}
      >⚙</button>
      <button type="button" title="keys (Phase 6 polish)" disabled>?</button>
    </div>
  </header>

  <Journey />

  <!-- Pane tree — recursive binary split. Drag handles live inside each
       <LayoutTree> node, scoped to the matching split. -->
  <div class="v4-grid" class:dragging={dragRef.value}>
    {#if tab}
      <LayoutTree node={tab.layout} focusedPaneId={focusedId} activeDragRef={dragRef} />
    {/if}
  </div>

  <!-- Status line -->
  <footer class="v4-statusline">
    <span class="v4-status-mode">● NORMAL</span>
    <span class="v4-status-center">
      <span>tab: {tab?.name ?? "—"}</span>
      <span class="v4-status-sep">·</span>
      <span>{focusedPane?.kind ?? "—"}</span>
      {#if activeTileId}
        <span class="v4-status-sep">·</span>
        <span>{activeTileId}</span>
      {/if}
    </span>
    <span class="v4-status-right">
      <span><b>⌘K</b> station</span>
      <span><b>:</b> ex</span>
      <span><b>⌘I</b> peek</span>
      <span><b>⌘G</b> graph</span>
      <span><b>hjkl</b> move</span>
      <span><b>⌘⇧hjkl</b> push</span>
    </span>
  </footer>

  <!-- The page component is the URL→state adapter; renders nothing
       visible but its onMount bootstraps the focused pane. -->
  <div style="display: none">{@render children()}</div>

  <Station />
  <PeekPopover />
  <FullscreenOverlay />
  <ColonCommandLine />
</div>

<style>
  .v4-root {
    position: fixed;
    inset: 0;
    display: grid;
    grid-template-rows: 40px 30px 1fr 26px;
    background: var(--v4-bg);
    color: var(--v4-ink);
    font-family: var(--v4-sans);
    font-size: 13px;
    overflow: hidden;
  }

  /* Top bar */
  .v4-topbar {
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) 380px auto;
    align-items: center;
    gap: 14px;
    padding: 0 14px;
    border-bottom: 1px solid var(--v4-hair);
  }
  .v4-brand { display: flex; align-items: center; gap: 8px; }
  .v4-mark {
    display: inline-block;
    width: 18px;
    height: 18px;
    background-image: url('/tesela-icon-light.svg');
    background-size: contain;
    background-position: center;
    background-repeat: no-repeat;
  }
  :global(.dark) .v4-mark { background-image: url('/tesela-icon-dark.svg'); }
  .v4-brand-name { font-size: 12.5px; color: var(--v4-ink2); font-weight: 500; }
  .v4-command-bar-hint {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink6);
  }
  .v4-command-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 10px;
    background: var(--v4-surface-lo);
    border: 1px solid var(--v4-hair);
    border-radius: 7px;
    color: var(--v4-ink5);
    cursor: pointer;
    transition: border-color 140ms, color 140ms;
  }
  .v4-command-bar:hover { border-color: var(--v4-hair2); color: var(--v4-ink2); }
  .v4-command-bar-kbd { font-family: var(--v4-mono); font-size: 10.5px; color: var(--v4-ink4); }
  .v4-topbar-icons { display: flex; align-items: center; gap: 4px; }
  .v4-topbar-icons button {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink5);
    border-radius: 6px;
    padding: 3px 9px;
    font-family: var(--v4-mono);
    font-size: 12px;
    cursor: pointer;
    transition: color 140ms, border-color 140ms;
  }
  .v4-topbar-icons button:hover:not(:disabled) {
    color: var(--v4-ink2);
    border-color: var(--v4-hair2);
  }
  .v4-topbar-icons button:disabled { cursor: default; opacity: 0.6; }

  /* Pane tree host — column-flex container so the <LayoutTree> root
     stretches to fill the 1fr grid cell. */
  .v4-grid {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--v4-hair);
  }
  .v4-grid > :global(.v4-split),
  .v4-grid > :global(.v4-pane) {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }
  .v4-grid.dragging { user-select: none; }
  .v4-grid.dragging :global(.cm-editor) { pointer-events: none; }

  /* Status line */
  .v4-statusline {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 14px;
    padding: 0 14px;
    border-top: 1px solid var(--v4-hair);
    font-family: var(--v4-mono);
    font-size: 10.5px;
    color: var(--v4-ink4);
  }
  .v4-status-mode { color: var(--v4-accent); flex-shrink: 0; }
  .v4-status-center {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    white-space: nowrap;
  }
  .v4-status-sep { color: var(--v4-ink6); }
  .v4-status-right {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }
  .v4-status-right b { color: var(--v4-accent); font-weight: 400; }
</style>
