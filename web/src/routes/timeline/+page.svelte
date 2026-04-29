<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";

  const queryClient = useQueryClient();

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { tag: "daily", limit: 100 }] as const,
    queryFn: () => api.listNotes({ tag: "daily", limit: 100 }),
  }));

  const dailyNotes: Note[] = $derived(
    ((notesQuery.data ?? []) as Note[]).sort((a, b) => b.title.localeCompare(a.title)),
  );

  // Check if today's note exists
  const todayStr = new Date().toISOString().slice(0, 10);
  const todayExists = $derived(dailyNotes.some((n) => n.title === todayStr || n.id === todayStr));

  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---")) return { frontmatter: "", body: content };
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return { frontmatter: "", body: content };
    const fmEnd = endIdx + 3;
    const afterFm = content.slice(fmEnd);
    const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
    return { frontmatter: content.slice(0, fmEnd) + "\n", body: afterFm.slice(bodyStart) };
  }

  function formatDate(dateStr: string): string {
    try {
      const d = new Date(dateStr + "T00:00:00");
      return d.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric" });
    } catch {
      return dateStr;
    }
  }

  function formatYear(dateStr: string): string {
    return dateStr.slice(0, 4);
  }

  function isToday(dateStr: string): boolean {
    return dateStr === todayStr;
  }

  async function createToday() {
    try {
      const note = await api.getDailyNote();
      queryClient.invalidateQueries({ queryKey: ["notes"] });
    } catch (e) {
      console.error("Failed to create daily note:", e);
    }
  }

  // Save handlers per note
  // TODO(3M.2): mirror the cancel-and-flush pattern from /p/[id]/+page.svelte
  // (per-noteId AbortController + cancelAndFlush, wired to BlockOutliner's
  // onCancelAndFlush prop). Until then, undo on the timeline degrades to the
  // existing focus-guarded behavior — i.e. an in-flight pre-undo PUT can still
  // race the restored state if the user blurs immediately after undo. The
  // common-case undo (focus stays on the block) is unaffected.
  const saveTimers = new Map<string, ReturnType<typeof setTimeout>>();

  function handleContentChange(noteId: string, fullContent: string) {
    const existing = saveTimers.get(noteId);
    if (existing) clearTimeout(existing);
    setSaving();
    saveTimers.set(noteId, setTimeout(async () => {
      try {
        await api.updateNote(noteId, fullContent);
        queryClient.invalidateQueries({ queryKey: ["note", noteId] });
        setSaved();
      } catch (e) {
        const msg = e instanceof Error ? e.message : "Unknown error";
        setSaveError(msg);
      }
    }, 500));
  }
</script>

<div class="flex-1 flex flex-col">
  <div class="flex-1 overflow-y-auto">
    <div class="max-w-3xl mx-auto px-10 py-10">

      <!-- Header -->
      <h1 class="font-display text-2xl font-semibold tracking-tight mb-8">Journal</h1>

      <!-- Create today's note if it doesn't exist -->
      {#if notesQuery.data && !todayExists}
        <div class="mb-8 p-6 rounded-xl border border-border bg-block-bg">
          <div class="flex items-center justify-between">
            <div>
              <div class="font-display text-lg font-semibold text-primary">{formatDate(todayStr)}</div>
              <div class="text-[12px] text-muted-foreground mt-1">No note for today yet</div>
            </div>
            <button
              onclick={createToday}
              class="px-4 py-2 rounded-lg bg-primary text-primary-foreground text-[13px] font-medium hover:opacity-90 transition-opacity"
            >
              Start Today's Note
            </button>
          </div>
        </div>
      {/if}

      {#if notesQuery.isLoading}
        <div class="text-muted-foreground">Loading journal…</div>
      {:else if dailyNotes.length === 0 && todayExists}
        <div class="text-muted-foreground">Your journal is empty.</div>
      {:else}
        <!-- Daily notes stream -->
        {#each dailyNotes as note (note.id)}
          {@const split = splitContent(note.content)}
          {@const today = isToday(note.title)}

          <div class="mb-10">
            <!-- Date header -->
            <div class="flex items-baseline gap-3 mb-4 pb-2 border-b border-border">
              <h2 class="font-display text-lg font-semibold {today ? 'text-primary' : 'text-foreground'}">
                {formatDate(note.title)}
              </h2>
              {#if today}
                <span class="text-[10px] px-2 py-0.5 rounded-full bg-primary text-primary-foreground font-medium">Today</span>
              {/if}
              <span class="text-[11px] text-muted-foreground">{formatYear(note.title)}</span>
            </div>

            <!-- Editable blocks inline -->
            <BlockOutliner
              noteId={note.id}
              body={split.body}
              frontmatter={split.frontmatter}
              onContentChange={(content) => handleContentChange(note.id, content)}
              onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
            />
          </div>
        {/each}
      {/if}

    </div>
  </div>
</div>
