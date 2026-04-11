<script lang="ts">
  import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
  import { onMount } from "svelte";
  import { connect } from "$lib/ws-client.svelte";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import CommandPalette from "$lib/components/CommandPalette.svelte";
  import LeaderMenu from "$lib/components/LeaderMenu.svelte";
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
    document.addEventListener("keydown", spaceHandler);
    document.addEventListener("tesela:leader", leaderHandler);
    return () => {
      document.removeEventListener("keydown", spaceHandler);
      document.removeEventListener("tesela:leader", leaderHandler);
    };
  });
</script>

<svelte:head>
  <title>Tesela</title>
</svelte:head>

<QueryClientProvider client={queryClient}>
  <div class="flex h-full dark">
    <Sidebar collapsed={sidebarCollapsed} onToggle={() => (sidebarCollapsed = !sidebarCollapsed)} />
    <main class="flex-1 flex flex-col min-w-0 overflow-hidden">
      {@render children()}
    </main>
  </div>
  <CommandPalette />
  {#if showLeaderMenu}
    <LeaderMenu onclose={() => (showLeaderMenu = false)} />
  {/if}
</QueryClientProvider>
