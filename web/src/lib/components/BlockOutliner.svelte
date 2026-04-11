<script lang="ts">
  import { parseBlocks } from "$lib/block-parser";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
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
  <div class="space-y-0.5">
    {#each blocks as block, index (block.id)}
      <div
        class="group flex items-start gap-1.5 rounded-sm transition-colors {focusedIndex === index ? 'bg-accent/10' : ''}"
        style="padding-left: {block.indent_level * 24}px"
      >
        <span class="mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-muted-foreground/40"></span>
        <div class="flex-1 min-w-0 py-0.5">
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
            startininsert={focusedIndex === index && block.raw_text === ""}
            onleader={onLeader}
            focused={focusedIndex === index}
          />
        </div>
      </div>
    {/each}
  </div>
{/if}
