<script lang="ts">
  /*
   * Prism v4 — picks the right view component for a note based on its
   * type, so an editor pane can show *any* note correctly (a Tag page
   * renders as a table, a Query note as a widget, a document-mode note
   * in the prose editor, everything else as the block outliner).
   *
   * This is the v4 counterpart of the view-mode switch buried in the
   * legacy `routes/p/[id]/+page.svelte`. It deliberately does NOT carry
   * the legacy route's drill / column-split / header concerns — those
   * are URL/chrome state the v4 pane model replaces. The legacy page
   * keeps its own inline switch until Phase 6 deletes that chrome.
   */
  import type { Note } from "$lib/types/Note";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import { widgetFromNote } from "$lib/widget-registry.svelte";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import DocumentEditor from "$lib/components/DocumentEditor.svelte";
  import QueryWidgetView from "$lib/components/QueryWidgetView.svelte";
  import CompactQueryView from "$lib/components/v5/CompactQueryView.svelte";
  import JournalView from "$lib/components/JournalView.svelte";
  import TagTable from "$lib/components/TagTable.svelte";
  import PropertyTypeConfig from "$lib/components/PropertyTypeConfig.svelte";

  let {
    note,
    paneId,
    size,
    onContentChange,
    onCancelAndFlush,
    onfocusedblockchange,
    onOpenNote,
    onLeader,
  }: {
    note: Note;
    paneId: string;
    /** Cell-units size of the hosting buffer, passed through from
     *  BufferShell. Optional — when omitted, renderers pick their full
     *  mode (back-compat with v4 callers). Phase 13a wires v5 BufferShell
     *  to always pass this so cascade modes can fire. */
    size?: { cols: number; rows: number };
    onContentChange: (fullContent: string) => void;
    onCancelAndFlush?: (fullContent: string) => void;
    onfocusedblockchange?: (block: ParsedBlock | null) => void;
    /** Row activation inside a Query-note widget view. */
    onOpenNote?: (noteId: string) => void;
    /** Space-in-NORMAL-mode trigger: opens the v5 leader chord menu. */
    onLeader?: () => void;
  } = $props();

  /** Query notes render as a wide table by default. Below ~50 cols
   *  (cell-units) we drop to the compact list mode. Matches the v5
   *  cascade pattern from Phase 10. */
  const QUERY_FULL_MIN_COLS = 50;
  const useCompactQuery = $derived(
    !!size && size.cols < QUERY_FULL_MIN_COLS,
  );

  /** Daily-typed notes (`tags: [daily]` + title `YYYY-MM-DD`) render as
   *  JournalView (multi-day continuous scroll) when there's even a modest
   *  amount of room. Below the threshold we fall back to a single-day
   *  BlockOutliner. Earlier rev required 28 rows which was a too-strict
   *  ceiling — having a derived pane below routinely pushed the daily
   *  below the threshold. 16 rows ≈ 320px which is "you can read at
   *  least one full day plus the next day's header." */
  const DAILY_JOURNAL_MIN_COLS = 60;
  const DAILY_JOURNAL_MIN_ROWS = 16;
  const isDaily = $derived(
    /^\d{4}-\d{2}-\d{2}$/.test(note.title) &&
      (note.metadata.tags ?? []).includes("daily"),
  );
  const useJournalFeed = $derived(
    isDaily &&
      // Default to journal feed when size is unknown (matches the user's
      // expectation that dailies are journal-shaped by default).
      (!size ||
        (size.cols >= DAILY_JOURNAL_MIN_COLS &&
          size.rows >= DAILY_JOURNAL_MIN_ROWS)),
  );

  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---")) return { frontmatter: "", body: content };
    const endIdx = content.indexOf("---", 3);
    if (endIdx === -1) return { frontmatter: "", body: content };
    const fmEnd = endIdx + 3;
    const afterFm = content.slice(fmEnd);
    const bodyStart = afterFm.startsWith("\n") ? 1 : 0;
    return { frontmatter: content.slice(0, fmEnd) + "\n", body: afterFm.slice(bodyStart) };
  }

  const split = $derived(splitContent(note.content));
  const noteType = $derived(note.metadata.note_type);
  /** Lowercased note type for case-insensitive dispatch. The tag-system spec
   *  uses `type: tag` (lowercase) but earlier auto-creates wrote `type: "Tag"`
   *  capitalized; matching lowercase here keeps both forms rendering correctly
   *  without an on-disk migration sweep. */
  const noteTypeLc = $derived((noteType ?? "").toLowerCase());
  const isDocumentMode = $derived(note.metadata.custom?.mode === "document");
</script>

{#if useJournalFeed}
  <JournalView anchorDate={note.title} />
{:else if noteTypeLc === "query"}
  {#if useCompactQuery}
    <CompactQueryView {note} onOpenRow={onOpenNote} />
  {:else}
    <QueryWidgetView
      widget={widgetFromNote(note)}
      onOpenRow={onOpenNote ? (pageId) => onOpenNote(pageId) : undefined}
    />
  {/if}
{:else if noteTypeLc === "tag"}
  <TagTable tagName={note.title} noteId={note.id} />
{:else if noteTypeLc === "property"}
  <PropertyTypeConfig {note} />
{:else if isDocumentMode}
  <DocumentEditor
    body={split.body}
    frontmatter={split.frontmatter}
    {onContentChange}
  />
{:else}
  <BlockOutliner
    noteId={note.id}
    body={split.body}
    frontmatter={split.frontmatter}
    {paneId}
    {onContentChange}
    {onCancelAndFlush}
    {onfocusedblockchange}
    onleader={onLeader}
  />
{/if}
