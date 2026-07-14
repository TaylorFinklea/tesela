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
  import { resolveChipIcon } from "$lib/icon-registry";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";

  let {
    note,
    paneId,
    onContentChange,
    onCancelAndFlush,
    onPrepareRelocation,
    onfocusedblockchange,
    onLeader,
  }: {
    note: Note;
    paneId?: string;
    onContentChange?: (text: string) => void;
    onCancelAndFlush?: (fullContent: string) => void;
    onPrepareRelocation?: () => Promise<void>;
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

  /** Type display name (title-cased frontmatter title, slug fallback). */
  const typeName = $derived(note.metadata.title ?? note.id);

  /** Plural display name (`plural:` frontmatter); falls back to the type name.
   *  Used in the tag-page header where the type is labelled in the plural. */
  const typePlural = $derived(
    typeof note.metadata.custom.plural === "string" && note.metadata.custom.plural.trim()
      ? (note.metadata.custom.plural as string)
      : typeName,
  );

  /** Resolved type icon — a bare Tabler name (e.g. `checkbox`) → component, or
   *  an emoji/raw string fallback. Mirrors the chip-icon resolution path. */
  const typeIcon = $derived(
    resolveChipIcon(
      typeof note.metadata.custom.icon === "string" ? (note.metadata.custom.icon as string) : null,
    ),
  );

  /** Pure data-flow synthetic Reference for the embedded derived render. */
  const tagReference = $derived({ kind: "tag" as const, value: tagValue });

  /** Instance count for the plural header ("N Tasks"). Sums page-level
   *  (frontmatter-tagged notes) + block-level (inline / `tags::`) instances —
   *  the same two queries InstancesOfTag uses below. */
  // Raised 500→5000 (tesela-sclr.1): a heavily-used tag (e.g. "daily") can
  // exceed 500 instances, silently undercounting the header past that point.
  const headerPagesQuery = createQuery(() => ({
    queryKey: ["notes", { tag: tagValue, limit: 5000 }] as const,
    queryFn: () => api.listNotes({ tag: tagValue, limit: 5000 }),
    enabled: !!tagValue,
  }));
  const headerBlocksQuery = createQuery(() => ({
    queryKey: ["typed-blocks", tagValue] as const,
    queryFn: () => api.getTypedBlocks(tagValue),
    enabled: !!tagValue,
  }));
  const instanceCount = $derived(
    ((headerPagesQuery.data ?? []) as Note[]).length +
      ((headerBlocksQuery.data ?? []) as ParsedBlock[]).length,
  );

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
  <header class="tag-page-header">
    {#if typeIcon.component}
      {@const Cmp = typeIcon.component as import("svelte").Component<{ size?: number; stroke?: number }>}
      <Cmp size={18} stroke={1.75} />
    {:else if typeIcon.emoji}
      <span class="tag-page-emoji">{typeIcon.emoji}</span>
    {/if}
    <span class="tag-page-plural">{instanceCount} {typePlural}</span>
  </header>

  <section class="tag-page-description">
    <BlockOutliner
      noteId={note.id}
      body={split.body}
      frontmatter={split.frontmatter}
      {paneId}
      {onContentChange}
      {onCancelAndFlush}
      {onPrepareRelocation}
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
  .tag-page-header {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--fg-default);
  }
  .tag-page-plural {
    font-size: 15px;
    font-weight: 600;
  }
  .tag-page-emoji {
    font-size: 16px;
    line-height: 1;
  }
  .tag-page-divider {
    height: 1px;
    background: var(--line-soft);
    margin: 4px 0;
  }
  .tag-page-instances {
    padding: 6px 0 16px 0;
  }
  .tag-page-properties {
    padding: 6px 0 16px 0;
  }
</style>
