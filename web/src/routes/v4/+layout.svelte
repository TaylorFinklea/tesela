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
    setColSizes,
    setRowSizes,
    openInEditor,
  } from "$lib/stores/pane-tree.svelte";
  import { MIN_PANE_WEIGHT } from "$lib/stores/pane-tree";
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
  import {
    openFullscreenGraph,
    openSettingsOverlay,
  } from "$lib/stores/fullscreen-overlay.svelte";
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
      // App-level shortcuts use ⌘ only — Ctrl is reserved for vim. Most
      // notably this gives vim its `<C-w>` window-prefix back (was being
      // eaten by the close-pane binding). Cross-platform Ctrl support can
      // come later as a pref; today the app is Mac-only.
      const mod = e.metaKey;

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

      // ⌘I / ⌘G — peek + fullscreen graph. Fire from anywhere (including
      // inside cm-editor) so the user doesn't need to escape vim to reach
      // them. Bare `K` / `g` below also work outside the editor.
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

      // ⌘[ / ⌘] walk the Journey breadcrumb. Fires regardless of focus —
      // these are app-level navigation, parallel to browser back/forward.
      // Routes through `openInEditor` so back/forward lands in the main
      // editor, not whatever pane currently has focus.
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

  // ── pane resize ──────────────────────────────────────────────────────────
  //
  // Each row is a flex row whose children carry `flex: <weight>` from
  // `tab.colSizes[r]`. The grid container is a flex column of those rows,
  // each one styled `flex: <weight>` from `tab.rowSizes`. A drag handler
  // swaps two adjacent weights such that their sum is preserved, so only
  // the pair on either side of the gutter moves.
  let gridEl = $state<HTMLElement | undefined>();
  let activeDrag = $state(false);

  function beginColDrag(ev: PointerEvent, rowIdx: number, leftIdx: number) {
    const t = getFocusedTab();
    if (!t) return;
    const rowEl = (ev.currentTarget as HTMLElement)?.parentElement;
    if (!rowEl) return;
    const rowPx = rowEl.getBoundingClientRect().width;
    if (rowPx <= 0) return;
    const startX = ev.clientX;
    const sizesStart = t.colSizes[rowIdx].slice();
    const totalW = sizesStart.reduce((s, w) => s + w, 0);
    const sL = sizesStart[leftIdx];
    const sR = sizesStart[leftIdx + 1];
    const sum = sL + sR;
    ev.preventDefault();
    activeDrag = true;
    document.body.style.cursor = "col-resize";
    const onMove = (e: PointerEvent) => {
      const dPx = e.clientX - startX;
      // Convert px delta into weight delta in proportion to total row
      // width: weight-per-px = totalW / rowPx.
      const dW = (dPx / rowPx) * totalW;
      let newL = sL + dW;
      newL = Math.max(MIN_PANE_WEIGHT, Math.min(sum - MIN_PANE_WEIGHT, newL));
      const next = sizesStart.slice();
      next[leftIdx] = newL;
      next[leftIdx + 1] = sum - newL;
      setColSizes(rowIdx, next);
    };
    const onUp = () => {
      activeDrag = false;
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }

  function beginRowDrag(ev: PointerEvent, topIdx: number) {
    const t = getFocusedTab();
    if (!t || !gridEl) return;
    const gridPx = gridEl.getBoundingClientRect().height;
    if (gridPx <= 0) return;
    const startY = ev.clientY;
    const sizesStart = t.rowSizes.slice();
    const totalW = sizesStart.reduce((s, w) => s + w, 0);
    const sT = sizesStart[topIdx];
    const sB = sizesStart[topIdx + 1];
    const sum = sT + sB;
    ev.preventDefault();
    activeDrag = true;
    document.body.style.cursor = "row-resize";
    const onMove = (e: PointerEvent) => {
      const dPx = e.clientY - startY;
      const dW = (dPx / gridPx) * totalW;
      let newT = sT + dW;
      newT = Math.max(MIN_PANE_WEIGHT, Math.min(sum - MIN_PANE_WEIGHT, newT));
      const next = sizesStart.slice();
      next[topIdx] = newT;
      next[topIdx + 1] = sum - newT;
      setRowSizes(next);
    };
    const onUp = () => {
      activeDrag = false;
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
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
        onclick={() => openSettingsOverlay("general")}
      >⚙</button>
      <button type="button" title="keys (Phase 6 polish)" disabled>?</button>
    </div>
  </header>

  <Journey />

  <!-- Pane grid — flex column of flex rows. Each row + pane carries a
       `flex` weight from `tab.rowSizes` / `tab.colSizes`; drag handles
       between siblings rewrite those weights live. -->
  <div class="v4-grid" class:dragging={activeDrag} bind:this={gridEl}>
    {#each tab?.layout ?? [] as rowArr, r (r)}
      <div
        class="v4-grid-row"
        style="flex: {tab?.rowSizes[r] ?? 1} 1 0"
      >
        {#each rowArr as pane, c (pane.id)}
          <div class="v4-pane-cell" style="flex: {tab?.colSizes[r][c] ?? 1} 1 0">
            <PaneShell
              {pane}
              row={r}
              col={c}
              focused={!!tab && r === tab.focus[0] && c === tab.focus[1]}
            />
          </div>
          {#if c < rowArr.length - 1}
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="v4-resizer v4-resizer-col"
              role="separator"
              aria-orientation="vertical"
              title="drag to resize"
              onpointerdown={(e) => beginColDrag(e, r, c)}
            ></div>
          {/if}
        {/each}
      </div>
      {#if r < (tab?.layout.length ?? 0) - 1}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="v4-resizer v4-resizer-row"
          role="separator"
          aria-orientation="horizontal"
          title="drag to resize"
          onpointerdown={(e) => beginRowDrag(e, r)}
        ></div>
      {/if}
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
      <span><b>⌘I</b> peek</span>
      <span><b>⌘G</b> graph</span>
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

  /* Pane grid — flex column of flex rows with explicit weights. */
  .v4-grid {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--v4-hair);
  }
  .v4-grid.dragging {
    /* While a drag is active, every cm-editor inside the grid loses pointer
       capture so the cursor and selection don't flicker. */
    user-select: none;
  }
  .v4-grid.dragging * {
    pointer-events: none;
  }
  .v4-grid-row {
    display: flex;
    flex-direction: row;
    min-height: 0;
    min-width: 0;
  }
  .v4-pane-cell {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }

  /* Resizers — invisible 1px hairlines that thicken on hover/drag. */
  .v4-resizer {
    background: var(--v4-hair);
    position: relative;
    flex-shrink: 0;
    transition: background 140ms;
  }
  .v4-resizer::after {
    content: "";
    position: absolute;
    inset: 0;
  }
  .v4-resizer-col {
    width: 1px;
    cursor: col-resize;
  }
  .v4-resizer-col::after {
    /* expand the hit area to ±3px so the user doesn't need pixel-perfect aim */
    left: -3px;
    right: -3px;
  }
  .v4-resizer-row {
    height: 1px;
    cursor: row-resize;
  }
  .v4-resizer-row::after {
    top: -3px;
    bottom: -3px;
  }
  .v4-resizer:hover {
    background: var(--v4-accent-dim);
  }
  /* Keep the cursor + hit area working during an active drag (overrides the
     blanket `pointer-events: none` from `.v4-grid.dragging *`). */
  .v4-grid.dragging .v4-resizer { pointer-events: auto; }

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
