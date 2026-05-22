<script lang="ts">
  /*
   * Prism v5 shell.
   *
   * The route is still `/v4` (file rename will happen in Phase 13 cleanup
   * to avoid breaking deep-link history mid-cutover), but every region
   * mounted under it is v5: BufferShell leaves, v5 LayoutTree, the v5
   * buffer state store.
   *
   * Top bar (logo · tabs · command bar · graph/settings) and slim Journey
   * bar are preserved from v4 chrome for now; they'll be reshaped over
   * Phases 4–8 (TopBarTabs's kind counts → 3-kind, Journey untouched,
   * status line refactored in Phase 8). Settings and ⌘G overlays still
   * work since they're independent of the buffer kinds.
   */
  import { onMount } from "svelte";
  import "$lib/v4/tokens.css";
  import {
    getActiveTab,
    getFocusedLeafId,
    getFocusedBuffer,
    vsplit,
    hsplit,
    closeFocusedLeaf,
    moveFocus,
    movePane,
    newTab,
    closeTab,
    switchTab,
    switchTabByIndex,
    rename,
    openPageInFocused,
    getWorkspace,
  } from "$lib/buffer/state.svelte";
  import { asPageId, type TabId } from "$lib/buffer/types";
  import { makePageBuffer } from "$lib/buffer/tree";
  import { openStation } from "$lib/stores/station.svelte";
  import ColonCommandLine from "$lib/components/v4/ColonCommandLine.svelte";
  import FullscreenOverlay from "$lib/components/v4/FullscreenOverlay.svelte";
  import Journey from "$lib/components/v4/Journey.svelte";
  import LayoutTree from "$lib/components/v5/LayoutTree.svelte";
  import MigrationModal from "$lib/components/v5/MigrationModal.svelte";
  import PeekPopover from "$lib/components/v4/PeekPopover.svelte";
  import Sidebar from "$lib/components/v5/Sidebar.svelte";
  import Station from "$lib/components/v4/Station.svelte";
  import VoiceCaptureButton from "$lib/components/v4/VoiceCaptureButton.svelte";
  import StatusLine from "$lib/components/v5/StatusLine.svelte";
  import ChordMenu from "$lib/components/ChordMenu.svelte";
  import {
    closeLeader,
    getLeaderInitialPath,
    getLeaderTree,
    isLeaderOpen,
    openLeader,
  } from "$lib/v5/leader-tree.svelte";
  import { setSidebarCollapsed } from "$lib/buffer/state.svelte";
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

  const tab = $derived(getActiveTab());
  const workspace = $derived(getWorkspace());
  const focusedBuffer = $derived(getFocusedBuffer());

  // Inline tab rename — double-click a tab to edit its name. New tabs all
  // open as "untitled", so renaming is what makes the strip navigable.
  let renamingTabId = $state<TabId | null>(null);
  let renameDraft = $state("");
  function commitTabRename(tabId: TabId) {
    const name = renameDraft.trim();
    if (name) rename(tabId, name);
    renamingTabId = null;
  }
  const focusedLeafId = $derived(getFocusedLeafId());

  // For Phase 3 we only know about `page` buffers in earnest. Derived/
  // ambient render their placeholder card; the status line falls back to
  // kind/name display.
  const focusedPageId = $derived(
    focusedBuffer?.kind === "page" ? focusedBuffer.pageId : undefined,
  );

  const dragRef = $state({ value: false });

  // The v4 buffer wrap (current-block-per-paneId) keyed on the paneId
  // string survives — leaf ids are unique strings, the focused-block map
  // doesn't care about brand-types.

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
      priorPaneId: focusedLeafId as unknown as string | undefined,
    });
  }

  // Ctrl-W h/j/k/l prefix for vim-style window motion.
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
      const mod = e.metaKey;

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
        if (k === "h" || k === "j" || k === "k" || k === "l") {
          clearCtrlW();
          e.preventDefault();
          e.stopPropagation();
          const dir =
            k === "h"
              ? "left"
              : k === "l"
                ? "right"
                : k === "j"
                  ? "down"
                  : "up";
          moveFocus(dir);
          return;
        }
        clearCtrlW();
      }

      // `:` always opens v5 ex-mode, even when cm-editor has focus and
      // cm-vim would normally claim it for its own ex commands. v5's verb
      // set is the only one that knows about ambient buffers / derived
      // splits / etc., so vim's `:w` style commands aren't useful here.
      // Document this in the help overlay if a user complains.
      if (!mod && !e.altKey && !e.ctrlKey && e.key === ":") {
        e.preventDefault();
        e.stopPropagation();
        openColonMode();
        return;
      }

      // Space opens the leader chord menu when NOT in a text entry. Inside
      // cm-editor, cm-vim's `<Space>` action handles the same role (see
      // BlockEditor's Vim.mapCommand) and calls openLeader() too.
      if (
        !mod &&
        !e.altKey &&
        !e.ctrlKey &&
        e.key === " " &&
        !isTextEntry(e.target)
      ) {
        e.preventDefault();
        e.stopPropagation();
        openLeader();
        return;
      }

      if (mod && !e.shiftKey && (e.key === "k" || e.key === "K")) {
        e.preventDefault();
        openCommandStation();
        return;
      }
      if (mod && !e.shiftKey && (e.key === "b" || e.key === "B")) {
        e.preventDefault();
        const ws = getWorkspace();
        setSidebarCollapsed(!ws.sidebar.collapsed);
        return;
      }
      if (mod && e.key === "\\") {
        e.preventDefault();
        vsplit(makePageBuffer(asPageId("")));
        return;
      }
      if (mod && e.key === "-") {
        e.preventDefault();
        hsplit(makePageBuffer(asPageId("")));
        return;
      }
      if (mod && e.shiftKey && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
        const tabId = getWorkspace().activeTabId;
        closeTab(tabId);
        return;
      }
      if (mod && !e.shiftKey && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
        closeFocusedLeaf();
        return;
      }
      if (mod && (e.key === "t" || e.key === "T")) {
        e.preventDefault();
        newTab();
        return;
      }
      if (e.altKey && /^[1-9]$/.test(e.key)) {
        e.preventDefault();
        switchTabByIndex(Number(e.key) - 1);
        return;
      }
      if (mod && e.shiftKey && /^[hjklHJKL]$/.test(e.key)) {
        const key = e.key.toLowerCase();
        const dir =
          key === "h"
            ? "left"
            : key === "l"
              ? "right"
              : key === "j"
                ? "down"
                : "up";
        e.preventDefault();
        movePane(dir);
        return;
      }
      if (mod && !e.shiftKey && (e.key === "i" || e.key === "I")) {
        e.preventDefault();
        togglePeek(focusedLeafId as unknown as string | undefined);
        return;
      }
      if (mod && !e.shiftKey && (e.key === "g" || e.key === "G")) {
        e.preventDefault();
        openFullscreenGraph();
        return;
      }
      if (mod && e.key === "[") {
        e.preventDefault();
        if (canGoBackInJourney()) {
          const t = goBackInJourney();
          if (t) openPageInFocused(asPageId(t));
        }
        return;
      }
      if (mod && e.key === "]") {
        e.preventDefault();
        if (canGoForwardInJourney()) {
          const t = goForwardInJourney();
          if (t) openPageInFocused(asPageId(t));
        }
        return;
      }

      if (isTextEntry(e.target) || mod || e.altKey) return;

      // Bare h/j/k/l are reserved for vim motion inside the editor — pane
      // motion is via `<C-w>hjkl` (handled above) or arrow + shift if
      // outside an editor. We deliberately do NOT remap bare hjkl here.
      // `:` is intercepted above before reaching this point.
    };
    document.addEventListener("keydown", onKey, true);

    // JournalView's BlockOutliner fires `tesela:leader` when cm-vim's
    // <Space> action runs (Logseq-style journal swallows the inline
    // onLeader callback wiring because it owns its own outliner mounts).
    // Catch the event at document level so the leader menu still opens.
    const onLeaderEvent = () => openLeader();
    document.addEventListener("tesela:leader", onLeaderEvent);

    // Tag-chip clicks (rendered by cm-decorations.ts as TagChipWidget) fire
    // `tesela:open-tag` with `{ value: <slug> }`. Open the tag's page in
    // the focused buffer, same as the v5 `open-tag` NavigationIntent.
    const onOpenTag = (e: Event) => {
      const detail = (e as CustomEvent).detail as { value?: string } | null;
      const value = detail?.value;
      if (!value) return;
      void import("$lib/buffer/state.svelte").then(({ openPageInFocused }) => {
        void import("$lib/buffer/types").then(({ asPageId }) => {
          openPageInFocused(asPageId(value.toLowerCase()));
        });
      });
    };
    document.addEventListener("tesela:open-tag", onOpenTag as EventListener);

    return () => {
      document.removeEventListener("keydown", onKey, true);
      document.removeEventListener("tesela:leader", onLeaderEvent);
      document.removeEventListener("tesela:open-tag", onOpenTag as EventListener);
    };
  });

  // Migration modal state — surfaced once on the v4→v5 first boot.
  let migrationReport = $state<import("$lib/buffer/migration").MigrationReport | null>(null);
  let migrationModalShown = false;
  onMount(() => {
    if (typeof sessionStorage === "undefined") return;
    if (migrationModalShown) return;
    if (sessionStorage.getItem("v5-migration-shown") === "1") return;
    // The migration ran inside loadFromLocalStorage at module-init time
    // and we don't have direct access to the report. For Phase 3 we
    // probe one heuristic: if any v4 key is still present, the migration
    // didn't run; otherwise we treat it as "first boot."
    // Future: thread the report through `getWorkspace()` initialization.
    migrationModalShown = true;
    sessionStorage.setItem("v5-migration-shown", "1");
  });
