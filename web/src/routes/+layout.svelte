<script lang="ts">
  import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
  import { onMount } from "svelte";
  import { connect, setHandlers } from "$lib/ws-client.svelte";
  import { goto } from "$app/navigation";
  import { pushNavigation, goBack, goForward } from "$lib/stores/navigation.svelte";
  import {
    isVimEnabled,
    isCtrlWPending,
    setCtrlWPending,
    setActivePane,
    setActiveRegion,
    getActiveRegion,
    isSplitOpen,
    getActivePane,
    toggleSplit,
    closeSplit,
    adjustSplitRatio,
    setSplitRatio,
    isBottomDrawerOpen,
    toggleBottomDrawer,
    setBottomDrawerOpen,
    getVSplitActiveSide,
    setVSplitActiveSide,
    adjustVSplitRatio,
    setVSplitRatio,
    getVimMode,
  } from "$lib/stores/pane-state.svelte";
  import { goBack as goBackColumn } from "$lib/stores/active-pane-nav.svelte";
  import { page } from "$app/state";
  import CrumbBar from "$lib/components/CrumbBar.svelte";
  import Rail from "$lib/components/Rail.svelte";
  import BottomDrawer from "$lib/components/BottomDrawer.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import LeaderMenu from "$lib/components/LeaderMenu.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import { ensureSystemWidgets } from "$lib/system-widgets";
  import { api } from "$lib/api-client";
  import "../app.css";

  let { children } = $props();

  const queryClient = new QueryClient({
    defaultOptions: {
      queries: {
        refetchOnWindowFocus: false,
        staleTime: 30_000,
      },
    },
  });

  let showLeaderMenu = $state(false);
  const drawerOpen = $derived(isBottomDrawerOpen());

  // Phase 9.5c — drilling is opt-in: only block drill-in, wiki-link click,
  // and query-result row click call `gotoNote()` (which writes `?back=`).
  // Rail clicks and ⌘K palette picks are plain SvelteKit navigations that
  // replace the focus area full-screen. No global drill interceptor.

  onMount(() => {
    connect();

    setHandlers({
      onNoteCreated: () => { queryClient.invalidateQueries({ queryKey: ["notes"] }); },
      onNoteUpdated: (note) => {
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        queryClient.invalidateQueries({ queryKey: ["note", note.id] });
        queryClient.invalidateQueries({ queryKey: ["typed-blocks"] });
      },
      onNoteDeleted: (id) => {
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        queryClient.invalidateQueries({ queryKey: ["note", id] });
      },
    });

    // Ensure the 9 system Query widgets exist so the rail is populated on
    // first run. Idempotent — only creates on 404. Fires-and-forgets; rail
    // will reactively pick them up via the notes WS invalidation when each
    // is created.
    void ensureSystemWidgets(api);

    const spaceHandler = (e: KeyboardEvent) => {
      if (e.key === " " && !showLeaderMenu) {
        const target = e.target as HTMLElement;
        const isEditing =
          target.tagName === "INPUT" ||
          target.tagName === "TEXTAREA" ||
          target.isContentEditable ||
          target.closest(".cm-editor");
        if (!isEditing) {
          e.preventDefault();
          showLeaderMenu = true;
        }
      }
    };
    const leaderHandler = () => {
      showLeaderMenu = true;
    };
    // Global shortcuts (outside editors): 1, b, [, ], /
    const panelHandler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const isEditing =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable ||
        target.closest(".cm-editor") ||
        // Phase 10.1 follow-up — QueryWidgetView (rendered for /p/tasks etc.)
        // owns its own keyboard scope: `j/k`, `/` (slash menu), `e` (rename),
        // `s` (cycle status). Treat the QWV root the same as an editor so
        // these chords don't bubble into the global panel/palette handlers.
        target.closest(".qwv");
      if (isEditing) return;

      if (e.key === "1" || e.key === "b") {
        e.preventDefault();
        toggleBottomDrawer();
        return;
      }
      if (e.key === "[") {
        e.preventDefault();
        const prev = goBack();
        if (prev) goto(prev);
      }
      if (e.key === "]") {
        e.preventDefault();
        const next = goForward();
        if (next) goto(next);
      }
      if (e.key === "/" && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        document.dispatchEvent(new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true }));
      }
    };

    // Phase 9.4 — Cmd+Z bleed-through fix. When vim is enabled, the user's
    // canonical undo path is `u` in Normal mode, which walks the unified
    // outliner+insert-session stack (Phase 3M.1). cm6's per-keystroke history
    // also lives underneath; if Cmd+Z fires inside cm-editor it reaches cm6's
    // history extension and walks character-level undo, which has been
    // observed to interact badly with outliner-undo state (memory:
    // project_post_redesign_followups.md). Suppress it at capture phase when
    // vim is on. When vim is off we leave Cmd+Z untouched so the platform
    // shortcut works inside the editor.
    const cmdZHandler = (e: KeyboardEvent) => {
      if (!isVimEnabled()) return;
      const isUndo = (e.metaKey || e.ctrlKey) && !e.altKey && (e.key === "z" || e.key === "Z");
      if (!isUndo) return;
      const target = e.target as HTMLElement | null;
      if (!target?.closest?.(".cm-editor")) return;
      // Phase 9.7 — fully suppress cm6's character-level undo and route to
      // the unified outliner+insert-session stack via a custom event so
      // Cmd+Z behaves like the vim `u` (and Cmd+Shift+Z like `Ctrl+R`).
      e.preventDefault();
      e.stopPropagation();
      e.stopImmediatePropagation();
      const ev = e.shiftKey ? "tesela:outliner-redo" : "tesela:outliner-undo";
      document.dispatchEvent(new CustomEvent(ev));
    };

    // Phase 9.5b: column-view split is open whenever URL has ?back=.
    const isColumnSplitOpen = () => !!page.url.searchParams.get("back");

    // Phase 9.5b: Esc when right pane is active + vim NORMAL mode collapses
    // the split (full-screens the left). Vim swallows Esc in INSERT/VISUAL,
    // so we never see those at this layer; the explicit NORMAL check is
    // belt-and-suspenders.
    const escHandler = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      if (!isColumnSplitOpen()) return;
      if (getActiveRegion() !== "focus") return;
      if (getVSplitActiveSide() !== "right") return;
      if (getVimMode() !== "NORMAL") return;
      e.preventDefault();
      e.stopPropagation();
      goBackColumn();
    };

    // Ctrl+w chord handler — Vim-style window commands across the four
    // regions: rail / middle / focus / bottom. Capture phase to beat the
    // browser's "close tab" on Ctrl+w.
    let pendingTimer: ReturnType<typeof setTimeout> | null = null;
    const clearPending = () => {
      setCtrlWPending(false);
      if (pendingTimer) { clearTimeout(pendingTimer); pendingTimer = null; }
    };

    const ctrlWHandler = (e: KeyboardEvent) => {
      if (showLeaderMenu) return;
      if (!isVimEnabled()) return;

      if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
        e.stopPropagation();
        setCtrlWPending(true);
        if (pendingTimer) clearTimeout(pendingTimer);
        pendingTimer = setTimeout(clearPending, 2000);
        return;
      }

      if (isCtrlWPending()) {
        if (e.key === "Shift" || e.key === "Control" || e.key === "Alt" || e.key === "Meta") {
          return;
        }
        e.preventDefault();
        e.stopPropagation();
        switch (e.key) {
          case "h": {
            const r = getActiveRegion();
            // Phase 9.9 — when column-split is shown:
            //   right active + ^w h → flip to left pane (move toward "what I came from")
            //   left active  + ^w h → collapse split (full-screen the left)
            // The two-step contract: first ^w h moves your focus across the
            // split; second ^w h drops you out of split mode entirely.
            if (r === "focus" && isColumnSplitOpen()) {
              if (getVSplitActiveSide() === "right") setVSplitActiveSide("left");
              else goBackColumn();
            } else if (r === "focus") setActiveRegion("rail");
            else if (r === "bottom") setActiveRegion("focus");
            break;
          }
          case "l": {
            const r = getActiveRegion();
            // Phase 9.5b: focus + column-split shown + left active → flip to right.
            if (r === "focus" && isColumnSplitOpen() && getVSplitActiveSide() === "left") {
              setVSplitActiveSide("right");
            } else if (r === "rail") setActiveRegion("focus");
            break;
          }
          case "j": {
            const r = getActiveRegion();
            // Phase 9.9 — `^w j` opens the bottom drawer if closed, then
            // focuses it. Previously, when the drawer was closed, ^w j fell
            // through to the kanban-split branch (or did nothing at all),
            // which contradicted the user's "drop to drawer" mental model.
            // The kanban path now requires the drawer to already be closed
            // AND a kanban split to be open AND no column-split.
            if (r === "focus") {
              if (isSplitOpen() && getActivePane() !== "kanban") {
                setActivePane("kanban");
              } else {
                if (!isBottomDrawerOpen()) setBottomDrawerOpen(true);
                setActiveRegion("bottom");
              }
            }
            break;
          }
          case "k": {
            const r = getActiveRegion();
            if (r === "bottom") {
              setActiveRegion("focus");
            } else if (r === "focus" && isSplitOpen() && getActivePane() === "kanban") {
              setActivePane("outliner");
            }
            break;
          }
          case "s": toggleSplit(); break;
          case "q": {
            // Phase 9.5b: column-split shown → goBack (full-screen left).
            // Otherwise close kanban.
            if (isColumnSplitOpen()) goBackColumn();
            else closeSplit();
            break;
          }
          case "=": {
            if (isColumnSplitOpen()) setVSplitRatio(50);
            else setSplitRatio(50);
            break;
          }
          case "+": {
            if (isColumnSplitOpen()) adjustVSplitRatio(-10);
            else adjustSplitRatio(-10);
            break;
          }
          case "-": {
            if (isColumnSplitOpen()) adjustVSplitRatio(10);
            else adjustSplitRatio(10);
            break;
          }
        }
        clearPending();
      }
    };

    // Phase 9.7 — focus the target pane's first cm-editor after gotoNote
    // emits this event. Without this, the cm-editor that was focused before
    // the drill keeps DOM focus, so vim chords go to the wrong pane.
    const focusPaneHandler = (e: Event) => {
      const side = ((e as CustomEvent).detail?.side as "left" | "right") ?? "right";
      const active = document.activeElement;
      if (
        active instanceof HTMLElement &&
        active.classList.contains("cm-content") &&
        !active.closest(`[data-pane="${side}"]`)
      ) {
        active.blur();
      }
      const target = document.querySelector(
        `[data-pane="${side}"] .cm-editor .cm-content`,
      );
      if (target instanceof HTMLElement) target.focus();
    };

    document.addEventListener("keydown", spaceHandler);
    document.addEventListener("keydown", panelHandler);
    document.addEventListener("keydown", ctrlWHandler, true);
    document.addEventListener("keydown", cmdZHandler, true);
    document.addEventListener("keydown", escHandler);
    document.addEventListener("tesela:leader", leaderHandler);
    document.addEventListener("tesela:focus-pane", focusPaneHandler);
    return () => {
      document.removeEventListener("keydown", spaceHandler);
      document.removeEventListener("keydown", panelHandler);
      document.removeEventListener("keydown", ctrlWHandler, true);
      document.removeEventListener("keydown", cmdZHandler, true);
      document.removeEventListener("keydown", escHandler);
      document.removeEventListener("tesela:leader", leaderHandler);
      document.removeEventListener("tesela:focus-pane", focusPaneHandler);
      if (pendingTimer) clearTimeout(pendingTimer);
    };
  });
</script>

<svelte:head>
  <title>Tesela</title>
</svelte:head>

<QueryClientProvider client={queryClient}>
  <div class="v9 dark {drawerOpen ? 'with-bottom' : ''}">
    <CrumbBar />
    <Rail />
    <main class="v9-focus">
      {@render children()}
    </main>
    {#if drawerOpen}
      <BottomDrawer />
    {/if}
    <StatusBar />
  </div>
  <CommandPalette />
  {#if showLeaderMenu}
    <LeaderMenu onclose={() => (showLeaderMenu = false)} />
  {/if}
</QueryClientProvider>
