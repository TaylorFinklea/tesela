<script lang="ts">
  /*
   * Prism v4 context-pane tab — the block outline of the followed note.
   * v4-native. Parses the note body into blocks and renders them
   * indented by `indent_level`. Clicking a row opens the note (drilling
   * to the specific block is a Phase 5 routing refinement).
   */
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { parseBlocks } from "$lib/block-parser";
  import type { Note } from "$lib/types/Note";

  let {
    noteId,
    onOpenNote,
  }: {
    noteId: string | undefined;
    onOpenNote: (noteId: string) => void;
  } = $props();

  const noteQuery = createQuery(() => ({
    queryKey: ["note", noteId] as const,
    queryFn: () => api.getNote(noteId as string),
    enabled: !!noteId,
  }));

  function bodyOf(note: Note): string {
    const c = note.content;
    if (!c.startsWith("---")) return c;
    const end = c.indexOf("---", 3);
    if (end === -1) return c;
    const after = c.slice(end + 3);
    return after.startsWith("\n") ? after.slice(1) : after;
  }

  // `parseBlocks` keeps the inline `<!-- bid:... -->` marker in `.text`;
  // the editor hides it via cm-decorations but a plain list shouldn't
  // show it.
  function stripBid(text: string): string {
    return text.replace(/\s*<!--\s*bid:[^>]*-->\s*/g, " ").trim();
  }

  const blocks = $derived.by(() => {
    const note = noteQuery.data as Note | undefined;
    if (!note) return [];
    return parseBlocks(note.id, bodyOf(note)).map((b) => ({
      ...b,
      text: stripBid(b.text),
    }));
  });
</script>

{#if !noteId}
  <p class="v4-ctx-empty">no note focused</p>
{:else if noteQuery.isLoading}
  <p class="v4-ctx-empty">loading…</p>
{:else if blocks.length === 0}
  <p class="v4-ctx-empty">empty note</p>
{:else}
  <ul class="v4-ctx-list">
    {#each blocks as block (block.id)}
      <li>
        <button
          type="button"
          class="v4-ctx-row v4-ctx-outline-row"
          style="padding-left: {6 + block.indent_level * 14}px"
          onclick={() => onOpenNote(block.note_id)}
          title={block.text}
        >
          <span class="v4-ctx-bullet">·</span>
          {block.text || "—"}
        </button>
      </li>
    {/each}
  </ul>
{/if}