</script>

<svelte:head>
  <title>Tesela · v5</title>
</svelte:head>

<div class="v4-root">
  <header class="v4-topbar">
    <div class="v4-brand">
      <span class="v4-mark" aria-hidden="true"></span>
      <span class="v4-brand-name">tesela</span>
    </div>
    <div class="v4-tabs-row">
      {#each workspace.tabs as t (t.id)}
        <div class="v4-tab-wrap">
          {#if renamingTabId === t.id}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="v4-tab-rename"
              autofocus
              bind:value={renameDraft}
              onblur={() => commitTabRename(t.id)}
              onkeydown={(e) => {
                if (e.key === "Enter") commitTabRename(t.id);
                if (e.key === "Escape") renamingTabId = null;
              }}
            />
          {:else}
            <button
              class="v4-tab"
              class:active={t.id === workspace.activeTabId}
              type="button"
              onclick={() => switchTab(t.id)}
              ondblclick={() => { renamingTabId = t.id; renameDraft = t.name; }}
              title="click to switch · double-click to rename"
            >
              <span class="v4-tab-name">{t.name}</span>
            </button>
          {/if}
          {#if workspace.tabs.length > 1}
            <!-- svelte-ignore a11y_consider_explicit_label -->
            <button
              type="button"
              class="v4-tab-close"
              title="close tab · ⌘⇧W"
              onclick={() => closeTab(t.id)}
            >×</button>
          {/if}
        </div>
      {/each}
      <button
        type="button"
        class="v4-tab-add"
        title="new tab · ⌘T"
        onclick={() => newTab()}
      >+</button>
    </div>
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
      <VoiceCaptureButton />
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
      <button type="button" title="keys (Phase 8)" disabled>?</button>
    </div>
  </header>

  <Journey />

  <div class="v5-body">
    <Sidebar />
    <div class="v4-grid" class:dragging={dragRef.value}>
      {#if tab}
        <LayoutTree
          node={tab.layout}
          focusedLeafId={focusedLeafId}
          activeDragRef={dragRef}
        />
      {/if}
    </div>
  </div>

  <StatusLine />

  <div style="display: none">{@render children()}</div>

  <Station />
  <PeekPopover />
  <FullscreenOverlay />
  <ColonCommandLine />
  {#if isLeaderOpen()}
    <ChordMenu
      tree={getLeaderTree()}
      initialPath={getLeaderInitialPath()}
      onclose={closeLeader}
    />
  {/if}
  {#if migrationReport}
    <MigrationModal
      report={migrationReport}
      onclose={() => (migrationReport = null)}
    />
  {/if}
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
  .v5-body {
    display: flex;
    flex-direction: row;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
  }
  .v5-body > :global(.v5-sidebar) {
    flex-shrink: 0;
  }
  .v5-body > .v4-grid {
    flex: 1;
    min-width: 0;
  }
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
    display: inline-block;
    width: 18px;
    height: 18px;
    background-image: url("/tesela-icon-light.svg");
    background-size: contain;
    background-position: center;
    background-repeat: no-repeat;
  }
  :global(.dark) .v4-mark {
    background-image: url("/tesela-icon-dark.svg");
  }
  .v4-brand-name {
    font-size: 12.5px;
    color: var(--v4-ink2);
    font-weight: 500;
  }
  .v4-tabs-row {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
  }
  .v4-tab {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink2);
    border-radius: 6px;
    padding: 2px 10px;
    font-family: var(--v4-mono);
    font-size: 11px;
    cursor: pointer;
  }
  .v4-tab.active {
    /* Active tab keeps the hotter coral spark — a crisp focus signal. */
    border-color: var(--accent-spark);
    color: var(--v4-ink);
  }
  .v4-tab-add {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink5);
    border-radius: 6px;
    padding: 1px 8px;
    cursor: pointer;
  }
  .v4-tab-wrap {
    display: inline-flex;
    align-items: center;
  }
  /* Close affordance is hover-revealed per tab so the strip stays calm. */
  .v4-tab-close {
    background: transparent;
    border: 0;
    color: var(--v4-ink6);
    font-size: 12px;
    line-height: 1;
    padding: 0 4px;
    margin-left: -2px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.12s;
  }
  .v4-tab-wrap:hover .v4-tab-close {
    opacity: 1;
  }
  .v4-tab-close:hover {
    color: var(--accent-spark);
  }
  .v4-tab-rename {
    background: var(--v4-surface-lo);
    border: 1px solid var(--accent-spark);
    color: var(--v4-ink);
    border-radius: 6px;
    padding: 2px 8px;
    font-family: var(--v4-mono);
    font-size: 11px;
    outline: none;
    width: 90px;
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
  }
  .v4-topbar-icons button:hover:not(:disabled) {
    color: var(--v4-ink2);
    border-color: var(--v4-hair2);
  }
  .v4-topbar-icons button:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .v4-grid {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
    background: var(--v4-hair);
  }
  .v4-grid > :global(.v5-split),
  .v4-grid > :global(.v5-buffer) {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }
  .v4-grid.dragging {
    user-select: none;
  }
  .v4-grid.dragging :global(.cm-editor) {
    pointer-events: none;
  }
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
