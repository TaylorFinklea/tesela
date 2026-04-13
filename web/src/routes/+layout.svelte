<script lang="ts">
  import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
  import { onMount } from "svelte";
  import { connect, setHandlers } from "$lib/ws-client.svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { pushNavigation, goBack, goForward } from "$lib/stores/navigation.svelte";
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
      const savedMode = localStorage.getItem("tesela:mode") ?? "day";
      applyTheme(savedMode);
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

    document.addEventListener("keydown", spaceHandler);
    document.addEventListener("keydown", panelHandler);
    document.addEventListener("tesela:leader", leaderHandler);
    return () => {
      document.removeEventListener("keydown", spaceHandler);
      document.removeEventListener("keydown", panelHandler);
      document.removeEventListener("tesela:leader", leaderHandler);
    };
  });
</script>

<svelte:head>
  <title>Tesela</title>
</svelte:head>

<QueryClientProvider client={queryClient}>
  <div class="flex flex-col h-full dark">
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
