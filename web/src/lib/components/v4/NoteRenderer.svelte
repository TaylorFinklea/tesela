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
  import TagTable from "$lib/components/TagTable.svelte";
  import PropertyTypeConfig from "$lib/components/PropertyTypeConfig.svelte";

  let {
    note,
    paneId,
    onContentChange,
    onCancelAndFlush,
    onfocusedblockchange,
    onOpenNote,
  }: {
    note: Note;
    paneId: string;
    onContentChange: (fullContent: string) => void;
    onCancelAndFlush?: (fullContent: string) => void;
    onfocusedblockchange?: (block: ParsedBlock | null) => void;
    /** Row activation inside a Query-note widget view. */
    onOpenNote?: (noteId: string) => void;
  } = $props();

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
  const isDocumentMode = $derived(note.metadata.custom?.mode === "document");
</script>

{#if noteType === "Query"}
  <QueryWidgetView
    widget={widgetFromNote(note)}
    onOpenRow={onOpenNote ? (pageId) => onOpenNote(pageId) : undefined}
  />
{:else if noteType === "Tag"}
  <TagTable tagName={note.title} noteId={note.id} />
{:else if noteType === "Property"}
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
  />
{/if}
