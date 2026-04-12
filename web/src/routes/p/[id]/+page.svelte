<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { api, ApiError } from "$lib/api-client";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import TagTable from "$lib/components/TagTable.svelte";
  import TagPropertyConfig from "$lib/components/TagPropertyConfig.svelte";
  import RightSidebar from "$lib/components/RightSidebar.svelte";
  import type { Note } from "$lib/types/Note";
  import { addRecent } from "$lib/stores/recents.svelte";
  import { goto } from "$app/navigation";
  import { untrack } from "svelte";
  import { IconTrash } from "@tabler/icons-svelte";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";

  const queryClient = useQueryClient();
  const noteId = $derived(page.params.id ?? "");

  // Track recently viewed notes (untrack the write to prevent infinite loop)
  $effect(() => {
    const id = noteId;
    if (id) untrack(() => addRecent(id));
  });

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId),
    enabled: noteId !== "",
  }));

  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---")) return { frontmatter: "", body: content };
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return { frontmatter: "", body: content };
    const fmEnd = endIdx + 3;
    const afterFm = content.slice(fmEnd);
    const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
    return { frontmatter: content.slice(0, fmEnd) + "\n", body: afterFm.slice(bodyStart) };
  }

  const note: Note | undefined = $derived(noteQuery.data as Note | undefined);
  const split = $derived(splitContent(note?.content ?? ""));

  // Detect if this is a Tag page (show table view)
  const isTagPage = $derived(note?.metadata.note_type === "Tag");

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let rightSidebarCollapsed = $state(false);

  async function deleteNote() {
    if (!note) return;
    const confirmed = window.confirm(`Delete "${note.title}"? This cannot be undone.`);
    if (!confirmed) return;
    try {
      await api.deleteNote(noteId);
      queryClient.invalidateQueries({ queryKey: ["notes"] });
      goto("/");
    } catch (e) {
      console.error("Failed to delete:", e);
    }
  }

  function handleContentChange(fullContent: string) {
    if (saveTimer) clearTimeout(saveTimer);
    setSaving();
    saveTimer = setTimeout(async () => {
      try {
        const updated = await api.updateNote(noteId, fullContent);
        queryClient.setQueryData(["note", noteId], updated);
        setSaved();
      } catch (e) {
        const msg = e instanceof Error ? e.message : "Unknown error";
        setSaveError(msg);
        console.error("Save failed:", e);
      }
    }, 500);
  }
</script>

<div class="flex-1 flex min-h-0">
  <div class="flex-1 flex flex-col min-w-0">
    <header class="border-b border-border px-6 h-[52px] flex items-center gap-3 shrink-0">
      <a href="/" class="text-[12px] text-muted-foreground/40 hover:text-primary transition-colors">&larr;</a>
      {#if note}
        <h1 class="text-[15px] font-bold tracking-tight truncate">{note.title}</h1>
        {#if note.metadata.tags.length > 0}
          <div class="flex gap-1.5">
            {#each note.metadata.tags as tag}
              <span class="text-[10px] px-2 py-0.5 rounded-full bg-blue-500/10 text-blue-300/80 border border-blue-500/10 font-medium">{tag}</span>
            {/each}
          </div>
        {/if}
        {#if isTagPage}
          <span class="text-[10px] px-2 py-0.5 rounded-full bg-primary/10 text-primary font-medium border border-primary/15">Tag</span>
        {/if}
        <div class="flex-1"></div>
        <button
          onclick={deleteNote}
          class="text-muted-foreground/40 hover:text-destructive p-1 rounded-md hover:bg-destructive/10 transition-all"
          title="Delete note"
        >
          <IconTrash size={14} stroke={1.5} />
        </button>
      {:else}
        <h1 class="text-[13px] font-semibold tracking-tight text-muted-foreground">Loading…</h1>
      {/if}
    </header>

    <div class="flex-1 overflow-y-auto px-6 py-4">
      {#if noteQuery.isLoading}
        <div class="text-sm text-muted-foreground">Loading…</div>
      {:else if noteQuery.isError}
        {@const error = noteQuery.error}
        <div class="text-sm">
          <div class="text-destructive font-medium">Could not load note</div>
          <div class="mt-1 text-muted-foreground">
            {error instanceof ApiError ? `${error.status} — ${error.body || "unknown"}` : error.message}
          </div>
        </div>
      {:else if note}
        <BlockOutliner
          noteId={note.id}
          body={split.body}
          frontmatter={split.frontmatter}
          onContentChange={handleContentChange}
          onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
        />

        {#if isTagPage}
          <div class="mt-6 pt-4 border-t border-border space-y-6">
            <TagPropertyConfig tagName={note.title} noteId={note.id} />

            <div>
              <h2 class="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
                #{note.title} Blocks
              </h2>
              <TagTable tagName={note.title} />
            </div>
          </div>
        {/if}
      {/if}
    </div>
  </div>

  <RightSidebar
    noteId={noteId}
    collapsed={rightSidebarCollapsed}
    onToggle={() => (rightSidebarCollapsed = !rightSidebarCollapsed)}
  />
</div>
