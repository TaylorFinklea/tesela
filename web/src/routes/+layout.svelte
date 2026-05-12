<script lang="ts">
  import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
  import { onMount } from "svelte";
  import { connect, setHandlers } from "$lib/ws-client.svelte";
  import {
    handleDeadlineApproaching,
    handleScheduledFires,
    handleRecurringRolled,
  } from "$lib/notifications";
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
    setDrawerRouteSuppressed,
    getVSplitActiveSide,
    setVSplitActiveSide,
    adjustVSplitRatio,
    setVSplitRatio,
    getVimMode,
    isRailOpen,
    toggleRail,
    getDrawerSide,
    getDrawerWidth,
    getDrawerHeight,
    toggleDrawerSide,
    pinBlock,
    pinPage,
    setBottomTab,
    cycleBottomDrawerTab,
  } from "$lib/stores/pane-state.svelte";
  import { getFocusedBlock } from "$lib/stores/current-block.svelte";
  import { goBack as goBackColumn } from "$lib/stores/active-pane-nav.svelte";
  import { page } from "$app/state";
  import CrumbBar from "$lib/components/CrumbBar.svelte";
  import Rail from "$lib/components/Rail.svelte";
  import BottomDrawer from "$lib/components/BottomDrawer.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import ChordMenu, { type ChordNode } from "$lib/components/ChordMenu.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import { ensureSystemWidgets } from "$lib/system-widgets";
  import { api } from "$lib/api-client";
  import { toast, getToast, clearToast } from "$lib/stores/toast.svelte";
  import { IconChevronRight } from "@tabler/icons-svelte";
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
  let leaderInitialPath = $state<string[]>([]);
  const drawerOpen = $derived(isBottomDrawerOpen());
  const railOpen = $derived(isRailOpen());
  const drawerSide = $derived(getDrawerSide());
  const drawerWidth = $derived(getDrawerWidth());
  const drawerHeight = $derived(getDrawerHeight());
  const activeToast = $derived(getToast());

  // Phase 10.2 — unified spacemacs-style leader chord tree. Block actions
  // dispatch `tesela:block-action` events that the focused BlockOutliner
  // listens for; page actions dispatch `tesela:page-action`. The trigger
  // path is opaque to the menu — `Space` from NORMAL mode and `Ctrl+,`
  // from any mode (works inside cm-editor INSERT) both open the same tree.
  const triggerCmdK = () =>
    document.dispatchEvent(new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true }));
  const emitBlock = (kind: string) =>
    document.dispatchEvent(new CustomEvent("tesela:block-action", { detail: { kind } }));
  const emitPage = (kind: string) =>
    document.dispatchEvent(new CustomEvent("tesela:page-action", { detail: { kind } }));
  const openDaily = async () => {
    try {
      const note = await api.getDailyNote();
      goto(`/p/${encodeURIComponent(note.id)}`);
    } catch (e) { console.error("Failed to open daily:", e); }
  };

  const leaderTree: ChordNode[] = [
    { key: "f", label: "File", children: [
      { key: "n", label: "New note",          action: triggerCmdK,                       hint: "⌘K" },
      { key: "d", label: "Daily",             action: openDaily,                         hint: "/p/<today>" },
      { key: "f", label: "Toggle favorite",   action: () => emitPage("favorite") },
      { key: "D", label: "Delete current",    action: () => emitPage("delete") },
    ]},
    { key: "b", label: "Block", children: [
      { key: "d", label: "Drill in",                  action: () => emitBlock("drillIn"),     hint: "⏎" },
      { key: "f", label: "Fold/unfold",               action: () => emitBlock("foldToggle"),  hint: "za" },
      { key: "s", label: "Cycle status (+ Task tag)", action: () => emitBlock("statusCycle"), hint: "⌘⏎" },
      { key: "D", label: "Delete block",              action: () => emitBlock("delete"),      hint: "dd" },
      { key: "y", label: "Yank block",                action: () => emitBlock("yank"),        hint: "yy" },
    ]},
    { key: "p", label: "Page", children: [
      { key: "f", label: "Toggle favorite",   action: () => emitPage("favorite") },
      { key: "m", label: "Toggle doc mode",   action: () => emitPage("docMode") },
      { key: "D", label: "Delete page",       action: () => emitPage("delete") },
    ]},
    { key: "s", label: "Search", children: [
      { key: "s", label: "Search palette",    action: triggerCmdK,                       hint: "⌘K" },
    ]},
    { key: "g", label: "Go to", children: [
      { key: "h", label: "Home",              action: () => goto("/"),                                              hint: "/" },
      { key: "d", label: "Daily",             action: openDaily,                                                    hint: "/p/<today>" },
      { key: "t", label: "Tasks",             action: () => goto("/p/tasks"),                                       hint: "/p/tasks" },
      { key: "i", label: "Inbox",             action: () => goto("/p/inbox"),                                       hint: "/p/inbox" },
      { key: "c", label: "Calendar",          action: () => goto("/p/calendar"),                                    hint: "/p/calendar" },
      { key: "p", label: "Pages",             action: () => goto("/p/pages"),                                       hint: "/p/pages" },
      { key: "f", label: "Follow wiki link",  action: () => emitBlock("followWiki"),                                hint: "[[ at ▌" },
    ]},
    { key: "w", label: "Window", children: [
      { key: "h", label: "Left pane",         action: () => { setVSplitActiveSide("left"); setActiveRegion("focus"); }, hint: "⌃w h" },
      { key: "l", label: "Right pane",        action: () => { setVSplitActiveSide("right"); setActiveRegion("focus"); }, hint: "⌃w l" },
      { key: "j", label: "Drawer",            action: () => { setBottomDrawerOpen(true); setActiveRegion("bottom"); },   hint: "⌃w j" },
      { key: "k", label: "Focus",             action: () => setActiveRegion("focus"),                                    hint: "⌃w k" },
      { key: "r", label: "Toggle rail",       action: toggleRail,                                                        hint: "r" },
      { key: "P", label: "Panel position",    action: toggleDrawerSide,                                                  hint: "p" },
      { key: "q", label: "Close split",       action: () => goBackColumn(),                                              hint: "⌃w q" },
    ]},
    { key: "T", label: "Toggle drawer",       action: toggleBottomDrawer, hint: "b" },
    { key: "P", label: "Pin", children: [
      { key: "b", label: "Pin focused block", action: () => pinFocusedBlock(), hint: "b" },
      { key: "p", label: "Pin current page",  action: () => pinCurrentPage(),  hint: "p" },
    ]},
    { key: "y", label: "Yank to clipboard",
      action: () => document.dispatchEvent(new CustomEvent("tesela:yank-clipboard")), hint: "leader Y" },
  ];

  function pinFocusedBlock() {
    const block = getFocusedBlock();
    if (!block) {
      toast("No block focused", "warn");
      return;
    }
    const preview = (block.raw_text ?? "").trim().slice(0, 40) || "(empty)";
    const id = pinBlock(block.note_id, block.id, preview);
    setBottomDrawerOpen(true);
    setBottomTab({ kind: "pinned", id });
  }

  function pinCurrentPage() {
    const url = new URL(window.location.href);
    const path = url.pathname;
    if (!path.startsWith("/p/")) {
      toast("No page to pin", "warn");
      return;
    }
    const noteId = decodeURIComponent(path.slice(3));
    const id = pinPage(noteId, noteId);
    setBottomDrawerOpen(true);
    setBottomTab({ kind: "pinned", id });
  }

  // Phase 9.5c — drilling is opt-in: only block drill-in, wiki-link click,
  // and query-result row click call `gotoNote()` (which writes `?back=`).
  // Rail clicks and ⌘K palette picks are plain SvelteKit navigations that
  // replace the focus area full-screen. No global drill interceptor.

  // Auto-collapse the bottom drawer on routes where it has nothing to do
  // (Settings is the obvious one — it has no per-note context to surface
  // in Backlinks/Properties/etc). Suppression is ephemeral: when the user
  // navigates away, their persisted drawer preference is restored. An
  // explicit `b` (toggleBottomDrawer) overrides the route suppression.
  const ROUTE_NO_DRAWER = /^\/settings(\/|$)/;
  $effect(() => {
    setDrawerRouteSuppressed(ROUTE_NO_DRAWER.test(page.url.pathname));
  });

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
      onDeadlineApproaching: handleDeadlineApproaching,
      onScheduledFires: handleScheduledFires,
      onRecurringRolled: handleRecurringRolled,
    });

    // Ensure the 9 system Query widgets exist so the rail is populated on
    // first run. Idempotent — only creates on 404. Fires-and-forgets; rail
    // will reactively pick them up via the notes WS invalidation when each
    // is created.
    void ensureSystemWidgets(api);

    const spaceHandler = (e: KeyboardEvent) => {
      if (e.key !== " " || showLeaderMenu) return;
      // Region gate: drawer / rail / middle own Space when active. Without
      // this, Space on the drawer wrapper (a tabindex=0 div) opens the
      // leader menu instead of letting the drawer's own keydown handler
      // run its cycle/toggle/edit action.
      if (getActiveRegion() !== "focus") return;
      const target = e.target as HTMLElement;
      const isEditing =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.tagName === "SELECT" ||
        target.isContentEditable ||
        target.closest(".cm-editor");
      if (!isEditing) {
        e.preventDefault();
        showLeaderMenu = true;
      }
    };

    // Phase 10.2 — INSERT-mode-friendly alt-trigger for the leader menu.
    // `Ctrl+,` (comma) opens the chord tree from anywhere, including inside
    // a cm-editor in INSERT mode where `Space` would just type a space.
    // Capture phase + stopImmediatePropagation so cm6 / cm-vim never see it.
    const altLeaderHandler = (e: KeyboardEvent) => {
      if (showLeaderMenu) return;
      if (!e.ctrlKey || e.key !== ",") return;
      if (e.metaKey || e.altKey) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      leaderInitialPath = [];
      showLeaderMenu = true;
    };

    // Phase 10.2 follow-up — `g` in vim NORMAL opens the leader menu pre-
    // descended into "Go to". BlockEditor's cm6 keymap dispatches this
    // event after checking vim mode. Also reachable programmatically by
    // any caller that wants to open at a specific sub-tree.
    const openLeaderAtHandler = (e: Event) => {
      if (showLeaderMenu) return;
      const detail = (e as CustomEvent).detail as { path?: string[] };
      leaderInitialPath = detail?.path ?? [];
      showLeaderMenu = true;
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
      if (e.key === "r") {
        e.preventDefault();
        toggleRail();
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

    // Focus the active pinned-tab editor inside the drawer (if any).
    // When no cm-editor is present (e.g. Backlinks tab), fall back to focusing
    // the drawer root itself (tabindex=0) so keyboard events land in the drawer
    // region and Tab won't escape to browser chrome.
    // Scheduled in a rAF so the region state settles before we query the DOM.
    const focusDrawerEditor = () => {
      requestAnimationFrame(() => {
        const cm = document.querySelector<HTMLElement>(".v9-bottom .cm-editor .cm-content");
        if (cm) { cm.focus(); return; }
        const root = document.querySelector<HTMLElement>(".v9-bottom");
        root?.focus();
      });
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
            else if (r === "bottom") {
              // When right-docked, ^w h means "go left" → back to focus.
              // When bottom-docked, ^w h also returns to focus (same logic).
              setActiveRegion("focus");
            }
            break;
          }
          case "l": {
            const r = getActiveRegion();
            // Phase 9.5b: focus + column-split shown + left active → flip to right.
            if (r === "focus" && isColumnSplitOpen() && getVSplitActiveSide() === "left") {
              setVSplitActiveSide("right");
            } else if (r === "focus" && isBottomDrawerOpen() && getDrawerSide() === "right") {
              // Right-docked drawer: ^w l from focus enters the drawer.
              setActiveRegion("bottom");
              focusDrawerEditor();
            } else if (r === "rail") setActiveRegion("focus");
            break;
          }
          case "j": {
            const r = getActiveRegion();
            // Phase 9.9 — `^w j` opens the bottom-docked drawer if closed, then
            // focuses it. When the drawer is right-docked, `j` is not a spatial
            // move toward it, so we skip drawer logic in that case.
            // The kanban path requires drawer to already be closed AND a kanban
            // split open AND no column-split.
            if (r === "focus") {
              const drawerIsRight = isBottomDrawerOpen() && getDrawerSide() === "right";
              if (isSplitOpen() && getActivePane() !== "kanban") {
                setActivePane("kanban");
              } else if (!drawerIsRight) {
                if (!isBottomDrawerOpen()) setBottomDrawerOpen(true);
                setActiveRegion("bottom");
                focusDrawerEditor();
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

    // Cmd+Ctrl+h/l — cycle drawer tabs regardless of vim mode.
    // This combination is not reserved by any browser or OS, so it reliably
    // fires even when focus is inside a cm-editor.  Ctrl+Tab was the previous
    // binding but Chrome reserves it for browser-tab switching at the OS level
    // and preventDefault() cannot reliably intercept it.
    const drawerTabHandler = (e: KeyboardEvent) => {
      if (!e.metaKey || !e.ctrlKey) return;
      if (e.key !== "h" && e.key !== "l") return;
      if (getActiveRegion() !== "bottom") return;
      if (!isBottomDrawerOpen()) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      cycleBottomDrawerTab(e.key === "l" ? 1 : -1);
      focusDrawerEditor();
    };

    // gt / gT chord — cycle drawer tabs when vim is enabled.
    // Only active when the drawer is focused AND the key event is NOT
    // originating from inside a cm-editor (inside editors, BlockEditor's
    // own vim action registration handles it).
    let pendingG: ReturnType<typeof setTimeout> | null = null;
    const clearPendingG = () => {
      if (pendingG) { clearTimeout(pendingG); pendingG = null; }
    };
    const gtHandler = (e: KeyboardEvent) => {
      if (!isVimEnabled()) return;
      if (getActiveRegion() !== "bottom") { clearPendingG(); return; }
      if (!isBottomDrawerOpen()) { clearPendingG(); return; }
      const target = e.target as HTMLElement;
      // Allow the chord from inside the drawer's own cm-editor (pinned-tab
      // editor) — the BlockEditor's `g` keymap returns false for isPinnedTab
      // so the event bubbles here. Only skip editors that are NOT inside the
      // drawer (focus-area editors, which have their own `g` → leader action).
      const inDrawerEditor = target.closest(".v9-bottom .cm-editor");
      if (target.closest(".cm-editor") && !inDrawerEditor) { clearPendingG(); return; }

      if (pendingG) {
        // Second key of chord
        if (e.key === "t") {
          e.preventDefault();
          clearPendingG();
          cycleBottomDrawerTab(1);
          focusDrawerEditor();
          return;
        } else if (e.key === "T") {
          e.preventDefault();
          clearPendingG();
          cycleBottomDrawerTab(-1);
          focusDrawerEditor();
          return;
        } else {
          // Not a drawer-tab chord — cancel and let the key through
          clearPendingG();
        }
      }

      if (e.key === "g" && !e.ctrlKey && !e.metaKey && !e.altKey) {
        // First key of chord: arm the pending state
        clearPendingG();
        pendingG = setTimeout(clearPendingG, 800);
        // Prevent this lone 'g' from doing anything else on the drawer root
        e.preventDefault();
      }
    };

    // Belt-and-suspenders Tab guard: when the active region is "bottom" and
    // focus is stranded outside the drawer (can happen after a tab-cycle unmount),
    // intercept Tab at capture phase and redirect focus into the drawer instead
    // of letting it walk the document's tabbable elements.
    const drawerTabGuard = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;
      if (getActiveRegion() !== "bottom") return;
      if (!isBottomDrawerOpen()) return;
      const active = document.activeElement as HTMLElement | null;
      // If focus is already inside the drawer, let the drawer's own handler deal with it.
      if (active?.closest(".v9-bottom")) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      focusDrawerEditor();
    };

    document.addEventListener("keydown", spaceHandler);
    document.addEventListener("keydown", altLeaderHandler, true);
    document.addEventListener("tesela:open-leader-at", openLeaderAtHandler);
    document.addEventListener("keydown", panelHandler);
    document.addEventListener("keydown", ctrlWHandler, true);
    document.addEventListener("keydown", cmdZHandler, true);
    document.addEventListener("keydown", escHandler);
    document.addEventListener("tesela:leader", leaderHandler);
    document.addEventListener("tesela:focus-pane", focusPaneHandler);
    document.addEventListener("keydown", drawerTabHandler, true);
    document.addEventListener("keydown", gtHandler);
    document.addEventListener("keydown", drawerTabGuard, true);
    return () => {
      document.removeEventListener("keydown", spaceHandler);
      document.removeEventListener("keydown", altLeaderHandler, true);
      document.removeEventListener("tesela:open-leader-at", openLeaderAtHandler);
      document.removeEventListener("keydown", panelHandler);
      document.removeEventListener("keydown", ctrlWHandler, true);
      document.removeEventListener("keydown", cmdZHandler, true);
      document.removeEventListener("keydown", escHandler);
      document.removeEventListener("tesela:leader", leaderHandler);
      document.removeEventListener("tesela:focus-pane", focusPaneHandler);
      document.removeEventListener("keydown", drawerTabHandler, true);
      document.removeEventListener("keydown", gtHandler);
      document.removeEventListener("keydown", drawerTabGuard, true);
      if (pendingTimer) clearTimeout(pendingTimer);
      clearPendingG();
    };
  });
</script>

<svelte:head>
  <title>Tesela</title>
</svelte:head>

<QueryClientProvider client={queryClient}>
  <div
    class="v9 dark {drawerOpen ? 'with-bottom' : ''} {railOpen ? '' : 'rail-collapsed'} drawer-{drawerSide}"
    style:--v9-drawer-h={drawerHeight + 'px'}
    style:--v9-drawer-w={drawerWidth + 'px'}
  >
    <CrumbBar />
    <Rail />
    {#if !railOpen}
      <button
        class="v9-rail-reveal"
        onclick={toggleRail}
        title="Expand rail (r)"
        aria-label="Expand rail"
      >
        <IconChevronRight size={14} stroke={2} />
      </button>
    {/if}
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
    <ChordMenu
      tree={leaderTree}
      initialPath={leaderInitialPath}
      onclose={() => { showLeaderMenu = false; leaderInitialPath = []; }}
    />
  {/if}
  {#if activeToast}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="fixed bottom-6 right-6 z-50 max-w-md rounded-lg border px-4 py-2.5 text-[13px] shadow-lg cursor-pointer transition-opacity tesela-toast tesela-toast-{activeToast.tone}"
      onclick={clearToast}
    >
      {activeToast.message}
    </div>
  {/if}
</QueryClientProvider>

<style>
  .tesela-toast {
    backdrop-filter: blur(8px);
    animation: tesela-toast-in 0.18s ease-out;
  }
  .tesela-toast-info {
    background: hsl(var(--popover) / 0.95);
    border-color: hsl(var(--border));
    color: hsl(var(--popover-foreground));
  }
  .tesela-toast-success {
    background: hsl(142 70% 35% / 0.95);
    border-color: hsl(142 70% 45%);
    color: white;
  }
  .tesela-toast-warn {
    background: hsl(38 92% 45% / 0.95);
    border-color: hsl(38 92% 55%);
    color: white;
  }
  .tesela-toast-error {
    background: hsl(0 75% 50% / 0.95);
    border-color: hsl(0 75% 60%);
    color: white;
  }
  @keyframes tesela-toast-in {
    from { opacity: 0; transform: translateY(8px); }
    to { opacity: 1; transform: translateY(0); }
  }
</style>
