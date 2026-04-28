<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { page } from "$app/state";
  import { api, ApiError } from "$lib/api-client";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import DocumentEditor from "$lib/components/DocumentEditor.svelte";
  import TagTable from "$lib/components/TagTable.svelte";
  import TagPropertyConfig from "$lib/components/TagPropertyConfig.svelte";
  import ViewSwitcher from "$lib/components/ViewSwitcher.svelte";
  import { IconTable, IconLayoutKanban } from "@tabler/icons-svelte";
  import KanbanBoard from "$lib/components/KanbanBoard.svelte";
  import SplitDivider from "$lib/components/SplitDivider.svelte";
  import RightSidebar from "$lib/components/RightSidebar.svelte";
  import PropertyTypeConfig from "$lib/components/PropertyTypeConfig.svelte";
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
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import { parseBlocks } from "$lib/block-parser";
  import { addRecent } from "$lib/stores/recents.svelte";
  import { goto } from "$app/navigation";
  import { untrack } from "svelte";
  import { IconTrash, IconStar, IconStarFilled, IconFileText, IconLayoutList } from "@tabler/icons-svelte";
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

  // Drill-in URL state
  const drillBlockId = $derived(page.url.searchParams.get("block") ?? "");
  const drillBlock = $derived.by((): ParsedBlock | null => {
    if (!drillBlockId || !note) return null;
    return parseBlocks(note.id, split.body).find(b => b.id === drillBlockId) ?? null;
  });

  function drillInto(blockId: string) {
    goto(`?block=${encodeURIComponent(blockId)}`, { replaceState: false, noScroll: true });
  }
  function drillOut() {
    goto(page.url.pathname, { replaceState: false, noScroll: true });
  }

  // Detect if this is a Tag page (show table view)
  const isTagPage = $derived(note?.metadata.note_type === "Tag");
  const isPropertyPage = $derived(note?.metadata.note_type === "Property");

  // Document mode: stored as `mode: document` in frontmatter
  const isDocumentMode = $derived(note?.metadata.custom.mode === "document");

  function toggleDocumentMode() {
    if (!note) return;
    const { frontmatter, body } = split;
    let newFm: string;
    if (isDocumentMode) {
      newFm = frontmatter.replace(/^mode: document\n/m, "");
    } else if (frontmatter) {
      const lastDash = frontmatter.lastIndexOf("---");
      newFm = frontmatter.slice(0, lastDash) + "mode: document\n---\n";
    } else {
      newFm = "---\nmode: document\n---\n";
    }
    handleContentChange(`${newFm}${body}`);
  }

  // When in document mode + drilled in, compute the subtree body and splice context
  function blocksToText(bs: ParsedBlock[]): string {
    return bs.map(b => {
      const indent = "  ".repeat(b.indent_level);
      const lines = b.raw_text.split("\n");
      const first = `${indent}- ${lines[0]}`;
      const rest = lines.slice(1).map(l => `${indent}  ${l}`);
      return [first, ...rest].join("\n");
    }).join("\n");
  }

  const drillSplice = $derived.by(() => {
    if (!isDocumentMode || !drillBlockId || !note) return null;
    const allBlocks = parseBlocks(note.id, split.body);
    const rootIdx = allBlocks.findIndex(b => b.id === drillBlockId);
    if (rootIdx < 0) return null;
    const rootIndent = allBlocks[rootIdx].indent_level;
    const sub: ParsedBlock[] = [];
    let endIdx = allBlocks.length;
    for (let i = rootIdx; i < allBlocks.length; i++) {
      if (i > rootIdx && allBlocks[i].indent_level <= rootIndent) { endIdx = i; break; }
      sub.push(allBlocks[i]);
    }
    return { prefix: allBlocks.slice(0, rootIdx), sub, suffix: allBlocks.slice(endIdx) };
  });

  const documentBody = $derived.by(() => {
    if (!isDocumentMode) return split.body;
    if (!drillSplice) return split.body;
    return blocksToText(drillSplice.sub) + "\n";
  });

  function handleDocumentChange(editedBody: string) {
    if (!drillSplice) {
      handleContentChange(`${split.frontmatter}${editedBody}`);
      return;
    }
    const pre = drillSplice.prefix.length > 0 ? blocksToText(drillSplice.prefix) + "\n" : "";
    const suf = drillSplice.suffix.length > 0 ? "\n" + blocksToText(drillSplice.suffix) + "\n" : "\n";
    handleContentChange(`${split.frontmatter}${pre}${editedBody.replace(/\n$/, "")}${suf}`);
  }

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
  let focusedBlock = $state<ParsedBlock | null>(null);

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
      onclick={() => setActivePane('outliner')}
    >
      <!-- Focus Mode header -->
      <div class="max-w-3xl mx-auto px-10 pt-10 pb-4">
        {#if note}
          <div class="flex items-center gap-2 text-[12px] text-muted-foreground mb-4">
            <a href="/" class="hover:text-primary transition-colors">Notes</a>
            <span>›</span>
            {#if drillBlock}
              <button onclick={drillOut} class="hover:text-primary transition-colors">{note.title}</button>
              <span>›</span>
              <span class="text-foreground/70 truncate max-w-[240px]">{drillBlock.text}</span>
            {:else}
              <span>{note.title}</span>
            {/if}
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
              onclick={toggleDocumentMode}
              class="p-1 rounded-md transition-all {isDocumentMode ? 'text-primary bg-primary/10' : 'text-muted-foreground/40 hover:text-primary/60 hover:bg-primary/10'}"
              title={isDocumentMode ? "Switch to outline mode" : "Switch to document mode"}
            >
              {#if isDocumentMode}
                <IconLayoutList size={14} stroke={1.5} />
              {:else}
                <IconFileText size={14} stroke={1.5} />
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
            {#if note.metadata.note_type}
              <span class="text-[11px] px-2.5 py-0.5 rounded-full bg-primary/10 text-primary font-medium">{note.metadata.note_type}</span>
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
        {#if isDocumentMode}
          {#key drillBlockId}
            <DocumentEditor
              body={documentBody}
              frontmatter={split.frontmatter}
              onContentChange={handleDocumentChange}
            />
          {/key}
        {:else}
          <BlockOutliner
            noteId={note.id}
            body={split.body}
            frontmatter={split.frontmatter}
            onContentChange={handleContentChange}
            onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
            onfocusedblockchange={(b) => { focusedBlock = b; }}
            {drillBlockId}
            onDrillIn={drillInto}
          />
        {/if}

        {#if isPropertyPage}
          <div class="mt-6 pt-4 border-t border-border">
            <PropertyTypeConfig note={note} />
          </div>
        {/if}

        {#if isTagPage}
          <div class="mt-6 pt-4 border-t border-border space-y-6">
            <TagPropertyConfig tagName={note.title} noteId={note.id} />

            <div>
              <div class="flex items-center justify-between mb-3">
                <h2 class="text-[10px] font-medium text-muted-foreground/60 uppercase tracking-widest">
                  #{note.title} Blocks
                </h2>
                <ViewSwitcher
                  views={[
                    { id: "table", label: "Table", Icon: IconTable },
                    { id: "kanban", label: "Kanban", Icon: IconLayoutKanban },
                  ]}
                  active={viewMode}
                  onChange={handleViewChange}
                />
              </div>
              {#if !showSplit}
                {#if viewMode === "kanban"}
                  <KanbanBoard tagName={note.title} />
                {:else}
                  <TagTable tagName={note.title} noteId={noteId} />
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
    {focusedBlock}
  />
</div>
