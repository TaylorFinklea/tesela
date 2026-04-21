<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { parseBlocks } from "$lib/block-parser";
  import { api } from "$lib/api-client";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { Note } from "$lib/types/Note";
  import BlockEditor from "./BlockEditor.svelte";
  import { IconArrowRight } from "@tabler/icons-svelte";

  let {
    noteId,
    body,
    frontmatter,
    onContentChange,
    onleader: onLeader,
    onfocusedblockchange,
    drillBlockId = "",
    onDrillIn,
  }: {
    noteId: string;
    body: string;
    frontmatter: string;
    onContentChange?: (fullContent: string) => void;
    onleader?: () => void;
    onfocusedblockchange?: (block: ParsedBlock | null) => void;
    drillBlockId?: string;
    onDrillIn?: (blockId: string) => void;
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

  // Drill-in: show only the target block and its descendants
  const drillRootIndent = $derived.by(() => {
    if (!drillBlockId) return 0;
    return blocks.find(b => b.id === drillBlockId)?.indent_level ?? 0;
  });
  const visibleBlocks = $derived.by(() => {
    if (!drillBlockId) return blocks;
    const rootIdx = blocks.findIndex(b => b.id === drillBlockId);
    if (rootIdx < 0) return blocks;
    const rootIndent = blocks[rootIdx].indent_level;
    const result: ParsedBlock[] = [];
    for (let i = rootIdx; i < blocks.length; i++) {
      if (i > rootIdx && blocks[i].indent_level <= rootIndent) break;
      result.push(blocks[i]);
    }
    return result;
  });

  // Block-visual mode state
  let blockVisualMode = $state(false);
  let visualAnchor = $state<number | null>(null);
  let visualExtent = $state<number | null>(null);
  const visualRange = $derived.by(() => {
    if (!blockVisualMode || visualAnchor === null || visualExtent === null) return new Set<number>();
    const lo = Math.min(visualAnchor, visualExtent);
    const hi = Math.max(visualAnchor, visualExtent);
    return new Set(Array.from({ length: hi - lo + 1 }, (_, i) => lo + i));
  });

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
    onfocusedblockchange?.(visibleBlocks[focusedIndex] ?? null);
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

  function removeBlockTag(block: ParsedBlock, tagName: string) {
    const escaped = tagName.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const re = new RegExp(`\\s*#${escaped}(?![A-Za-z0-9_/-])`, "gi");
    const newText = block.raw_text.replace(re, "").trim();
    handleBlockChange(block.id, newText);
  }

  function handleStatusCycle(vi: number) {
    const block = visibleBlocks[vi];
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
      : Math.min(visibleBlocks.length - 1, focusedIndex + 1);
    focusedIndex = next;
  }

  function handleEnter(vi: number, textAfterCursor: string = "") {
    const current = visibleBlocks[vi];
    if (!current) return;
    const fullIdx = blocks.findIndex(b => b.id === current.id);
    if (fullIdx < 0) return;
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
      const updatedCurrent: ParsedBlock = { ...current, id: `${noteId}:split-${Date.now() + 1}` };
      mountHint = { blockId: newBlock.id, pos: 0, startInInsert: true };
      blocks = [...blocks.slice(0, fullIdx), updatedCurrent, newBlock, ...blocks.slice(fullIdx + 1)];
    } else {
      blocks = [...blocks.slice(0, fullIdx + 1), newBlock, ...blocks.slice(fullIdx + 1)];
    }
    saveBlocks(blocks);
    focusedIndex = vi + 1;
  }

  function handleIndent(vi: number, direction: "indent" | "outdent") {
    const block = visibleBlocks[vi];
    if (!block) return;
    const newLevel = direction === "indent" ? block.indent_level + 1 : Math.max(0, block.indent_level - 1);
    if (newLevel === block.indent_level) return;
    blocks = blocks.map(b => b.id === block.id ? { ...b, indent_level: newLevel } : b);
    saveBlocks(blocks);
  }

  function handleBackspace(vi: number) {
    const block = visibleBlocks[vi];
    if (!block || block.raw_text !== "" || blocks.length <= 1) return;
    blocks = blocks.filter(b => b.id !== block.id);
    saveBlocks(blocks);
    if (focusedIndex !== null && focusedIndex > 0) focusedIndex = focusedIndex - 1;
  }

  function handleBackspaceMerge(vi: number, currentText: string) {
    if (vi === 0) return;
    const prev = visibleBlocks[vi - 1];
    const current = visibleBlocks[vi];
    if (!prev || !current) return;
    const fullPrevIdx = blocks.findIndex(b => b.id === prev.id);
    const fullCurrIdx = blocks.findIndex(b => b.id === current.id);
    if (fullPrevIdx < 0 || fullCurrIdx < 0) return;
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
      ...blocks.slice(0, fullPrevIdx),
      mergedBlock,
      ...blocks.slice(fullCurrIdx + 1),
    ];
    saveBlocks(blocks);
    focusedIndex = vi - 1;
  }

  // Pending mount hint: cursor position + optional insert-mode entry for the next block to mount
  let mountHint = $state<{ blockId: string; pos: number; startInInsert: boolean } | null>(null);

  // Block clipboard for yy/p (array to support multi-block visual yank)
  let blockClipboard = $state<ParsedBlock[]>([]);

  function handleDeleteBlock(vi: number) {
    if (visibleBlocks.length <= 1) return;
    const block = visibleBlocks[vi];
    if (!block) return;
    const prev = Math.max(0, vi - 1);
    blocks = blocks.filter(b => b.id !== block.id);
    saveBlocks(blocks);
    focusedIndex = Math.min(prev, visibleBlocks.length - 2);
  }

  function handleYankBlock(vi: number) {
    const block = visibleBlocks[vi];
    if (block) blockClipboard = [{ ...block }];
  }

  function handlePasteBlock(vi: number) {
    if (blockClipboard.length === 0) return;
    const anchor = visibleBlocks[vi];
    if (!anchor) return;
    const fullIdx = blocks.findIndex(b => b.id === anchor.id);
    if (fullIdx < 0) return;
    const pasted = blockClipboard.map((b, i) => ({
      ...b,
      id: `${noteId}:paste-${Date.now()}-${i}`,
    }));
    blocks = [...blocks.slice(0, fullIdx + 1), ...pasted, ...blocks.slice(fullIdx + 1)];
    saveBlocks(blocks);
    focusedIndex = vi + pasted.length;
  }

  function handleNewBlockAbove(vi: number) {
    const current = visibleBlocks[vi];
    if (!current) return;
    const fullIdx = blocks.findIndex(b => b.id === current.id);
    if (fullIdx < 0) return;
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
    blocks = [...blocks.slice(0, fullIdx), newBlock, ...blocks.slice(fullIdx)];
    saveBlocks(blocks);
    focusedIndex = vi;
  }

  // Visual mode handlers
  function enterBlockVisualMode() {
    if (focusedIndex === null) return;
    blockVisualMode = true;
    visualAnchor = focusedIndex;
    visualExtent = focusedIndex;
  }

  function exitBlockVisualMode() {
    blockVisualMode = false;
    visualAnchor = null;
    visualExtent = null;
  }

  function handleVisualNav(dir: "up" | "down") {
    if (dir === "down") visualExtent = Math.min(visibleBlocks.length - 1, (visualExtent ?? 0) + 1);
    else visualExtent = Math.max(0, (visualExtent ?? 0) - 1);
  }

  function deleteVisualBlocks() {
    const sorted = [...visualRange].sort((a, b) => a - b);
    if (sorted.length === 0) return;
    const ids = new Set(sorted.map(vi => visibleBlocks[vi]?.id).filter(Boolean));
    if (blocks.length - ids.size < 1) return;
    const newFocus = Math.min(sorted[0]!, visibleBlocks.length - 1 - sorted.length);
    blocks = blocks.filter(b => !ids.has(b.id));
    saveBlocks(blocks);
    focusedIndex = Math.max(0, newFocus);
    exitBlockVisualMode();
  }

  function yankVisualBlocks() {
    const sorted = [...visualRange].sort((a, b) => a - b);
    blockClipboard = sorted.map(vi => ({ ...visibleBlocks[vi]! })).filter(b => b.id);
    exitBlockVisualMode();
  }
