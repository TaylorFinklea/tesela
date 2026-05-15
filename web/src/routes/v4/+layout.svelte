<script lang="ts">
  /*
   * Prism v4 shell. Phase 1: top bar (logo + tab/command placeholders +
   * graph/help icons), slim Journey bar, the 2D pane grid, and a status
   * line. The QueryClientProvider + WebSocket connection are inherited
   * from the legacy root layout (this route nests under it); the root
   * layout renders `/v4` "bare" — no rail, drawer, or crumb bar — and
   * suppresses its own global keydown handlers while the path is /v4.
   *
   * Keybindings here are mounted on `document` for the lifetime of this
   * layout only (i.e. only while the user is on /v4), so there's no
   * cross-talk with the legacy chrome.
   */
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
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
    stackNext,
    stackAdd,
    jumpToTile,
    newTab,
    closeTab,
    switchTabByIndex,
  } from "$lib/stores/pane-tree.svelte";
  import { openStation } from "$lib/stores/station.svelte";
  import ColonCommandLine from "$lib/components/v4/ColonCommandLine.svelte";
  import FullscreenOverlay from "$lib/components/v4/FullscreenOverlay.svelte";
  import Journey from "$lib/components/v4/Journey.svelte";
  import PaneShell from "$lib/components/v4/PaneShell.svelte";
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
  import { openFullscreenGraph } from "$lib/stores/fullscreen-overlay.svelte";
  import { togglePeek } from "$lib/stores/peek.svelte";

  let { children } = $props();

  const tab = $derived(getFocusedTab());
  const focusedPane = $derived(getFocusedPane());
  const activeTileId = $derived(
    focusedPane?.kind === "editor"
      ? focusedPane.tiles[focusedPane.activeIdx]
      : undefined,
  );

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

  onMount(() => {
    const onKey = (e: KeyboardEvent) => {
      const mod = e.metaKey || e.ctrlKey;

      // ⌘K opens the Station. Station's own keydown handler intercepts
      // ⌘K while open (toggle-close), so this only ever fires the open.
      if (mod && !e.shiftKey && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        openCommandStation();
        return;
      }

      // Splits + tab/pane management — fire regardless of focus (capture).
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

      // ⌘[ / ⌘] walk the Journey breadcrumb. Fires regardless of focus —
      // these are app-level navigation, parallel to browser back/forward.
      if (mod && e.key === "[") {
        e.preventDefault();
        if (canGoBackInJourney()) {
          const t = goBackInJourney();
          if (t) jumpToTile(t, "back");
        }
        return;
      }
      if (mod && e.key === "]") {
        e.preventDefault();
        if (canGoForwardInJourney()) {
          const t = goForwardInJourney();
          if (t) jumpToTile(t, "forward");
        }
        return;
      }

      // The rest only fire when a cm-editor / input doesn't own focus.
      if (isTextEntry(e.target) || mod || e.altKey) return;

      switch (e.key) {
        case "h":
        case "ArrowLeft":
          e.preventDefault();
          moveFocus(0, -1);
          break;
        case "l":
        case "ArrowRight":
          e.preventDefault();
          moveFocus(0, 1);
          break;
        case "j":
        case "ArrowDown":
          e.preventDefault();
          moveFocus(1, 0);
          break;
        case "k":
        case "ArrowUp":
          e.preventDefault();
          moveFocus(-1, 0);
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
        // Phase 5 surfaces.
        case ":":
          e.preventDefault();
          openColonMode();
          break;
        case "K":
          // Peek — `K` mirrors vim's "look up keyword" semantically.
          // Lowercase `k` is move-focus-up above; uppercase is unbound in
          // our pane keymap, so this doesn't collide with vim insert (`i`).
          e.preventDefault();
          togglePeek(getFocusedPaneId());
          break;
        case "g":
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

  // Phase 1 scaffolding — crude prompts to populate panes. The Command
  // Station (Phase 4) replaces both with real fuzzy search.
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
      <span class="v4-mark" aria-hidden="true">◧</span>
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
        title="fullscreen graph · g"
        onclick={() => openFullscreenGraph()}
      >✦</button>
      <button
        type="button"
        title="settings — devices, sync, mosaic…"
        onclick={() => goto("/settings/general")}
      >⚙</button>
      <button type="button" title="keys (Phase 6 polish)" disabled>?</button>
    </div>
  </header>

  <Journey />

  <!-- Pane grid -->
  <div
    class="v4-grid"
    style="grid-template-rows: {(tab?.layout ?? []).map(() => '1fr').join(' ')}"
  >
    {#each tab?.layout ?? [] as rowArr, r (r)}
      <div
        class="v4-grid-row"
        style="grid-template-columns: {rowArr.map(() => '1fr').join(' ')}"
      >
        {#each rowArr as pane, c (pane.id)}
          <PaneShell
            {pane}
            row={r}
            col={c}
            focused={!!tab && r === tab.focus[0] && c === tab.focus[1]}
          />
        {/each}
      </div>
    {/each}
  </div>

  <!-- Status line -->
  <footer class="v4-statusline">
    <span class="v4-status-mode">● NORMAL</span>
    <span class="v4-status-center">
      <span>tab: {tab?.name ?? "—"}</span>
      <span class="v4-status-sep">·</span>
      <span>
        row {(tab?.focus[0] ?? 0) + 1}, col {(tab?.focus[1] ?? 0) + 1}
      </span>
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
      <span><b>K</b> peek</span>
      <span><b>g</b> graph</span>
      <span><b>hjkl</b> move</span>
    </span>
  </footer>

  <!-- The page component is the URL→state adapter; it renders nothing
       visible in Phase 1 but its onMount bootstraps the focused pane. -->
  <div style="display: none">{@render children()}</div>

  <!-- Phase 4 — Command Station modal. Owns its own keydown handler
       while open (Esc, ⌘1–⌘4, ⌘K toggle-close). -->
  <Station />

  <!-- Phase 5 surfaces. Each owns its own Esc handler while open. -->
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
  .v4-brand {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .v4-mark {
    color: var(--v4-accent);
    font-size: 14px;
  }
  .v4-brand-name {
    font-size: 12.5px;
    color: var(--v4-ink2);
    font-weight: 500;
  }
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
  .v4-command-bar:hover {
    border-color: var(--v4-hair2);
    color: var(--v4-ink2);
  }
  .v4-command-bar-kbd {
    font-family: var(--v4-mono);
    font-size: 10.5px;
    color: var(--v4-ink4);
  }
  .v4-topbar-icons {
    display: flex;
    align-items: center;
    gap: 4px;
  }
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
  .v4-topbar-icons button:disabled {
    cursor: default;
    opacity: 0.6;
  }

  /* Journey bar styles live in Journey.svelte. */

  /* Pane grid */
  .v4-grid {
    display: grid;
    gap: 1px;
    background: var(--v4-hair);
    min-height: 0;
    min-width: 0;
  }
  .v4-grid-row {
    display: grid;
    gap: 1px;
    background: var(--v4-hair);
    min-height: 0;
    min-width: 0;
  }

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
  .v4-status-mode {
    color: var(--v4-accent);
    flex-shrink: 0;
  }
  .v4-status-center {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    white-space: nowrap;
  }
  .v4-status-sep {
    color: var(--v4-ink6);
  }
  .v4-status-right {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }
  .v4-status-right b {
    color: var(--v4-accent);
    font-weight: 400;
  }
</style>
