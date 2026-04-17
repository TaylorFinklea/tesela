<script lang="ts">
  import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
  import { onMount } from "svelte";
  import { connect, setHandlers } from "$lib/ws-client.svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { pushNavigation, goBack, goForward } from "$lib/stores/navigation.svelte";
  import {
    isVimEnabled,
    isCtrlWPending,
    setCtrlWPending,
    setActivePane,
    toggleSplit,
    closeSplit,
    adjustSplitRatio,
    setSplitRatio,
  } from "$lib/stores/pane-state.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import LeaderMenu from "$lib/components/LeaderMenu.svelte";
  import StatusBar from "$lib/components/StatusBar.svelte";
  import { applyTheme } from "$lib/themes";
  import { browser } from "$app/environment";
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

  let sidebarCollapsed = $state(false);
  let showLeaderMenu = $state(false);

  onMount(() => {
    connect();

    // Wire WS events to invalidate TanStack Query caches globally
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

    // Apply saved theme
    if (browser) {
      const mode = localStorage.getItem("tesela:mode") ?? "day";
      applyTheme(mode);
    }

    // Space leader key — works outside editors AND from Vim normal mode (via custom event)
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
    // Listen for leader menu trigger from Vim normal mode inside CM6
    const leaderHandler = () => {
      showLeaderMenu = true;
    };
    // Global shortcuts (outside editors): 1, [, ], /
    const panelHandler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement;
      const isEditing =
        target.tagName === "INPUT" ||
        target.tagName === "TEXTAREA" ||
        target.isContentEditable ||
        target.closest(".cm-editor");
      if (isEditing) return;

      if (e.key === "1") { e.preventDefault(); sidebarCollapsed = !sidebarCollapsed; }
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
        // Dispatch ⌘K to open command palette
        document.dispatchEvent(new KeyboardEvent("keydown", { key: "k", metaKey: true, bubbles: true }));
      }
    };

    // Ctrl+w chord handler — Vim-style window commands.
    // Capture phase to beat browser's "close tab" on Ctrl+w.
    let pendingTimer: ReturnType<typeof setTimeout> | null = null;
    const clearPending = () => {
      setCtrlWPending(false);
      if (pendingTimer) { clearTimeout(pendingTimer); pendingTimer = null; }
    };

    const ctrlWHandler = (e: KeyboardEvent) => {
      if (showLeaderMenu) return; // leader menu takes priority
      if (!isVimEnabled()) return;

      // First key: Ctrl+w (lowercase or uppercase)
      if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key === "w" || e.key === "W")) {
        e.preventDefault();
        e.stopPropagation();
        setCtrlWPending(true);
        if (pendingTimer) clearTimeout(pendingTimer);
        pendingTimer = setTimeout(clearPending, 2000);
        return;
      }

      // Second key: dispatch window command
      if (isCtrlWPending()) {
        // Ignore modifier-only keydowns so user can press Shift to type "+"
        if (e.key === "Shift" || e.key === "Control" || e.key === "Alt" || e.key === "Meta") {
          return;
        }
        e.preventDefault();
        e.stopPropagation();
        switch (e.key) {
          case "j": setActivePane("kanban"); break;
          case "k": setActivePane("outliner"); break;
          case "s": toggleSplit(); break;
          case "q": closeSplit(); break;
          case "=": setSplitRatio(50); break;
          case "+": adjustSplitRatio(-10); break;
          case "-": adjustSplitRatio(10); break;
          // Escape or any other key: cancel silently
        }
        clearPending();
      }
    };

    document.addEventListener("keydown", spaceHandler);
    document.addEventListener("keydown", panelHandler);
    document.addEventListener("keydown", ctrlWHandler, true); // capture phase
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
  <div class="flex flex-col h-screen dark overflow-hidden">
    <div class="flex flex-1 min-h-0">
      <Sidebar collapsed={sidebarCollapsed} onToggle={() => (sidebarCollapsed = !sidebarCollapsed)} />
      <main class="flex-1 flex flex-col min-w-0 overflow-hidden">
        {@render children()}
      </main>
    </div>
    <StatusBar />
  </div>
  <CommandPalette />
  {#if showLeaderMenu}
    <LeaderMenu onclose={() => (showLeaderMenu = false)} />
  {/if}
</QueryClientProvider>
