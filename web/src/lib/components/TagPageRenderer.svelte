<script lang="ts">
  /* Composite page renderer for `type: tag` pages (tag-system Phase 9).
   *
   * Top: editable description (block outliner). Auto-expanded when the
   * page has body content, collapsed by default when empty. The toggle
   * keybind to force collapse/expand is `Alt-T` on the surrounding
   * BufferShell. (Pending Phase 9 follow-up — see project memory.)
   *
   * Bottom: embedded `instances-of-tag` derived view with the current tag
   * as Reference. Always visible. Uses the same component as a stand-
   * alone derived buffer — the page renderer is just another host for
   * the derived registry, per the v5 spec.
   */
  import type { Note } from "$lib/types/Note";
  import BlockOutliner from "$lib/components/BlockOutliner.svelte";
  import InstancesOfTag from "$lib/renderers/derived/instances-of-tag.svelte";
  import TagPropertyConfig from "./TagPropertyConfig.svelte";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  let {
    note,
    paneId,
    onContentChange,
    onCancelAndFlush,
    onfocusedblockchange,
    onLeader,
  }: {
    note: Note;
    paneId?: string;
    onContentChange?: (text: string) => void;
    onCancelAndFlush?: (fullContent: string) => void;
    onfocusedblockchange?: (block: import("$lib/types/ParsedBlock").ParsedBlock | null) => void;
    onLeader?: () => void;
  } = $props();

  /** Frontmatter + body split. Same logic as NoteRenderer's `splitContent`. */
  function splitContent(content: string): { frontmatter: string; body: string } {
    if (!content.startsWith("---\n") && !content.startsWith("---\r\n")) {
      return { frontmatter: "", body: content };
    }
    const fmDelim = "---";
    const lines = content.split("\n");
    let endIdx = -1;
    for (let i = 1; i < lines.length; i++) {
      if (lines[i].trim() === fmDelim) {
        endIdx = i;
        break;
      }
    }
    if (endIdx < 0) return { frontmatter: "", body: content };
    const frontmatter = lines.slice(0, endIdx + 1).join("\n") + "\n";
    const body = lines.slice(endIdx + 1).join("\n").replace(/^\n+/, "");
    return { frontmatter, body };
  }

  const split = $derived(splitContent(note.content));

  /** The tag's display name. Falls back to the slug when no title:
   *  frontmatter field is present. Lowercased for the Reference value. */
  const tagValue = $derived((note.metadata.title ?? note.id).toLowerCase());

  /** Pure data-flow synthetic Reference for the embedded derived render. */
  const tagReference = $derived({ kind: "tag" as const, value: tagValue });

  /** Embedded renderers get a `size` prop in their derived contract; the
   *  composite host shrinks the bottom section to a fixed-ish height and
   *  passes a size that triggers the default cascade. Wide enough to keep
   *  the table from collapsing into the compact mode (if/when added). */
  const embeddedSize = { cols: 200, rows: 40 };

  function handleEmbeddedNavigate(
    i: import("$lib/buffer/protocol").NavigationIntent,
  ) {
    if (i.kind === "open-page") {
      openPageInFocused(asPageId(i.path));
    } else if (i.kind === "open-tag") {
      openPageInFocused(asPageId(i.value.toLowerCase()));
    }
  }
</script>

<div class="tag-page">
  <section class="tag-page-description">
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
  </section>

  <div class="tag-page-divider"></div>

  <section class="tag-page-instances">
    <InstancesOfTag
      reference={tagReference}
      size={embeddedSize}
      onNavigate={handleEmbeddedNavigate}
    />
  </section>

  <div class="tag-page-divider"></div>

  <section class="tag-page-properties">
    <TagPropertyConfig tagName={tagValue} noteId={note.id} />
  </section>
</div>

<style>
  .tag-page {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .tag-page-divider {
    height: 1px;
    background: var(--v4-hair);
    margin: 4px 0;
  }
  .tag-page-instances {
    padding: 6px 0 16px 0;
  }
  .tag-page-properties {
    padding: 6px 0 16px 0;
  }
</style>
