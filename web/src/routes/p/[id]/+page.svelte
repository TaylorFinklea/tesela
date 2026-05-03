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
  import PropertyTypeConfig from "$lib/components/PropertyTypeConfig.svelte";
  import QueryWidgetView from "$lib/components/QueryWidgetView.svelte";
  import JournalView from "$lib/components/JournalView.svelte";
  import { widgetFromNote } from "$lib/widget-registry.svelte";
  import {
    setFocusedBlock,
    setLeftFocusedBlock,
    setRightFocusedBlock,
  } from "$lib/stores/current-block.svelte";
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
    getVSplitActiveSide,
    setVSplitActiveSide,
    getVSplitRatio,
    setVSplitRatio,
  } from "$lib/stores/pane-state.svelte";
  import { gotoNote, collapseSplit } from "$lib/stores/active-pane-nav.svelte";
  import type { Note } from "$lib/types/Note";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import { parseBlocks } from "$lib/block-parser";
  import { addRecent } from "$lib/stores/recents.svelte";
  import { goto } from "$app/navigation";
  import { onDestroy, untrack } from "svelte";
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

  // Drill-in URL state. The right (current) pane's drilled block.
  const drillBlockId = $derived(page.url.searchParams.get("block") ?? "");
  const drillBlock = $derived.by((): ParsedBlock | null => {
    if (!drillBlockId || !note) return null;
    return parseBlocks(note.id, split.body).find(b => b.id === drillBlockId) ?? null;
  });

  function drillInto(blockId: string) {
    // Per the column-view rule, drilling into a block is just another nav:
    // source = current right (this note unzoomed), target = same note + block.
    // gotoNote handles ?back= bookkeeping.
    gotoNote(noteId, blockId);
  }
  function drillOut() {
    // Drill-out of a block stays in the same pane (right) — clears ?block=
    // but preserves ?back= (the left pane shouldn't change).
    const params = new URLSearchParams(page.url.search);
    params.delete("block");
    const qs = params.toString();
    goto(`${page.url.pathname}${qs ? `?${qs}` : ""}`, { replaceState: false, noScroll: true });
  }

  // Detect if this is a Tag page (show table view)
  const isTagPage = $derived(note?.metadata.note_type === "Tag");
  const isPropertyPage = $derived(note?.metadata.note_type === "Property");
  const isQueryWidget = $derived(note?.metadata.note_type === "Query");
  const widget = $derived(note && isQueryWidget ? widgetFromNote(note) : null);

  // Phase 9.6 — Journal/Daily detection. The page renders the JournalView
  // (continuous multi-day scroll) when:
  //   - the note id is the "dailies" anchor, OR
  //   - the note has the `daily` tag (so /p/<YYYY-MM-DD> works), OR
  //   - the note has note_type "Daily" (forward-compat).
  // Drilling into a single block (?block=) opts back into the standard
  // BlockOutliner so the user can focus on one block without the journal scroll.
  const isDailyJournal = $derived(
    !drillBlockId &&
    (
      noteId === "dailies" ||
      note?.metadata.note_type === "Daily" ||
      (note?.metadata.tags ?? []).includes("daily")
    ),
  );
  function todayIso(): string { return new Date().toISOString().slice(0, 10); }
  const journalAnchor = $derived(
    /^\d{4}-\d{2}-\d{2}$/.test(noteId) ? noteId : todayIso(),
  );

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

  // Phase 9.5b — column-view state. The path is the right (current) pane;
  // ?back=<noteId>&backBlock=<id?> is the left (back-context) pane. The
  // split is shown whenever ?back= is present; URL is the source of truth.
  const vSplitActiveSide = $derived(getVSplitActiveSide());
  const vSplitRatio = $derived(getVSplitRatio());
  const backNoteId = $derived(page.url.searchParams.get("back") ?? "");
  const backBlockId = $derived(page.url.searchParams.get("backBlock") ?? "");
  const vSplitShown = $derived(backNoteId !== "");

  // Left (back-context) pane note query, enabled only when ?back= is set.
  const backNoteQuery = createQuery(() => ({
    queryKey: ["note", backNoteId] as const,
    queryFn: () => api.getNote(backNoteId),
    enabled: backNoteId !== "",
  }));
  const backNote: Note | undefined = $derived(backNoteQuery.data as Note | undefined);
  const backSplit = $derived(splitContent(backNote?.content ?? ""));
  const backIsQueryWidget = $derived(backNote?.metadata.note_type === "Query");
  const backWidget = $derived(backNote && backIsQueryWidget ? widgetFromNote(backNote) : null);
  const backIsDailyJournal = $derived(
    !backBlockId &&
    (
      backNoteId === "dailies" ||
      backNote?.metadata.note_type === "Daily" ||
      (backNote?.metadata.tags ?? []).includes("daily")
    ),
  );
  const backJournalAnchor = $derived(
    /^\d{4}-\d{2}-\d{2}$/.test(backNoteId) ? backNoteId : todayIso(),
  );
  const backDrillBlock = $derived.by((): ParsedBlock | null => {
    if (!backBlockId || !backNote) return null;
    return parseBlocks(backNote.id, backSplit.body).find(b => b.id === backBlockId) ?? null;
  });

  // Drilling within the left pane: source = back pane, target = anything.
  // gotoNote consults active-side automatically; we just route through it.
  function drillIntoBack(blockId: string) {
    if (!backNoteId) return;
    gotoNote(backNoteId, blockId);
  }
  function drillOutBack() {
    // Clear the left's ?backBlock= but preserve everything else.
    const params = new URLSearchParams(page.url.search);
    params.delete("backBlock");
    const qs = params.toString();
    goto(`${page.url.pathname}${qs ? `?${qs}` : ""}`, { replaceState: false, noScroll: true });
  }

  // Auto-open kanban on tag pages in kanban mode (with Vim on). Mutex with
  // column-view: collapse ?back= first so the focus region belongs to kanban.
  $effect(() => {
    if (isTagPage && vimOn && viewMode === "kanban" && !isSplitOpen()) {
      untrack(() => {
        if (vSplitShown) collapseSplit();
        openSplit();
      });
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
  let inFlightController: AbortController | null = null;
  let pendingContent: string | null = null;

  // Phase 9.5b — left/back pane has its own debounced save state to avoid
  // cross-pane PUT collisions when both panes are edited concurrently.
  let backSaveTimer: ReturnType<typeof setTimeout> | null = null;
  let backInFlightController: AbortController | null = null;
  let backPendingContent: string | null = null;

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
    pendingContent = fullContent;
    if (saveTimer) clearTimeout(saveTimer);
    setSaving();
    saveTimer = setTimeout(() => {
      void flushSave();
    }, 500);
  }

  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (pendingContent === null) return;
    const content = pendingContent;
    pendingContent = null;
    // Cancel any in-flight PUT — its result would race ours.
    if (inFlightController) inFlightController.abort();
    const controller = new AbortController();
    inFlightController = controller;
    try {
      const updated = await api.updateNote(noteId, content, controller.signal);
      if (controller.signal.aborted) return;
      queryClient.setQueryData(["note", noteId], updated);
      setSaved();
    } catch (e) {
      // An aborted PUT is expected when undo cancels a debounced save.
      // It must NOT trip the UI into a "save failed" state.
      if ((e as { name?: string })?.name === "AbortError") return;
      const msg = e instanceof Error ? e.message : "Unknown error";
      setSaveError(msg);
      console.error("Save failed:", e);
    } finally {
      if (inFlightController === controller) inFlightController = null;
    }
  }

  /**
   * Cancel any pending or in-flight PUT and immediately PUT `fullContent`.
   * Called by BlockOutliner from `applySnapshot` so the server's WS echo
   * carries the restored body, not the pre-undo body. No-op if nothing
   * needs to change, but the immediate flush is still valuable because the
   * snapshot's body must reach the server before the next typing burst.
   */
  function cancelAndFlush(fullContent: string) {
    pendingContent = fullContent;
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (inFlightController) {
      inFlightController.abort();
      inFlightController = null;
    }
    void flushSave();
  }

  // ----- Phase 9.5b back-pane (left) save plumbing — mirrors right, scoped to backNoteId. -----

  function handleBackContentChange(fullContent: string) {
    backPendingContent = fullContent;
    if (backSaveTimer) clearTimeout(backSaveTimer);
    setSaving();
    backSaveTimer = setTimeout(() => { void flushBackSave(); }, 500);
  }

  async function flushBackSave() {
    if (backSaveTimer) { clearTimeout(backSaveTimer); backSaveTimer = null; }
    if (backPendingContent === null) return;
    if (!backNoteId) return;
    const content = backPendingContent;
    backPendingContent = null;
    if (backInFlightController) backInFlightController.abort();
    const controller = new AbortController();
    backInFlightController = controller;
    try {
      const updated = await api.updateNote(backNoteId, content, controller.signal);
      if (controller.signal.aborted) return;
      queryClient.setQueryData(["note", backNoteId], updated);
      setSaved();
    } catch (e) {
      if ((e as { name?: string })?.name === "AbortError") return;
      const msg = e instanceof Error ? e.message : "Unknown error";
      setSaveError(msg);
      console.error("Back-pane save failed:", e);
    } finally {
      if (backInFlightController === controller) backInFlightController = null;
    }
  }

  function cancelAndFlushBack(fullContent: string) {
    backPendingContent = fullContent;
    if (backSaveTimer) { clearTimeout(backSaveTimer); backSaveTimer = null; }
    if (backInFlightController) { backInFlightController.abort(); backInFlightController = null; }
    void flushBackSave();
  }

  onDestroy(() => {
    if (saveTimer) clearTimeout(saveTimer);
    if (inFlightController) inFlightController.abort();
    if (backSaveTimer) clearTimeout(backSaveTimer);
    if (backInFlightController) backInFlightController.abort();
    // Clear the focused-block store so the bottom drawer doesn't keep
    // displaying properties for a block from a now-unmounted page.
    setFocusedBlock(null);
    setLeftFocusedBlock(null);
    setRightFocusedBlock(null);
  });
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="flex-1 flex min-w-0 h-full" style="flex-direction: row;">
  <!-- Phase 9.5b — left (back-context) pane: present when ?back= is in URL.
       vSplitRatio is the LEFT pane's percentage (default 30); the right pane
       gets the remainder. -->
  {#if vSplitShown}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="flex flex-col min-w-0 h-full overflow-y-auto transition-shadow"
      style="flex-basis: {vSplitRatio}%; flex-grow: 1; flex-shrink: 1; {vSplitActiveSide === 'left' ? 'box-shadow: inset 2px 0 0 0 var(--primary);' : ''}"
      onclick={() => setVSplitActiveSide('left')}
    >
      <div class="max-w-3xl mx-auto px-10 pt-10 pb-4 w-full">
        {#if backNote}
          <div class="flex items-center gap-2 text-[12px] text-muted-foreground mb-4">
            {#if backDrillBlock}
              <button onclick={drillOutBack} class="hover:text-primary transition-colors">{backNote.title}</button>
              <span>›</span>
              <span class="text-foreground/70 truncate max-w-[240px]">{backDrillBlock.text}</span>
            {:else}
              <span>{backNote.title}</span>
            {/if}
          </div>
          <h1 class="font-display text-2xl font-semibold tracking-tight leading-tight mb-6">{backNote.title}</h1>
        {:else if backNoteId && backNoteQuery.isLoading}
          <div class="py-8 text-muted-foreground">Loading…</div>
        {:else if backNoteQuery.isError}
          <div class="py-8 text-destructive">Could not load left pane.</div>
        {/if}
      </div>
      <div class="max-w-3xl mx-auto px-10 pb-16 w-full">
        {#if backNote && backIsDailyJournal}
          {#key backJournalAnchor}
            <JournalView anchorDate={backJournalAnchor} />
          {/key}
        {:else if backNote && backIsQueryWidget && backWidget}
          <QueryWidgetView widget={backWidget} />
        {:else if backNote}
          <BlockOutliner
            noteId={backNote.id}
            body={backSplit.body}
            frontmatter={backSplit.frontmatter}
            onContentChange={handleBackContentChange}
            onCancelAndFlush={cancelAndFlushBack}
            onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
            onfocusedblockchange={(b) => setLeftFocusedBlock(b)}
            drillBlockId={backBlockId}
            onDrillIn={drillIntoBack}
          />
        {/if}
      </div>
    </div>
    <SplitDivider orientation="vertical" onresize={(r: number) => setVSplitRatio(r)} />
  {/if}

  <!-- Phase 9.5b — right (current) pane: the path-driven content. -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="flex flex-col min-w-0 h-full"
    style="flex-basis: {vSplitShown ? `${100 - vSplitRatio}%` : '100%'}; flex-grow: 1; flex-shrink: 1; {vSplitShown && vSplitActiveSide === 'right' ? 'box-shadow: inset 2px 0 0 0 var(--primary);' : ''}"
    onclick={() => { if (vSplitShown) setVSplitActiveSide('right'); }}
  >
    <!-- Note header + outliner + tag config + (inline kanban/table when not split) -->
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
        {#if isDailyJournal}
          {#key journalAnchor}
            <JournalView anchorDate={journalAnchor} />
          {/key}
        {:else if isQueryWidget && widget}
          <QueryWidgetView {widget} />
        {:else if isDocumentMode}
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
            onCancelAndFlush={cancelAndFlush}
            onleader={() => document.dispatchEvent(new CustomEvent("tesela:leader"))}
            onfocusedblockchange={(b) => setRightFocusedBlock(b)}
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

    <!-- Kanban split divider + bottom pane (existing 9.0 behavior) -->
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
  </div>  <!-- /right (current) pane -->
</div>
