<script lang="ts">
  import { parseBlocks, extractWikiLinks } from "$lib/block-parser";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import BlockEditor from "./BlockEditor.svelte";

  let {
    noteId,
    body,
    frontmatter,
    onContentChange,
  }: {
    noteId: string;
    body: string;
    frontmatter: string;
    onContentChange?: (fullContent: string) => void;
  } = $props();

  let blocks = $state<ParsedBlock[]>(parseBlocks(noteId, body));
  let focusedIndex = $state<number | null>(null);
  let lastBodyFromServer = $state(body);

  // Only reset blocks when the body prop changes from an EXTERNAL source
  // (e.g., WebSocket push, navigation). Ignore changes from our own saves.
  $effect(() => {
    if (body !== lastBodyFromServer) {
      lastBodyFromServer = body;
      // Only reset if we're not currently editing (no focused block)
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
    focusedIndex = direction === "up"
      ? Math.max(0, focusedIndex - 1)
      : Math.min(blocks.length - 1, focusedIndex + 1);
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
      // Don't save yet — let the user type first. Save happens on block change.
    }}
  >
    Click to start writing…
  </div>
{:else}
  <div class="space-y-0.5" tabindex="-1">
    {#each blocks as block, index (block.id)}
      <div
        class="group flex items-start gap-1.5 rounded-sm transition-colors {focusedIndex === index ? 'bg-accent/20' : 'hover:bg-accent/30'}"
        style="padding-left: {block.indent_level * 24}px"
      >
        <span class="mt-[7px] h-1.5 w-1.5 shrink-0 rounded-full bg-muted-foreground/50"></span>
        <div class="flex-1 min-w-0 py-0.5">
          {#if focusedIndex === index}
            <BlockEditor
              initialText={block.raw_text}
              onblur={() => { if (focusedIndex === index) focusedIndex = null; }}
              onchange={(text) => handleBlockChange(block.id, text)}
              onnavigate={handleNavigate}
              onescape={() => (focusedIndex = null)}
              onenter={() => handleEnter(index)}
              onindent={(dir) => handleIndent(index, dir)}
              onbackspaceempty={() => handleBackspace(index)}
            />
          {:else}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="text-sm leading-relaxed cursor-text min-h-[24px]" onclick={() => (focusedIndex = index)}>
              {@render blockDisplayText(block)}
              {#if block.tags.length > 0}
                <span class="ml-2 inline-flex gap-1">
                  {#each block.tags as tag}
                    <a
                      href="/p/{encodeURIComponent(tag.toLowerCase())}"
                      class="text-xs px-1.5 py-0.5 rounded bg-accent text-accent-foreground hover:bg-accent/80"
                      onclick={(e) => e.stopPropagation()}
                    >#{tag}</a>
                  {/each}
                </span>
              {/if}
              {#if Object.keys(block.properties).length > 0}
                <div class="mt-0.5 space-y-0">
                  {#each Object.entries(block.properties) as [key, value]}
                    <div class="text-xs text-muted-foreground">
                      <span class="text-muted-foreground/70">{key}::</span>
                      {@render propertyValue(value)}
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

{#snippet blockDisplayText(block: ParsedBlock)}
  {@const text = block.text}
  {@const links = extractWikiLinks(text)}
  {#if links.length === 0}
    <span>{text}</span>
  {:else}
    {@const parts = buildParts(text, links)}
    {#each parts as part}
      {#if part.type === "text"}
        <span>{part.content}</span>
      {:else}
        <a
          href="/p/{encodeURIComponent(part.target.toLowerCase())}"
          class="text-primary underline underline-offset-2 decoration-primary/40 hover:decoration-primary"
          onclick={(e) => e.stopPropagation()}
        >{part.content}</a>
      {/if}
    {/each}
  {/if}
{/snippet}

{#snippet propertyValue(value: string)}
  {@const links = extractWikiLinks(value)}
  {#if links.length === 0}
    <span>{value}</span>
  {:else}
    {@const parts = buildParts(value, links)}
    {#each parts as part}
      {#if part.type === "text"}
        <span>{part.content}</span>
      {:else}
        <a href="/p/{encodeURIComponent(part.target.toLowerCase())}" class="text-primary underline underline-offset-2 decoration-primary/40 hover:decoration-primary">{part.content}</a>
      {/if}
    {/each}
  {/if}
{/snippet}

<script lang="ts" module>
  type TextPart = { type: "text"; content: string };
  type LinkPart = { type: "link"; content: string; target: string };
  type Part = TextPart | LinkPart;

  function buildParts(text: string, links: Array<{ target: string; display: string; start: number; end: number }>): Part[] {
    const parts: Part[] = [];
    let lastEnd = 0;
    for (const link of links) {
      if (link.start > lastEnd) parts.push({ type: "text", content: text.slice(lastEnd, link.start) });
      parts.push({ type: "link", content: link.display, target: link.target });
      lastEnd = link.end;
    }
    if (lastEnd < text.length) parts.push({ type: "text", content: text.slice(lastEnd) });
    return parts;
  }
</script>
