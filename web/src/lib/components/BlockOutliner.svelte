<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { parseBlocks } from "$lib/block-parser";
  import { api } from "$lib/api-client";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { Note } from "$lib/types/Note";
  import BlockEditor from "./BlockEditor.svelte";

  let {
    noteId,
    body,
    frontmatter,
    onContentChange,
    onleader: onLeader,
  }: {
    noteId: string;
    body: string;
    frontmatter: string;
    onContentChange?: (fullContent: string) => void;
    onleader?: () => void;
  } = $props();

  // Fetch notes list for autocomplete
  const notesForAutocomplete = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
  }));
  const notesList = $derived(
    ((notesForAutocomplete.data ?? []) as Note[]).map((n) => ({
      id: n.id,
      title: n.title,
      tags: n.metadata.tags,
    })),
  );

  let blocks = $state<ParsedBlock[]>(parseBlocks(noteId, body));
  let focusedIndex = $state<number | null>(null);
  let lastBodyFromServer = $state(body);

  $effect(() => {
    if (body !== lastBodyFromServer) {
      lastBodyFromServer = body;
      if (focusedIndex === null) {
        blocks = parseBlocks(noteId, body);
      }
    }
  });

  function saveBlocks(updated: ParsedBlock[]) {
    const bodyLines = updated
      .map((b) => {
        const indent = "  ".repeat(b.indent_level);
        const lines = b.raw_text.split("\n");
        const first = `${indent}- ${lines[0]}`;
        const rest = lines.slice(1).map((l: string) => `${indent}  ${l}`);
        return [first, ...rest].join("\n");
      })
      .join("\n");
    onContentChange?.(`${frontmatter}${bodyLines}\n`);
  }

  function handleBlockChange(blockId: string, newRawText: string) {
    blocks = blocks.map((b) =>
      b.id === blockId
        ? { ...b, raw_text: newRawText, text: (newRawText.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim() }
        : b,
    );
    saveBlocks(blocks);
  }

  function handleNavigate(direction: "up" | "down") {
    if (focusedIndex === null) return;
    const next = direction === "up"
      ? Math.max(0, focusedIndex - 1)
      : Math.min(blocks.length - 1, focusedIndex + 1);
    focusedIndex = next;
  }

  function handleEnter(atIndex: number) {
    const current = blocks[atIndex];
    if (!current) return;
    const newBlock: ParsedBlock = {
      id: `${noteId}:new-${Date.now()}`,
      text: "",
      raw_text: "",
      tags: [],
      properties: {},
      indent_level: current.indent_level,
      note_id: noteId,
    };
    blocks = [...blocks.slice(0, atIndex + 1), newBlock, ...blocks.slice(atIndex + 1)];
    saveBlocks(blocks);
    focusedIndex = atIndex + 1;
  }

  function handleIndent(atIndex: number, direction: "indent" | "outdent") {
    const block = blocks[atIndex];
    if (!block) return;
    const newLevel = direction === "indent" ? block.indent_level + 1 : Math.max(0, block.indent_level - 1);
    if (newLevel === block.indent_level) return;
    blocks = blocks.map((b, i) => (i === atIndex ? { ...b, indent_level: newLevel } : b));
    saveBlocks(blocks);
  }

  function handleBackspace(atIndex: number) {
    const block = blocks[atIndex];
    if (!block || block.raw_text !== "" || blocks.length <= 1) return;
    blocks = blocks.filter((_, i) => i !== atIndex);
    saveBlocks(blocks);
    if (focusedIndex !== null && focusedIndex > 0) focusedIndex = focusedIndex - 1;
  }

  // Merge the current block's text into the previous block.
  // Triggered when Backspace is pressed at cursor position 0 of a non-empty block.
  function handleBackspaceMerge(atIndex: number, currentText: string) {
    if (atIndex === 0) return; // no previous block to merge into
    const prev = blocks[atIndex - 1];
    if (!prev) return;
    const mergePos = prev.raw_text.length;
    const mergedText = prev.raw_text + currentText;
    // New id forces BlockEditor to remount with the merged text + cursor position
    const mergedBlock: ParsedBlock = {
      ...prev,
      id: `${noteId}:merged-${Date.now()}`,
      raw_text: mergedText,
      text: (mergedText.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
    };
    mergeCursorHint = { blockId: mergedBlock.id, pos: mergePos };
    blocks = [
      ...blocks.slice(0, atIndex - 1),
      mergedBlock,
      ...blocks.slice(atIndex + 1),
    ];
    saveBlocks(blocks);
    focusedIndex = atIndex - 1;
  }

  // Pending initial-cursor hint for the next block to mount (cleared after one read)
  let mergeCursorHint = $state<{ blockId: string; pos: number } | null>(null);

  // Block clipboard for yy/p
  let blockClipboard = $state<ParsedBlock | null>(null);

  function handleDeleteBlock(atIndex: number) {
    if (blocks.length <= 1) return;
    const prev = Math.max(0, atIndex - 1);
    blocks = blocks.filter((_, i) => i !== atIndex);
    saveBlocks(blocks);
    focusedIndex = Math.min(prev, blocks.length - 1);
  }

  function handleYankBlock(atIndex: number) {
    const block = blocks[atIndex];
    if (block) blockClipboard = { ...block };
  }

  function handlePasteBlock(atIndex: number) {
    if (!blockClipboard) return;
    const pasted: ParsedBlock = {
      ...blockClipboard,
      id: `${noteId}:paste-${Date.now()}`,
    };
    blocks = [...blocks.slice(0, atIndex + 1), pasted, ...blocks.slice(atIndex + 1)];
    saveBlocks(blocks);
    focusedIndex = atIndex + 1;
  }

  function handleNewBlockAbove(atIndex: number) {
    const current = blocks[atIndex];
    if (!current) return;
    const newBlock: ParsedBlock = {
      id: `${noteId}:new-${Date.now()}`,
      text: "",
      raw_text: "",
      tags: [],
      properties: {},
      indent_level: current.indent_level,
      note_id: noteId,
    };
    blocks = [...blocks.slice(0, atIndex), newBlock, ...blocks.slice(atIndex)];
    saveBlocks(blocks);
    focusedIndex = atIndex;
  }
</script>

{#if blocks.length === 0}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="text-sm text-muted-foreground cursor-text py-2 hover:bg-accent/20 rounded px-2"
    onclick={() => {
      const newBlock = {
        id: `${noteId}:new-${Date.now()}`,
        text: "",
        raw_text: "",
        tags: [],
        properties: {},
        indent_level: 0,
        note_id: noteId,
      };
      blocks = [newBlock];
      focusedIndex = 0;
    }}
  >
    Click to start writing…
  </div>
{:else}
  <div class="space-y-0">
    {#each blocks as block, index (block.id)}
      <div
        class="group flex items-start transition-all relative {focusedIndex === index ? 'bg-accent/40' : ''}"
        style="padding-left: {block.indent_level * 24}px;"
      >
        <!-- Threading lines -->
        {#if block.indent_level > 0}
          {#each { length: block.indent_level } as _, lvl}
            <span
              class="absolute top-0 bottom-0 w-px"
              style="left: {lvl * 24 + 10}px; background: var(--thread-border);"
            ></span>
          {/each}
        {/if}

        <!-- Bullet -->
        <div class="shrink-0 pt-[12px] pl-2 pr-1.5">
          <span
            class="block w-[5px] h-[5px] rounded-full transition-colors {focusedIndex === index ? 'bg-primary' : 'bg-muted-foreground/30'}"
          ></span>
        </div>

        <!-- Content -->
        <div class="flex-1 min-w-0 py-1 pr-3">
          <BlockEditor
            initialText={block.raw_text}
            onblur={() => { if (focusedIndex === index) focusedIndex = null; }}
            onfocus={() => { focusedIndex = index; }}
            onchange={(text) => handleBlockChange(block.id, text)}
            onnavigate={handleNavigate}
            onescape={() => { focusedIndex = null; }}
            onenter={() => handleEnter(index)}
            onindent={(dir) => handleIndent(index, dir)}
            onbackspaceempty={() => handleBackspace(index)}
            onbackspacemerge={(text: string) => handleBackspaceMerge(index, text)}
            initialCursorPos={mergeCursorHint?.blockId === block.id ? mergeCursorHint.pos : undefined}
            startininsert={focusedIndex === index && block.raw_text === ""}
            onleader={onLeader}
            ondeleteblock={() => handleDeleteBlock(index)}
            onyankblock={() => handleYankBlock(index)}
            onpasteblock={() => handlePasteBlock(index)}
            onnewblockbelow={() => handleEnter(index)}
            onnewblockabove={() => handleNewBlockAbove(index)}
            focused={focusedIndex === index}
            noteslist={notesList}
          />
        </div>
      </div>
    {/each}
  </div>
{/if}
