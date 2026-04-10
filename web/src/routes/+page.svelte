<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api, ApiError } from "$lib/api-client";
  import { getConnected, setHandlers } from "$lib/ws-client.svelte";
  import type { Note } from "$lib/types/Note";
  import { onMount } from "svelte";

  const notesQuery = createQuery(() => ({ queryKey: ["notes", { limit: 100 }] as const, queryFn: () => api.listNotes({ limit: 100 }) }));
  const notes: Note[] | undefined = $derived(notesQuery.data as Note[] | undefined);
  const wsConnected = $derived(getConnected());

  onMount(() => {
    setHandlers({
      onNoteCreated: () => notesQuery.refetch(),
      onNoteUpdated: () => notesQuery.refetch(),
      onNoteDeleted: () => notesQuery.refetch(),
    });
  });

  function formatTimestamp(iso: string): string {
    try {
      const d = new Date(iso);
      return d.toLocaleString(undefined, { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
    } catch {
      return iso;
    }
  }
</script>

<div class="flex-1 flex flex-col">
  <header class="border-b border-border px-6 py-3 flex items-center justify-between">
    <span class="text-xs text-muted-foreground">All Notes</span>
    <div class="flex items-center gap-2 text-xs text-muted-foreground">
      <span class="inline-block h-1.5 w-1.5 rounded-full {wsConnected ? 'bg-emerald-500' : 'bg-muted-foreground'}"></span>
      <span>{notesQuery.isLoading ? "loading" : wsConnected ? "live" : "offline"}</span>
    </div>
  </header>

  <section class="flex-1 overflow-y-auto">
    {#if notesQuery.isLoading}
      <div class="px-6 py-8 text-sm text-muted-foreground">Loading…</div>
    {:else if notesQuery.isError}
      {@const error = notesQuery.error}
      <div class="px-6 py-8 text-sm">
        <div class="text-destructive font-medium">Could not reach tesela-server</div>
        <div class="mt-1 text-muted-foreground">
          {error instanceof ApiError ? `${error.status} — ${error.body || "no body"}` : error.message}
        </div>
        <div class="mt-3 text-xs text-muted-foreground">
          Start it with <code class="font-mono">cargo run -p tesela-server</code> and reload.
        </div>
      </div>
    {:else if notes && notes.length === 0}
      <div class="px-6 py-8 text-sm text-muted-foreground">No notes yet.</div>
    {:else if notes}
      <ul class="divide-y divide-border">
        {#each notes as note (note.id)}
          <li>
            <a href="/p/{encodeURIComponent(note.id)}" class="block px-6 py-3 hover:bg-accent/50">
              <div class="flex items-baseline justify-between gap-4">
                <span class="text-sm font-medium truncate">{note.title}</span>
                <span class="text-xs text-muted-foreground font-mono shrink-0">{formatTimestamp(note.modified_at)}</span>
              </div>
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>
