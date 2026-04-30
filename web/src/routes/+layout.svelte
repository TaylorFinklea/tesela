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
  } from "$lib/stores/pane-state.svelte";
  import CrumbBar from "$lib/components/CrumbBar.svelte";
  import Rail from "$lib/components/Rail.svelte";
  import MiddleColumn from "$lib/components/MiddleColumn.svelte";
  import BottomDrawer from "$lib/components/BottomDrawer.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import LeaderMenu from "$lib/components/LeaderMenu.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
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
        target.closest(".cm-editor");
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
            if (r === "focus") setActiveRegion("middle");
            else if (r === "middle") setActiveRegion("rail");
            else if (r === "bottom") setActiveRegion("focus");
            break;
          }
          case "l": {
            const r = getActiveRegion();
            if (r === "rail") setActiveRegion("middle");
            else if (r === "middle") setActiveRegion("focus");
            break;
          }
          case "j": {
            const r = getActiveRegion();
            if (r === "focus") {
              if (isBottomDrawerOpen()) setActiveRegion("bottom");
              else if (isSplitOpen()) setActivePane("kanban");
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
          case "q": closeSplit(); break;
          case "=": setSplitRatio(50); break;
          case "+": adjustSplitRatio(-10); break;
          case "-": adjustSplitRatio(10); break;
        }
        clearPending();
      }
    };

    document.addEventListener("keydown", spaceHandler);
    document.addEventListener("keydown", panelHandler);
    document.addEventListener("keydown", ctrlWHandler, true);
    document.addEventListener("tesela:leader", leaderHandler);
    return () => {
      document.removeEventListener("keydown", spaceHandler);
      document.removeEventListener("keydown", panelHandler);
      document.removeEventListener("keydown", ctrlWHandler, true);
      document.removeEventListener("tesela:leader", leaderHandler);
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
    <MiddleColumn />
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
