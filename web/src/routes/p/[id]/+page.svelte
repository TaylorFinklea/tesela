<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { api, ApiError } from "$lib/api-client";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import TagTable from "$lib/components/TagTable.svelte";
  import RightSidebar from "$lib/components/RightSidebar.svelte";
  import type { Note } from "$lib/types/Note";

  const queryClient = useQueryClient();
  const noteId = $derived(page.params.id ?? "");

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

  function handleContentChange(fullContent: string) {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(async () => {
      try {
        const updated = await api.updateNote(noteId, fullContent);
        queryClient.setQueryData(["note", noteId], updated);
      } catch (e) {
        console.error("Save failed:", e);
      }
    }, 500);
  }
</script>

<div class="flex-1 flex min-h-0">
  <div class="flex-1 flex flex-col min-w-0">
    <header class="border-b border-border px-6 py-4 flex items-center gap-4">
      <a href="/" class="text-xs text-muted-foreground hover:text-foreground">&larr; Notes</a>
      {#if note}
        <h1 class="text-sm font-medium tracking-tight truncate">{note.title}</h1>
        {#if note.metadata.tags.length > 0}
          <div class="flex gap-1">
            {#each note.metadata.tags as tag}
              <span class="text-xs px-1.5 py-0.5 rounded bg-accent text-accent-foreground">{tag}</span>
            {/each}
          </div>
        {/if}
        {#if isTagPage}
          <span class="text-xs px-1.5 py-0.5 rounded bg-primary/10 text-primary">Tag</span>
        {/if}
      {:else}
        <h1 class="text-sm font-medium tracking-tight">Loading…</h1>
      {/if}
    </header>

    <div class="flex-1 overflow-y-auto px-8 py-4">
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
          <div class="mt-6 pt-4 border-t border-border">
            <h2 class="text-xs font-medium text-muted-foreground uppercase tracking-wider mb-3">
              #{note.title} Blocks
            </h2>
            <TagTable tagName={note.title} />
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
