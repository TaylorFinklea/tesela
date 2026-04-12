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
  <header class="border-b border-border px-6 h-[52px] flex items-center justify-between shrink-0">
    <span class="text-[15px] font-bold tracking-tight">All Notes</span>
    <div class="flex items-center gap-2 text-[11px] text-muted-foreground/40">
      <span class="inline-block h-[6px] w-[6px] rounded-full {wsConnected ? 'bg-emerald-400/70' : 'bg-muted-foreground/30'}"></span>
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
      <ul>
        {#each notes as note (note.id)}
          <li>
            <a href="/p/{encodeURIComponent(note.id)}" class="block px-6 py-3 hover:bg-muted/30 transition-all border-b border-border/30">
              <div class="flex items-center justify-between gap-4">
                <div class="flex items-center gap-2.5 min-w-0">
                  <span class="text-[13px] font-medium truncate">{note.title}</span>
                  {#if note.metadata.tags.length > 0}
                    <span class="text-[9px] px-1.5 py-px rounded-full bg-blue-500/8 text-blue-300/50 border border-blue-500/8 shrink-0">{note.metadata.tags[0]}</span>
                  {/if}
                </div>
                <span class="text-[10px] text-muted-foreground/30 font-mono shrink-0">{formatTimestamp(note.modified_at)}</span>
              </div>
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>
