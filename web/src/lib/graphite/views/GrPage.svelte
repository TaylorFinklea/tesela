<!-- web/src/lib/graphite/views/GrPage.svelte — Part A, Task A3 + Gate B
     note_type dispatch.
     Page / project outliner. REUSES BlockOutliner (the CodeMirror editing
     engine) untouched, fetching + saving the note via the same TanStack
     pattern BufferShell uses (createQuery + 500ms-debounced api.updateNote
     with optimistic setQueryData). The Graphite block look comes from A1's
     variable remap + decoration overrides (graphite-editor.css).

     Typed pages dispatch like v5's NoteRenderer (which stays in the
     deletion target, so the switch is mirrored here instead of imported):
     Query → QueryWidgetView / CompactQueryView (narrow panes), tag →
     TagPageRenderer (description outliner + instances-of-tag), property →
     PropertyTypeConfig, `mode: document` → DocumentEditor, everything else
     → BlockOutliner. PageTagsChips renders above body-text pages. Dailies
     never reach this view (GrLeaf routes them to GrDaily). All leaf
     components are REUSED READ-ONLY; a token shim on `.gr-outline` maps
     their v4-/v9-era CSS variables onto Graphite tokens.

     Layout: a focus pane (title + GrTypeTag + meta head, `.gr-outline`
     body hosting the dispatched view) beside a side pane of linked
     references (`.gr-refcard`s from api.getBacklinks) + a page-properties
     list (`.gr-proplist`). Query/property pages hide the side pane — the
     widget table/kanban and the config form take the full width.
     BlockOutliner, the API, and the buffer store are imported READ-ONLY. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import {
    saveAdmissionRegistry,
    type SaveAdmissionLease,
  } from "$lib/block-ops-saver";
  import type { Note } from "$lib/types/Note";
  import type { Link } from "$lib/types/Link";
  import type { RelationBacklink } from "$lib/types/RelationBacklink";
  import type { PageDirectoryEntry } from "$lib/node-relations";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  import { setSaving, setSaved, setSaveError } from "$lib/stores/save-state.svelte";
  import { pendingContentJump } from "$lib/stores/content-jump.svelte";
  import { isFavorite, toggleFavorite } from "$lib/stores/favorites.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { widgetFromNote } from "$lib/widget-registry.svelte";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import DocumentEditor from "$lib/components/DocumentEditor.svelte";
  import PropertyTypeConfig from "$lib/components/PropertyTypeConfig.svelte";
  import QueryWidgetView from "$lib/components/QueryWidgetView.svelte";
  import CompactQueryView from "$lib/components/CompactQueryView.svelte";
  import TagPageRenderer from "$lib/components/TagPageRenderer.svelte";
  import PageTagsChips from "$lib/components/PageTagsChips.svelte";
  import GrTypeTag from "$lib/graphite/GrTypeTag.svelte";
  import GrIcon from "$lib/graphite/GrIcon.svelte";
  import PropertyEditor from "$lib/components/PropertyEditor.svelte";
  import { buildRegistry } from "$lib/property-registry";

  let { pageId, paneId }: { pageId: string; paneId?: string } = $props();

  const queryClient = useQueryClient();

  // ── note fetch (mirrors BufferShell) ──────────────────────────────────
  const noteQuery = createQuery(() => ({
    queryKey: ["note", pageId] as const,
    queryFn: () => api.getNote(pageId),
    enabled: !!pageId,
  }));
  const note = $derived(noteQuery.data as Note | undefined);
  const contentJump = $derived(pendingContentJump(pageId));

  // ── backlinks for the linked-refs side pane ───────────────────────────
  const backlinksQuery = createQuery(() => ({
    queryKey: ["backlinks", pageId] as const,
    queryFn: () => api.getBacklinks(pageId),
    enabled: !!pageId,
  }));
  const backlinks = $derived((backlinksQuery.data ?? []) as Link[]);
  const pageDirectoryQuery = createQuery(() => ({
    queryKey: ["page-directory"] as const,
    queryFn: () => api.getPageDirectory(),
  }));
  const pageDirectory = $derived((pageDirectoryQuery.data ?? []) as PageDirectoryEntry[]);
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  const propertyRegistry = $derived(buildRegistry((allNotesQuery.data ?? []) as Note[]));
  let editingPageProperty = $state<{ key: string; value: string; x: number; y: number } | null>(null);
  const currentPageIdentity = $derived(
    pageDirectory.find((entry) => entry.slug === pageId && !entry.deleted && !entry.conflict)?.page_id ?? null,
  );
  const relationBacklinksQuery = createQuery(() => ({
    queryKey: ["relation-backlinks", currentPageIdentity] as const,
    queryFn: () => api.getRelationBacklinks(currentPageIdentity as string),
    enabled: currentPageIdentity !== null,
  }));
  const relationBacklinks = $derived((relationBacklinksQuery.data ?? []) as RelationBacklink[]);

  // ── frontmatter / body split (mirrors NoteRenderer.splitContent) ───────
  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---")) return { frontmatter: "", body: content };
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return { frontmatter: "", body: content };
    const fmEnd = endIdx + 3;
    const afterFm = content.slice(fmEnd);
    const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
    return {
      frontmatter: content.slice(0, fmEnd) + "\n",
      body: afterFm.slice(bodyStart),
    };
  }
  const split = $derived(note ? splitContent(note.content) : { frontmatter: "", body: "" });

  const noteType = $derived((note?.metadata.note_type ?? "note").toLowerCase());
  const tags = $derived(note?.metadata.tags ?? []);
  const pageFavorite = $derived(isFavorite(pageId));

  function togglePageFavorite() {
    if (pageId) toggleFavorite(pageId);
  }

  // ── note_type dispatch (mirror of NoteRenderer, Gate B) ────────────────
  const isDocumentMode = $derived(note?.metadata.custom?.mode === "document");
  /** Tag-chip strip above body-text pages. Hidden for query/property pages
   *  (they manage their own frontmatter UI) — same rule as NoteRenderer. */
  const showTagChips = $derived(noteType !== "query" && noteType !== "property");
  /** Query/property pages take the full leaf width (widget table/kanban,
   *  config form); everything else keeps the References side pane. */
  const showSidePane = $derived(noteType !== "query" && noteType !== "property");

  /** Compact cascade for Query notes in narrow panes. NoteRenderer flips at
   *  50 cell-unit cols; BufferShell's cell is CHAR_WIDTH=7px → 350px. GrPage
   *  has no size prop, so it measures its own body width instead. */
  const QUERY_FULL_MIN_PX = 350;
  let outlineWidth = $state(0);
  const useCompactQuery = $derived(outlineWidth > 0 && outlineWidth < QUERY_FULL_MIN_PX);

  // Page-level properties for the side `.gr-proplist`. Pulls the flat
  // string fields from frontmatter `custom` (the freeform property bag).
  const pageProps = $derived.by<Array<{ k: string; v: string }>>(() => {
    const custom = note?.metadata.custom ?? {};
    const out: Array<{ k: string; v: string }> = [];
    for (const [k, raw] of Object.entries(custom)) {
      if (raw == null) continue;
      const v = typeof raw === "string" ? raw : JSON.stringify(raw);
      out.push({ k, v });
    }
    return out;
  });

  // ── debounced save (mirrors BufferShell) ───────────────────────────────
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let inFlight: AbortController | null = null;
  let inFlightPromise: Promise<void> | null = null;
  let saveFailed = false;
  let saveFailure: unknown;
  let pending: string | null = null;
  let saveAdmission: SaveAdmissionLease | null = null;
  // Edit BASE for the pending save (body the outliner last reseeded from),
  // sent as `base_content` so the server diffs the author's real changes and
  // never re-asserts an untouched block over a concurrent peer edit. First
  // base of the window wins; cleared on flush.
  let pendingBase: string | undefined = undefined;

  function ensureSaveAdmission(): void {
    if (saveAdmission || !pageId) return;
    saveAdmission = saveAdmissionRegistry.admit(pageId, settleSave);
  }

  function releaseSaveAdmissionIfQuiet(): void {
    if (
      saveFailed
      || saveTimer !== null
      || pending !== null
      || inFlightPromise !== null
    ) return;
    const admission = saveAdmission;
    saveAdmission = null;
    admission?.release();
  }

  function handleContentChange(fullContent: string, baseContent?: string) {
    ensureSaveAdmission();
    pending = fullContent;
    if (pendingBase === undefined) pendingBase = baseContent;
    setSaving();
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void flushSave(), 500);
  }

  function flushSave(): Promise<void> {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (pending === null || !pageId) return inFlightPromise ?? Promise.resolve();
    if (inFlightPromise) {
      const predecessor = inFlightPromise;
      return predecessor.then(
        () => flushSave(),
        async (error) => {
          await flushSave();
          throw error;
        },
      );
    }
    const content = pending;
    pending = null;
    const base = pendingBase;
    pendingBase = undefined;
    const controller = new AbortController();
    inFlight = controller;
    if (note) queryClient.setQueryData(["note", pageId], { ...note, content });
    const completion = (async () => {
      try {
        const updated = await api.updateNote(pageId, content, base, controller.signal);
        if (controller.signal.aborted) return;
        queryClient.setQueryData(["note", pageId], updated);
        setSaved();
      } catch (e) {
        if ((e as { name?: string })?.name === "AbortError") return;
        if (!saveFailed) {
          saveFailed = true;
          saveFailure = e;
        }
        // Surface the failure instead of swallowing it. Keep the optimistic
        // cache (the user's only live copy of the unsaved edit) — do NOT roll
        // back, which would feed pre-edit content into the live editor and
        // destroy in-progress work. Mirrors BufferShell's setSaveError path.
        console.error("GrPage save failed:", e);
        setSaveError(e instanceof Error ? e.message : "Unknown error");
        toast("Failed to save page", "error");
        throw e;
      } finally {
        if (inFlight === controller) {
          inFlight = null;
          inFlightPromise = null;
        }
        releaseSaveAdmissionIfQuiet();
      }
    })();
    inFlightPromise = completion;
    void completion.catch(() => {});
    return completion;
  }

  async function settleSave(): Promise<void> {
    let failed = false;
    let firstFailure: unknown;
    try {
      while (true) {
        if (inFlightPromise) {
          try {
            await inFlightPromise;
          } catch (error) {
            if (!failed) firstFailure = error;
            failed = true;
          }
          continue;
        }
        if (pending === null) {
          if (saveFailed) throw saveFailure;
          if (failed) throw firstFailure;
          return;
        }
        try {
          await flushSave();
        } catch (error) {
          if (!failed) firstFailure = error;
          failed = true;
        }
      }
    } finally {
      releaseSaveAdmissionIfQuiet();
    }
  }

  async function cancelAndFlush(fullContent: string, baseContent?: string) {
    ensureSaveAdmission();
    pending = fullContent;
    if (baseContent !== undefined) pendingBase = baseContent;
    await settleSave();
  }

  onDestroy(() => {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    const completion = settleSave();
    void completion.catch((error) => console.error("GrPage teardown save failed:", error));
  });

  function openRef(target: string) {
    openPageInFocused(asPageId(target));
  }

  function openPagePropertyEditor(event: MouseEvent, key: string, value: string) {
    const definition = propertyRegistry.get(key.toLowerCase());
    if (definition?.value_type !== "node") return;
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    editingPageProperty = { key, value, x: rect.left, y: rect.bottom + 2 };
  }

  async function savePageProperty(value: string) {
    const editing = editingPageProperty;
    if (!editing) return;
    await api.setPageProperty(pageId, editing.key, value || null);
    editingPageProperty = null;
    await queryClient.invalidateQueries({ queryKey: ["note", pageId] });
    await queryClient.invalidateQueries({ queryKey: ["relation-backlinks"] });
  }
