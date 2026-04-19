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
    onfocusedblockchange,
  }: {
    noteId: string;
    body: string;
    frontmatter: string;
    onContentChange?: (fullContent: string) => void;
    onleader?: () => void;
    onfocusedblockchange?: (block: ParsedBlock | null) => void;
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

  // Fetch Status property page for dynamic status choices
  const statusPropertyQuery = createQuery(() => ({
    queryKey: ["note", "status"] as const,
    queryFn: () => api.getNote("status"),
  }));
  const statusChoices = $derived.by((): string[] => {
    const statusNote = statusPropertyQuery.data as Note | undefined;
    if (!statusNote) return ["todo", "doing", "done"];
    const choices = statusNote.metadata.custom.choices;
    return Array.isArray(choices) ? (choices as string[]) : ["todo", "doing", "done"];
  });
  const statusCycle = $derived(["", ...statusChoices]);

  let blocks = $state<ParsedBlock[]>(parseBlocks(noteId, body));
  let focusedIndex = $state<number | null>(null);
  let lastExternalBody = $state(body);
  let lastSentBody = $state(body);

  $effect(() => {
    if (body === lastExternalBody) return;
    lastExternalBody = body;
    if (body === lastSentBody) return;
    if (focusedIndex === null) {
      blocks = parseBlocks(noteId, body);
    }
  });

  // Notify parent when a block GAINS focus — keeps focusedBlock as "last focused"
  // so the sidebar doesn't lose its context when the user clicks into it.
  $effect(() => {
    if (focusedIndex === null) return;
    onfocusedblockchange?.(blocks[focusedIndex] ?? null);
  });

  function parseProperties(rawText: string): Record<string, string> {
    const props: Record<string, string> = {};
    for (const m of rawText.matchAll(/([A-Za-z_][A-Za-z0-9_]*):: (.+)/g)) {
      props[m[1]] = m[2];
    }
    return props;
  }

  function statusChar(s: string): string {
    if (s === "done") return "✓";
    if (s === "doing" || s === "in-review") return "◑";
    if (s === "todo") return "○";
    return "·";
  }

  function statusColorClass(s: string): string {
    if (s === "done") return "text-emerald-400/80";
    if (s === "doing" || s === "in-review") return "text-blue-400/80";
    if (s === "todo") return "text-amber-400/80";
    return "text-muted-foreground/60";
  }

  function setBlockStatus(rawText: string, status: string): string {
    const hasStatus = /^status:: .+$/m.test(rawText);
    if (status === "") {
      return rawText.replace(/\nstatus:: [^\n]+/g, "").replace(/^status:: [^\n]+\n?/gm, "");
    } else if (hasStatus) {
      return rawText.replace(/^status:: .+$/m, `status:: ${status}`);
    } else {
      return rawText + `\nstatus:: ${status}`;
    }
  }

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
    lastSentBody = `${bodyLines}\n`;
    onContentChange?.(`${frontmatter}${bodyLines}\n`);
  }

  function handleBlockChange(blockId: string, newRawText: string) {
    blocks = blocks.map((b) =>
      b.id === blockId
        ? {
            ...b,
            raw_text: newRawText,
            text: (newRawText.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
            properties: parseProperties(newRawText),
          }
        : b,
    );
    saveBlocks(blocks);
  }

  function handleStatusCycle(atIndex: number) {
    const block = blocks[atIndex];
    if (!block) return;
    const current = block.properties.status ?? "";
    const idx = statusCycle.indexOf(current);
    const next = statusCycle[(idx + 1) % statusCycle.length] ?? "";
    handleBlockChange(block.id, setBlockStatus(block.raw_text, next));
  }

  function handleNavigate(direction: "up" | "down") {
    if (focusedIndex === null) return;
    const next = direction === "up"
      ? Math.max(0, focusedIndex - 1)
      : Math.min(blocks.length - 1, focusedIndex + 1);
    focusedIndex = next;
  }

  function handleEnter(atIndex: number, textAfterCursor: string = "") {
    const current = blocks[atIndex];
    if (!current) return;
    const newBlock: ParsedBlock = {
      id: `${noteId}:new-${Date.now()}`,
      text: (textAfterCursor.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
      raw_text: textAfterCursor,
      tags: [],
      inherited_tags: [],
      properties: {},
      indent_level: current.indent_level,
      note_id: noteId,
    };
    if (textAfterCursor) {
      // Mid-block split: force current block remount so CM6 shows trimmed text
      const updatedCurrent: ParsedBlock = { ...current, id: `${noteId}:split-${Date.now() + 1}` };
      mountHint = { blockId: newBlock.id, pos: 0, startInInsert: true };
      blocks = [...blocks.slice(0, atIndex), updatedCurrent, newBlock, ...blocks.slice(atIndex + 1)];
    } else {
      blocks = [...blocks.slice(0, atIndex + 1), newBlock, ...blocks.slice(atIndex + 1)];
    }
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

  function handleBackspaceMerge(atIndex: number, currentText: string) {
    if (atIndex === 0) return;
    const prev = blocks[atIndex - 1];
    if (!prev) return;
    const mergePos = prev.raw_text.length;
    const mergedText = prev.raw_text + currentText;
    const mergedBlock: ParsedBlock = {
      ...prev,
      id: `${noteId}:merged-${Date.now()}`,
      raw_text: mergedText,
      text: (mergedText.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
    };
    mountHint = { blockId: mergedBlock.id, pos: mergePos, startInInsert: false };
    blocks = [
      ...blocks.slice(0, atIndex - 1),
      mergedBlock,
      ...blocks.slice(atIndex + 1),
    ];
    saveBlocks(blocks);
    focusedIndex = atIndex - 1;
  }

  // Pending mount hint: cursor position + optional insert-mode entry for the next block to mount
  let mountHint = $state<{ blockId: string; pos: number; startInInsert: boolean } | null>(null);

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
      inherited_tags: [],
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
      const newBlock: ParsedBlock = {
        id: `${noteId}:new-${Date.now()}`,
        text: "",
        raw_text: "",
        tags: [],
        inherited_tags: [],
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

        <!-- Bullet / Status icon -->
        {#if block.properties.status}
          <!-- svelte-ignore a11y_consider_explicit_label -->
          <button
            class="shrink-0 pt-[10px] pl-2 pr-1 cursor-pointer opacity-70 hover:opacity-100 transition-opacity"
            onclick={(e) => { e.stopPropagation(); handleStatusCycle(index); }}
            title="Cycle status ({block.properties.status})"
          >
            <span class="block text-[12px] leading-none font-mono w-[14px] text-center {statusColorClass(block.properties.status)}">
              {statusChar(block.properties.status)}
            </span>
          </button>
        {:else}
          <!-- svelte-ignore a11y_consider_explicit_label -->
          <button
            class="shrink-0 pt-[12px] pl-2 pr-1.5 cursor-default hover:cursor-pointer transition-opacity"
            onclick={(e) => { e.stopPropagation(); handleStatusCycle(index); }}
            title="Set status"
          >
            <span class="block w-[5px] h-[5px] rounded-full transition-colors {focusedIndex === index ? 'bg-primary' : 'bg-muted-foreground/30'}"></span>
          </button>
        {/if}

        <!-- Content -->
        <div class="flex-1 min-w-0 py-1 pr-3">
          <BlockEditor
            initialText={block.raw_text}
            onblur={() => { if (focusedIndex === index) focusedIndex = null; }}
            onfocus={() => { focusedIndex = index; }}
            onchange={(text) => handleBlockChange(block.id, text)}
            onnavigate={handleNavigate}
            onescape={() => { focusedIndex = null; }}
            onenter={(textAfter: string) => handleEnter(index, textAfter)}
            onindent={(dir) => handleIndent(index, dir)}
            onbackspaceempty={() => handleBackspace(index)}
            onbackspacemerge={(text: string) => handleBackspaceMerge(index, text)}
            initialCursorPos={mountHint?.blockId === block.id ? mountHint.pos : undefined}
            startininsert={(mountHint?.blockId === block.id && mountHint.startInInsert) || (focusedIndex === index && block.raw_text === "")}
            onleader={onLeader}
            oncyclestatus={() => handleStatusCycle(index)}
            ondeleteblock={() => handleDeleteBlock(index)}
            onyankblock={() => handleYankBlock(index)}
            onpasteblock={() => handlePasteBlock(index)}
            onnewblockbelow={() => handleEnter(index)}
            onnewblockabove={() => handleNewBlockAbove(index)}
            focused={focusedIndex === index}
            noteslist={notesList}
            statusChoices={statusChoices}
          />
        </div>
      </div>
    {/each}
  </div>
{/if}
