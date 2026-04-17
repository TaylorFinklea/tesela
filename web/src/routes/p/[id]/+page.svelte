<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { api, ApiError } from "$lib/api-client";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import TagTable from "$lib/components/TagTable.svelte";
  import TagPropertyConfig from "$lib/components/TagPropertyConfig.svelte";
  import ViewSwitcher from "$lib/components/ViewSwitcher.svelte";
  import KanbanBoard from "$lib/components/KanbanBoard.svelte";
  import SplitDivider from "$lib/components/SplitDivider.svelte";
  import RightSidebar from "$lib/components/RightSidebar.svelte";
  import { getViewMode, setViewMode } from "$lib/stores/tag-view-prefs.svelte";
  import {
    isSplitOpen,
    getActivePane,
    getSplitRatio,
    setActivePane,
    openSplit,
    closeSplit,
    setSplitRatio,
    isVimEnabled,
  } from "$lib/stores/pane-state.svelte";
  import type { Note } from "$lib/types/Note";
  import { addRecent } from "$lib/stores/recents.svelte";
  import { goto } from "$app/navigation";
  import { untrack } from "svelte";
  import { IconTrash, IconStar, IconStarFilled } from "@tabler/icons-svelte";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";
  import { isFavorite, toggleFavorite } from "$lib/stores/favorites.svelte";

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

  // Split pane derived state
  const tagName = $derived(note?.title ?? "");
  const viewMode = $derived(tagName ? getViewMode(tagName) : "table");
  const vimOn = $derived(isVimEnabled());
  const showSplit = $derived(vimOn && isSplitOpen() && isTagPage && viewMode === "kanban");
  const activePane = $derived(getActivePane());
  const splitRatio = $derived(getSplitRatio());

  // Auto-open split on tag pages in kanban mode (with Vim on)
  $effect(() => {
    if (isTagPage && vimOn && viewMode === "kanban" && !isSplitOpen()) {
      untrack(() => openSplit());
    }
  });

  // Close split when navigating away from tag pages
  $effect(() => {
    if (!isTagPage && isSplitOpen()) {
      untrack(() => closeSplit());
    }
  });

  function handleViewChange(mode: "table" | "kanban") {
    if (!tagName) return;
    setViewMode(tagName, mode);
    if (mode === "kanban" && vimOn) openSplit();
    else closeSplit();
  }

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
    <!-- Top pane: note header + outliner + tag config + (inline kanban/table when not split) -->
    <div
      class="overflow-y-auto transition-shadow"
      style="
        {showSplit ? `height: ${splitRatio}%` : 'flex: 1 1 0%'};
        {showSplit && activePane === 'outliner' ? 'box-shadow: inset 2px 0 0 0 var(--primary)' : ''}
      "
      onclick={() => { if (showSplit) setActivePane('outliner'); }}
    >
      <!-- Focus Mode header -->
      <div class="max-w-3xl mx-auto px-10 pt-10 pb-4">
        {#if note}
          <div class="flex items-center gap-2 text-[12px] text-muted-foreground mb-4">
            <a href="/" class="hover:text-primary transition-colors">Notes</a>
            <span>›</span>
            <span>{note.title}</span>
            <div class="flex-1"></div>
            <button
              onclick={() => toggleFavorite(noteId)}
              class="p-1 rounded-md transition-all {isFavorite(noteId) ? 'text-primary' : 'text-muted-foreground/40 hover:text-primary/60 hover:bg-primary/10'}"
              title={isFavorite(noteId) ? "Remove from favorites" : "Add to favorites"}
            >
              {#if isFavorite(noteId)}
                <IconStarFilled size={14} stroke={1.5} />
              {:else}
                <IconStar size={14} stroke={1.5} />
              {/if}
            </button>
            <button
              onclick={deleteNote}
              class="text-muted-foreground/40 hover:text-destructive p-1 rounded-md hover:bg-destructive/10 transition-all"
              title="Delete note"
            >
              <IconTrash size={14} stroke={1.5} />
            </button>
          </div>
          <h1 class="font-display text-3xl font-semibold tracking-tight leading-tight mb-2">{note.title}</h1>
          <div class="flex items-center gap-3 mb-8">
            {#if note.metadata.tags.length > 0}
              {#each note.metadata.tags as tag}
                <span class="text-[11px] px-2.5 py-0.5 rounded-full bg-primary/10 text-primary font-medium">{tag}</span>
              {/each}
            {/if}
            {#if isTagPage}
              <span class="text-[11px] px-2.5 py-0.5 rounded-full bg-primary/10 text-primary font-medium">Tag</span>
            {/if}
          </div>
        {:else}
          <div class="py-8 text-muted-foreground">Loading…</div>
        {/if}
      </div>

      <!-- Content -->
      <div class="max-w-3xl mx-auto px-10 pb-16">
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
              <div class="flex items-center justify-between mb-3">
                <h2 class="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">
                  #{note.title} Blocks
                </h2>
                <ViewSwitcher mode={viewMode} onchange={handleViewChange} />
              </div>
              {#if !showSplit}
                {#if viewMode === "kanban"}
                  <KanbanBoard tagName={note.title} />
                {:else}
                  <TagTable tagName={note.title} />
                {/if}
              {:else}
                <div class="text-[11px] text-muted-foreground/60 italic py-2">
                  Kanban open in split pane below. Ctrl+w j/k to switch panes.
                </div>
              {/if}
            </div>
          </div>
        {/if}
      {/if}
      </div>
    </div>

    <!-- Split divider + bottom pane -->
    {#if showSplit && note}
      <SplitDivider onresize={(r: number) => setSplitRatio(r)} />
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="overflow-hidden flex flex-col transition-shadow"
        style="
          height: {100 - splitRatio}%;
          background: var(--surface);
          {activePane === 'kanban' ? 'box-shadow: inset 2px 0 0 0 var(--primary)' : ''}
        "
        onclick={() => setActivePane('kanban')}
      >
        <div class="flex-1 overflow-y-auto px-4 py-3">
          <KanbanBoard tagName={note.title} focused={activePane === "kanban"} />
        </div>
      </div>
    {/if}
  </div>

  <RightSidebar
    noteId={noteId}
    collapsed={rightSidebarCollapsed}
    onToggle={() => (rightSidebarCollapsed = !rightSidebarCollapsed)}
  />
</div>