</script>

<div class="gr-pane focus">
  <div class="gr-pane-head">
    <span class="ttl">{note?.title || pageId || "Untitled page"}</span>
    <GrTypeTag type={noteType === "tag" ? "person" : "project"}>{noteType}</GrTypeTag>
    <button
      type="button"
      class="gr-favorite"
      class:active={pageFavorite}
      aria-pressed={pageFavorite}
      aria-label={pageFavorite ? "Remove page from favorites" : "Add page to favorites"}
      title={pageFavorite ? "Remove from favorites" : "Add to favorites"}
      onclick={togglePageFavorite}
    >
      <GrIcon name="star" size={14} />
      <span>{pageFavorite ? "Favorited" : "Favorite"}</span>
    </button>
    <span class="sp"></span>
    {#if note}
      <span class="meta">{tags.length ? tags.map((t) => `#${t}`).join(" ") : ""}</span>
    {/if}
  </div>

  <div class="gr-outline" bind:clientWidth={outlineWidth}>
    {#if noteQuery.isLoading}
      <div class="gr-empty">loading…</div>
    {:else if noteQuery.isError}
      <div class="gr-empty">could not load {pageId}</div>
    {:else if note}
      {#key pageId}
        {#if showTagChips}
          <PageTagsChips {note} onContentChange={handleContentChange} />
        {/if}
        {#if noteType === "query"}
          {#if useCompactQuery}
            <CompactQueryView {note} onOpenRow={(id) => openRef(id)} />
          {:else}
            <QueryWidgetView
              widget={widgetFromNote(note)}
              onOpenRow={(rowPageId) => openRef(rowPageId)}
            />
          {/if}
        {:else if noteType === "tag"}
          <TagPageRenderer
            {note}
            {paneId}
            onContentChange={handleContentChange}
            onCancelAndFlush={cancelAndFlush}
            onPrepareRelocation={settleSave}
          />
        {:else if noteType === "property"}
          <PropertyTypeConfig {note} />
        {:else if isDocumentMode}
          <DocumentEditor
            body={split.body}
            frontmatter={split.frontmatter}
            onContentChange={handleContentChange}
          />
        {:else}
          <BlockOutliner
            noteId={note.id}
            body={split.body}
            frontmatter={split.frontmatter}
            {paneId}
            onContentChange={handleContentChange}
            onCancelAndFlush={cancelAndFlush}
            onPrepareRelocation={settleSave}
            contentJump={contentJump}
          />
        {/if}
      {/key}
    {/if}
  </div>
</div>

{#if showSidePane}
  <div class="gr-pane side">
    <div class="gr-pane-head">
      <span class="ttl side-ttl">References</span>
      <span class="sp"></span>
      <span class="meta">{backlinks.length + relationBacklinks.length}</span>
    </div>
    <div class="gr-side-body">
      {#if backlinks.length === 0 && relationBacklinks.length === 0}
        <div class="gr-empty">No linked references</div>
      {:else}
        {#each backlinks as ref (ref.target + ":" + ref.position)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="gr-refcard" onclick={() => openRef(ref.target)}>
            <div class="src">
              <GrIcon name="link" size={13} />
              <span>{ref.target}</span>
            </div>
            {#if ref.text}<div class="snip">{ref.text}</div>{/if}
          </div>
        {/each}
      {/if}
      {#if relationBacklinks.length > 0}
        {#each relationBacklinks as relation (relation.edge.source_page_id + ":" + (relation.edge.source_block_id ?? "page") + ":" + relation.edge.property_key)}
          <!-- svelte-ignore a11y_click_events_have_key_events -->
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div class="gr-refcard" onclick={() => openRef(relation.source_slug)}>
            <div class="src">
              <GrIcon name="share-2" size={13} />
              <span>{relation.source_title}</span>
            </div>
            <div class="snip">{relation.edge.property_key}</div>
          </div>
        {/each}
      {/if}

      {#if pageProps.length > 0}
        <div class="gr-proplist">
          <div class="ph">Properties</div>
          {#each pageProps as p (p.k)}
            <button type="button" class="gr-prow" onclick={(event) => openPagePropertyEditor(event, p.k, p.v)}>
              <span class="chord"></span>
              <span class="k">{p.k}</span>
              <span class="v">{p.v}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}

{#if editingPageProperty}
  {@const definition = propertyRegistry.get(editingPageProperty.key.toLowerCase())}
  {#if definition}
    <PropertyEditor
      propertyName={definition.name}
      currentValue={editingPageProperty.value}
      valueType={definition.value_type}
      choices={definition.choices}
      position={{ x: editingPageProperty.x, y: editingPageProperty.y }}
      onselect={(value) => void savePageProperty(value)}
      onclose={() => (editingPageProperty = null)}
    />
  {/if}
{/if}

<style>
  /* Panes mirror GrPane's `.focus` / `.side` flex ratios so GrPage drops
     into the shell's `.gr-main` flex row as a focus+side pair. */
  .gr-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    min-height: 0;
  }
  .gr-pane.focus {
    flex: 1.7;
  }
  .gr-pane.side {
    flex: 1;
    background: var(--surface);
    border-left: 1px solid var(--line);
    max-width: 420px;
  }
  .gr-pane-head {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 14px 18px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .gr-pane-head .ttl {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gr-pane-head .side-ttl {
    font-size: 13px;
    color: var(--fg2);
  }
  .gr-pane-head .sp {
    flex: 1;
  }
  .gr-pane-head .meta {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
    white-space: nowrap;
  }
  .gr-favorite {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    appearance: none;
    border: 1px solid var(--line-2);
    border-radius: 6px;
    padding: 4px 7px;
    background: transparent;
    color: var(--subtle);
    font-family: var(--mono);
    font-size: 10px;
    cursor: pointer;
  }
  .gr-favorite:hover,
  .gr-favorite.active {
    color: var(--coral);
    border-color: var(--coral-line);
    background: var(--coral-dim);
  }
  .gr-outline {
    flex: 1;
    overflow: auto;
    padding: 14px 18px;
    min-height: 0;

    /* Token shim for reused typed-page leaf components that still consume
       `--v9-*` aliases. App role tokens are bridged by graphite/tokens.css;
       these page-scoped v9 aliases keep reused renderers on the Graphite
       palette without leaking into shell chrome. */
    --v9-bg: var(--bg);
    --v9-bg-2: var(--raised);
    --v9-bg-3: var(--raised-2);
    --v9-bg-4: var(--raised-3);
    --v9-ink: var(--fg);
    --v9-rose: var(--task);
    --v9-amber: var(--note);
    --v9-ochre: var(--note);
    --v9-indigo: var(--project);
    --v9-sage: var(--query);
    --v9-teal: var(--event);
    --v9-plum: var(--person);
  }
  .gr-empty {
    color: var(--faint);
    font-family: var(--mono);
    font-size: 12px;
    padding: 8px 2px;
  }

  /* Linked-refs side pane (verbatim Graphite CSS). */
  .gr-side-body {
    flex: 1;
    overflow: auto;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }
  .gr-refcard {
    padding: 10px 12px;
    border-radius: 10px;
    background: var(--raised);
    border: 1px solid var(--line);
    cursor: pointer;
  }
  .gr-refcard .src {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 11px;
    color: var(--fg2);
  }
  .gr-refcard .snip {
    font-size: 12.5px;
    color: var(--muted);
    margin-top: 5px;
    line-height: 1.4;
  }
  .gr-refcard .snip :global(em) {
    background: var(--coral-dim);
    color: var(--coral);
    font-style: normal;
    padding: 0 2px;
    border-radius: 3px;
  }

  /* Properties list (verbatim Graphite CSS). */
  .gr-proplist .ph {
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
    padding: 4px 2px 7px;
  }
  .gr-prow {
    display: grid;
    grid-template-columns: 18px 84px 1fr;
    align-items: center;
    gap: 8px;
    padding: 6px 7px;
    border-radius: 7px;
  }
  .gr-prow:hover {
    background: var(--raised);
  }
  .gr-prow .chord {
    font-family: var(--mono);
    font-size: 9.5px;
    text-align: center;
    color: var(--subtle);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 2px 0;
  }
  .gr-prow .k {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--subtle);
  }
  .gr-prow .v {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--fg2);
  }
</style>