</script>

{#if visibleBlocks.length === 0}
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
    {#each visibleBlocks as block, vi (block.id)}
      {@const displayIndent = block.indent_level - drillRootIndent}
      <div
        class="group flex items-start transition-all relative
          {focusedIndex === vi ? 'bg-accent/40' : ''}
          {visualRange.has(vi) ? 'bg-primary/10 ring-1 ring-primary/20 rounded-md' : ''}"
        style="padding-left: {displayIndent * 24}px;"
      >
        <!-- Threading lines -->
        {#if displayIndent > 0}
          {#each { length: displayIndent } as _, lvl}
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
            onclick={(e) => { e.stopPropagation(); handleStatusCycle(vi); }}
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
            onclick={(e) => { e.stopPropagation(); handleStatusCycle(vi); }}
            title="Set status"
          >
            <span class="block w-[5px] h-[5px] rounded-full transition-colors {focusedIndex === vi ? 'bg-primary' : 'bg-muted-foreground/30'}"></span>
          </button>
        {/if}

        <!-- Drill-in icon (shows on hover) -->
        <!-- svelte-ignore a11y_consider_explicit_label -->
        <button
          class="shrink-0 pt-[11px] px-0.5 opacity-0 group-hover:opacity-30 hover:!opacity-90 transition-opacity"
          onclick={(e) => { e.stopPropagation(); onDrillIn?.(block.id); }}
          title="Drill in (Enter)"
        >
          <IconArrowRight size={11} stroke={1.5} class="text-muted-foreground" />
        </button>

        <!-- Content -->
        <div class="flex-1 min-w-0 py-1">
          <BlockEditor
            initialText={block.raw_text}
            onblur={() => { if (focusedIndex === vi) focusedIndex = null; }}
            onfocus={() => { focusedIndex = vi; }}
            onchange={(text) => handleBlockChange(block.id, text)}
            onnavigate={handleNavigate}
            onescape={() => { focusedIndex = null; }}
            onenter={(textAfter: string) => handleEnter(vi, textAfter)}
            onindent={(dir) => handleIndent(vi, dir)}
            onbackspaceempty={() => handleBackspace(vi)}
            onbackspacemerge={(text: string) => handleBackspaceMerge(vi, text)}
            initialCursorPos={mountHint?.blockId === block.id ? mountHint.pos : undefined}
            startininsert={(mountHint?.blockId === block.id && mountHint.startInInsert) || (focusedIndex === vi && block.raw_text === "")}
            onleader={onLeader}
            oncyclestatus={() => handleStatusCycle(vi)}
            ondeleteblock={() => handleDeleteBlock(vi)}
            onyankblock={() => handleYankBlock(vi)}
            onpasteblock={() => handlePasteBlock(vi)}
            onnewblockbelow={() => handleEnter(vi)}
            onnewblockabove={() => handleNewBlockAbove(vi)}
            ondrillIn={() => onDrillIn?.(block.id)}
            onentervisualmode={enterBlockVisualMode}
            onexitvisualmode={exitBlockVisualMode}
            onvisualnav={handleVisualNav}
            onvisualdelete={deleteVisualBlocks}
            onvisualyank={yankVisualBlocks}
            inVisualMode={blockVisualMode}
            focused={focusedIndex === vi}
            noteslist={notesList}
            statusChoices={statusChoices}
          />
        </div>

        <!-- Tag pills (right side) -->
        {#if block.tags.length > 0}
          <div class="shrink-0 flex items-center gap-1 self-center pr-2 py-1">
            {#each block.tags as tag}
              <span class="group/tag inline-flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70 font-medium">
                {tag}
                <!-- svelte-ignore a11y_consider_explicit_label -->
                <button
                  class="opacity-0 group-hover/tag:opacity-100 leading-none text-primary/40 hover:text-destructive transition-opacity ml-0.5"
                  onclick={(e) => { e.stopPropagation(); removeBlockTag(block, tag); }}
                  title="Remove #{tag}"
                >×</button>
              </span>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  </div>
{/if}
