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
    setRefreshCallback,
    scheduleNoteRefresh,
    flushNoteRefreshNow,
  } from "$lib/ws-refresh-coordinator";
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

    // The coordinator owns the *timing* of WS-driven refetches (coalescing a
    // burst into ONE pass) and own-echo suppression; this callback owns *which*
    // queries to touch. Without coalescing, the server's per-PUT `note_updated`
    // echo made each edit fan out into a full multi-query refetch (the daily
    // list, the autocomplete list, every sidebar/ambient `["notes",...]` query)
    // — N edits → N passes, none merged — and those stale list responses
    // reseeded the actively-edited editor's body prop, clobbering the edit.
    setRefreshCallback(({ noteIds, broad }) => {
      if (broad) {
        // `["notes"]` prefix-matches every mounted `["notes", {…}]` query
        // (daily list, autocomplete list, sidebar/ambient lists). These feed
        // *views*, not the focused editor's body, so refreshing them on the
        // coalesced pass can't clobber an in-flight edit.
        queryClient.invalidateQueries({ queryKey: ["notes"] });
        queryClient.invalidateQueries({ queryKey: ["typed-blocks"] });
        queryClient.invalidateQueries({ queryKey: ["agenda"] });
        queryClient.invalidateQueries({ queryKey: ["widget", "inbox"] });
      }
      // Targeted `["note", id]` refetches feed the page/editor buffer
      // directly. Own-echo ids were already filtered out upstream
      // (`scheduleNoteRefresh`), so this only fires for genuine remote
      // changes to notes this client did NOT just save.
      for (const id of noteIds) {
        queryClient.invalidateQueries({ queryKey: ["note", id] });
      }
    });

    setHandlers({
      onNoteCreated: (note) => {
        // A new note may carry dated/untriaged blocks that belong on the
        // agenda or in the inbox — broad refresh picks them up.
        scheduleNoteRefresh(note.id, true);
      },
      onNoteUpdated: (note) => {
        // Any block-level change (status flip, scheduled / deadline /
        // recurring property edit, text edit) can shift which rows belong on
        // the agenda or in the inbox, so request the broad list refresh. The
        // targeted `["note", id]` refetch is skipped for our own echo inside
        // the coordinator so it can't race our optimistic editor update.
        scheduleNoteRefresh(note.id, true);
      },
      onNoteDeleted: (id) => {
        // A delete is never our own optimistic edit clobbering itself, but
        // routing it through the coordinator still coalesces it with any
        // concurrent burst. `isOwnEcho` would suppress a recently-saved id's
        // targeted refetch, which is fine — a deleted note's `["note", id]`
        // query is unmounted anyway; the broad refresh removes it from lists.
        scheduleNoteRefresh(id, true);
      },
      onDeadlineApproaching: handleDeadlineApproaching,
      onScheduledFires: handleScheduledFires,
      onRecurringRolled: handleRecurringRolled,
      onReconnected: () => {
        // After a WebSocket drop, server-side WsEvents that fired during the
        // gap were lost. Recover by forcing an immediate broad refresh (no
        // debounce — we want missed remote changes visible at once). Also
        // refresh every `["note", …]` query since we can't know which notes
        // changed while the socket was down.
        scheduleNoteRefresh(null, true);
        queryClient.invalidateQueries({ queryKey: ["note"] });
        flushNoteRefreshNow();
      },
      onBinaryDelta: (updates) => {
        // C2.1 STUB: the server now broadcasts TLR2 Loro-delta frames on every
        // edit and web decodes them here. We do NOT apply them to any Loro doc
        // yet — that's C2.2/C2.3. For now just log that a frame arrived + the
        // first doc id (hex) so the binary path is observable in the console.
        const firstDocHex = updates[0]
          ? Array.from(updates[0].doc)
              .map((b) => b.toString(16).padStart(2, "0"))
              .join("")
          : "(none)";
        console.debug(
          `[ws] TLR2 binary delta: ${updates.length} update(s), first doc=${firstDocHex}`,
        );
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
