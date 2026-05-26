<script lang="ts">
  /*
   * Top-level layout for the SvelteKit app. Phase 6 hard-swap stripped
   * this down from the legacy chrome (Rail / BottomDrawer / CrumbBar /
   * CommandPalette / ChordMenu / leader tree) to a thin shell that just
   * wires the cross-cutting services every route depends on:
   *
   *   - QueryClient (TanStack Query)
   *   - WebSocket: note invalidations + notification handlers
   *   - System Query widgets bootstrap (idempotent)
   *   - Toast surface
   *
   * Visible chrome lives per-route: `/v4` mounts the Prism v4 shell;
   * `/settings/*` mounts its own settings nav. Routes outside those two
   * groups render directly without chrome — `/` redirects into `/v4`
   * and `/p/<id>` / `/daily` / `/timeline` / `/graph` / `/properties`
   * redirect via their `+page.ts` files, so the no-chrome surface is
   * never reached in practice.
   */
  import { QueryClient, QueryClientProvider } from "@tanstack/svelte-query";
  import { registerAppQueryClient } from "$lib/app-query-client.svelte";
  import { onMount } from "svelte";
  import { connect, setHandlers } from "$lib/ws-client.svelte";
  import {
    handleDeadlineApproaching,
    handleScheduledFires,
    handleRecurringRolled,
  } from "$lib/notifications";
  import { ensureSystemWidgets } from "$lib/system-widgets";
  import { api } from "$lib/api-client";
  import { getToast, clearToast } from "$lib/stores/toast.svelte";
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
  // Expose to plain-TS modules (commands.ts etc.) so verbs can invalidate
  // cache entries without going through Svelte context.
  registerAppQueryClient(queryClient);

  const activeToast = $derived(getToast());

  onMount(() => {
    connect();

    setHandlers({
      onNoteCreated: () => {
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        // A new note may carry dated/untriaged blocks that belong on
        // the agenda or in the inbox — refresh both ambients so they
        // pick up the new rows without a manual reload.
        queryClient.invalidateQueries({ queryKey: ["agenda"] });
        queryClient.invalidateQueries({ queryKey: ["widget", "inbox"] });
      },
      onNoteUpdated: (note) => {
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        queryClient.invalidateQueries({ queryKey: ["note", note.id] });
        queryClient.invalidateQueries({ queryKey: ["typed-blocks"] });
        // Any block-level change (status flip, scheduled / deadline /
        // recurring property edit, text edit) can shift which rows
        // belong on the agenda or in the inbox. Cheaper to invalidate
        // unconditionally than to scan the diff.
        queryClient.invalidateQueries({ queryKey: ["agenda"] });
        queryClient.invalidateQueries({ queryKey: ["widget", "inbox"] });
      },
      onNoteDeleted: (id) => {
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        queryClient.invalidateQueries({ queryKey: ["note", id] });
        queryClient.invalidateQueries({ queryKey: ["agenda"] });
        queryClient.invalidateQueries({ queryKey: ["widget", "inbox"] });
      },
      onDeadlineApproaching: handleDeadlineApproaching,
      onScheduledFires: handleScheduledFires,
      onRecurringRolled: handleRecurringRolled,
      onReconnected: () => {
        // After a WebSocket drop, server-side WsEvents that fired
        // during the gap were lost. Invalidate the same query keys
        // the per-note handlers would have to recover. Cheaper than
        // letting the user see stale data until next manual refresh.
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        queryClient.invalidateQueries({ queryKey: ["note"] });
        queryClient.invalidateQueries({ queryKey: ["typed-blocks"] });
        queryClient.invalidateQueries({ queryKey: ["agenda"] });
        queryClient.invalidateQueries({ queryKey: ["widget", "inbox"] });
      },
    });

    void ensureSystemWidgets(api);
  });
</script>

<svelte:head>
  <title>Tesela</title>
</svelte:head>

<QueryClientProvider client={queryClient}>
  {@render children()}
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
