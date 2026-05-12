<script module lang="ts">
  // Phase 12.X — singleton "last-active outliner" so the journal view can
  // dispatch a single `tesela:restore-focus` and have only the outliner
  // the user was actually editing in respond. Without this, every
  // BlockOutliner instance on the page restores focus and the last one in
  // DOM order wins — visibly snapping the user to the bottom-most day.
  let lastActiveOutliner: HTMLElement | null = null;
  export function setLastActiveOutliner(el: HTMLElement | null) {
    lastActiveOutliner = el;
  }
  export function isLastActiveOutliner(el: HTMLElement | null): boolean {
    return !!el && el === lastActiveOutliner;
  }

  // Phase 12.X — one-shot flag so cross-day j/k navigation in the journal
  // can land on an empty block in NORMAL mode. The outliner's auto-INSERT
  // heuristic (empty + focused + !autoFocused + !restoredFocus) would
  // otherwise drop us into INSERT every hop, forcing j Esc j Esc j Esc.
  let nextFocusIsCrossNav = false;
  export function markNextFocusAsCrossNav() {
    nextFocusIsCrossNav = true;
  }
  export function consumeCrossNavFocus(): boolean {
    if (!nextFocusIsCrossNav) return false;
    nextFocusIsCrossNav = false;
    return true;
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { createQuery } from "@tanstack/svelte-query";
  import { parseBlocks } from "$lib/block-parser";
  import { toggleBlockTag, getBlockTags } from "$lib/block-tags";
  import { api } from "$lib/api-client";
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import type { Note } from "$lib/types/Note";
  import BlockEditor from "./BlockEditor.svelte";
  import QueryBlock from "./QueryBlock.svelte";
  import CollectionBlock from "./CollectionBlock.svelte";
  import { IconChevronRight, IconChevronDown, IconLock, IconChecklist } from "@tabler/icons-svelte";
  import {
    buildRegistry,
    buildInheritanceMap,
    getTagPropertyDefs,
    resolveTagChain,
    type PropertyDefinition,
  } from "$lib/property-registry";
  import DisplayChip from "./DisplayChip.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import type { HiddenKeysConfig } from "$lib/cm-decorations";
  import { prefs } from "$lib/preferences.svelte";
  import { OutlinerHistory, type OutlinerSnapshot } from "$lib/stores/outliner-history.svelte";
  import { pinBlock, setBottomDrawerOpen, setBottomTab } from "$lib/stores/pane-state.svelte";

  let {
    noteId,
    body,
    frontmatter,
    onContentChange,
    onCancelAndFlush,
    onleader: onLeader,
    onfocusedblockchange,
    drillBlockId = "",
    onDrillIn,
    isPinnedTab = false,
  }: {
    noteId: string;
    body: string;
    frontmatter: string;
    onContentChange?: (fullContent: string) => void;
    /** Cancel any pending/in-flight save and PUT immediately. Called from
     *  `applySnapshot` so undo/redo restored bodies win the race against any
     *  in-flight pre-undo PUT. Falls back to the debounced `onContentChange`
     *  path if the parent didn't wire it. */
    onCancelAndFlush?: (fullContent: string) => void;
    onleader?: () => void;
    onfocusedblockchange?: (block: ParsedBlock | null) => void;
    drillBlockId?: string;
    onDrillIn?: (blockId: string) => void;
    isPinnedTab?: boolean;
  } = $props();

  // Fetch notes list for autocomplete + tag-property visibility resolution
  const notesForAutocomplete = createQuery(() => ({
    queryKey: ["notes", { limit: 200 }] as const,
    queryFn: () => api.listNotes({ limit: 200 }),
  }));
  const allNotes = $derived((notesForAutocomplete.data ?? []) as Note[]);
  const notesList = $derived(
    allNotes.map((n) => ({
      id: n.id,
      title: n.title,
      tags: n.metadata.tags,
      note_type: n.metadata.note_type,
    })),
  );
  const propertyRegistry = $derived(buildRegistry(allNotes));
  const inheritanceMap = $derived(buildInheritanceMap(allNotes));

  /**
   * For a tag being toggled ON via `toggleBlockTag`, return the property names
   * that should be auto-appended as empty continuation lines. Skips any
   * property marked `hide_by_default` — those start hidden anyway, no value
   * to nudge the user toward filling in yet.
   */
  function autoFillNamesForTag(tagName: string): string[] {
    const defs = getTagPropertyDefs(tagName, allNotes, propertyRegistry, inheritanceMap);
    return defs.filter((d) => !d.hide_by_default).map((d) => d.name);
  }

  /**
   * Compute the keys to hide in this block's editor based on the block's
   * inherited tag chain. A key gets `hide_by_default` if any tag-property def
   * has that flag; same for `hide_empty`.
   */
  // System keys for query/collection blocks. Always hidden by default — the
  // user manages them through the block's UI (tab strip, view switcher, etc.)
  // not by editing the raw `key:: value` lines.
  const SYSTEM_HIDDEN_KEYS: ReadonlySet<string> = new Set([
    "query",
    "view",
    "views",
    "active_view",
    "collection",
  ]);

  /**
   * Phase 10.5 — for each block, return the ordered list of property keys
   * its tags want surfaced as inline chips. Walks `display_chips` from
   * each tag page (direct + inherited) and dedupes by key. Empty array
   * means "no chips" (block falls back to plain prose + tag pills only).
   *
   * Pair with `block.properties[key]` to render — chips with no value or
   * an empty string value are skipped entirely so the block stays
   * compact when a property is unset.
   */
  /**
   * Phase 12.4 — count direct subtasks (one indent deeper) and how many
   * are done. Returns null when the block has no children with `status::`,
   * so the rollup chip stays out of the way for non-task hierarchies.
   * Direct children only; deeper grandchildren are intentionally excluded
   * to keep the rollup actionable ("close these N to close the parent").
   */
  function subtaskRollup(block: ParsedBlock): { done: number; total: number } | null {
    const idx = blocks.findIndex((b) => b.id === block.id);
    if (idx < 0) return null;
    let done = 0;
    let total = 0;
    for (let i = idx + 1; i < blocks.length; i++) {
      const sub = blocks[i];
      if (sub.indent_level <= block.indent_level) break;
      if (sub.indent_level !== block.indent_level + 1) continue;
      const status = sub.properties.status;
      if (!status) continue;
      total++;
      if (status === "done") done++;
    }
    return total === 0 ? null : { done, total };
  }

  /**
   * Phase 12.4 — true when the block has a non-empty `blocked_by::` and at
   * least one referenced block is not yet `done`. We render a small lock
   * indicator so the user sees at a glance which tasks are gated. The
   * value is parsed as comma-separated block ids `<note_id>:<line>`.
   */
  function isBlocked(block: ParsedBlock): boolean {
    const raw = block.properties["blocked_by"];
    if (!raw) return false;
    const refs = raw
      .split(",")
      .map((s) => s.trim().replace(/^\[\[/, "").replace(/\]\]$/, ""))
      .filter(Boolean);
    if (refs.length === 0) return false;
    // Best-effort: only consider refs that resolve within the same note.
    // Cross-note dependencies need a wider lookup that v1 doesn't ship.
    return refs.some((ref) => {
      const target = blocks.find((b) => b.id === ref);
      if (!target) return true; // unresolved → conservatively treat as blocked
      return target.properties.status !== "done";
    });
  }

  function displayChipsFor(block: ParsedBlock): Array<{ key: string; value: string; def: PropertyDefinition }> {
    const allTags = [...new Set([...block.tags, ...block.inherited_tags])];
    const seen = new Set<string>();
    const out: Array<{ key: string; value: string; def: PropertyDefinition }> = [];
    for (const tag of allTags) {
      for (const ancestor of resolveTagChain(tag, inheritanceMap)) {
        const tagPage = allNotes.find(
          (n) => n.title.toLowerCase() === ancestor && n.metadata.note_type === "Tag",
        );
        if (!tagPage) continue;
        const chipsRaw = tagPage.metadata.custom.display_chips;
        if (!Array.isArray(chipsRaw)) continue;
        for (const rawKey of chipsRaw as string[]) {
          const k = String(rawKey).toLowerCase();
          if (seen.has(k)) continue;
          seen.add(k);
          const value = block.properties[k];
          if (!value || !value.trim()) continue;
          const def = propertyRegistry.get(k);
          if (!def) continue;
          out.push({ key: k, value, def });
        }
      }
    }
    return out;
  }

  /**
   * Phase 10.4 — full property defs for the block's tag chain. Used to drive
   * the in-block `/p` chord submenu (key picker → value entry) so the user
   * can edit properties without leaving the editor flow. Walks all tags
   * (direct + inherited), dedupes by lowercased property name.
   */
  function propertyDefsFor(block: ParsedBlock) {
    const allTags = [...new Set([...block.tags, ...block.inherited_tags])];
    const seen = new Set<string>();
    const out = [];
    for (const tag of allTags) {
      for (const def of getTagPropertyDefs(tag, allNotes, propertyRegistry, inheritanceMap)) {
        const k = def.name.toLowerCase();
        if (!seen.has(k)) { seen.add(k); out.push(def); }
      }
    }
    return out;
  }

  function hiddenKeysFor(block: ParsedBlock): HiddenKeysConfig {
    const allTags = [...new Set([...block.tags, ...block.inherited_tags])];
    const hide = new Set<string>(SYSTEM_HIDDEN_KEYS);
    const hideEmpty = new Set<string>();
    // Phase 9.9 — every `key:: value` line on the block gets the hidden-line
    // decoration by default. The drawer's Properties tab is the canonical
    // editing surface; the inline view stays compact. The user reveals a
    // block's properties via the chevron toggle (or `gp` vim chord) which
    // adds `.show-props` to the wrapper, and the CSS rule
    // `.show-props .cm-tesela-hidden-prop-line { display: block }` overrides.
    for (const key of Object.keys(block.properties)) {
      hide.add(key.toLowerCase());
    }
    for (const tag of allTags) {
      for (const def of getTagPropertyDefs(tag, allNotes, propertyRegistry, inheritanceMap)) {
        const k = def.name.toLowerCase();
        if (def.hide_by_default) hide.add(k);
        if (def.hide_empty) hideEmpty.add(k);
      }
    }
    return { hide, hideEmpty };
  }

  /**
   * Whether to render the status indicator next to the bullet. Logseq-style:
   * present when status is set, OR when any tag in the block's chain declares
   * a `status` property (so a tagged Task block shows an empty placeholder).
   */
  function shouldShowStatus(block: ParsedBlock): boolean {
    if (block.properties.status !== undefined) return true;
    const tags = [...new Set([...block.tags, ...block.inherited_tags])];
    for (const tag of tags) {
      const defs = getTagPropertyDefs(tag, allNotes, propertyRegistry, inheritanceMap);
      if (defs.some((d) => d.name.toLowerCase() === "status")) return true;
    }
    return false;
  }

  /**
   * Whether the block has any hidden property lines that the chevron should
   * reveal. True when raw_text contains a `key:: value` line (or empty-value
   * line) whose key is configured as hide_by_default OR (hide_empty + empty).
   */
  const HIDDEN_PROBE_RE = /^([A-Za-z_][A-Za-z0-9_]*)::[ \t]?(.*)$/gm;
  function hasHiddenContent(block: ParsedBlock): boolean {
    const config = hiddenKeysFor(block);
    if (config.hide.size === 0 && config.hideEmpty.size === 0) return false;
    HIDDEN_PROBE_RE.lastIndex = 0;
    let m: RegExpExecArray | null;
    while ((m = HIDDEN_PROBE_RE.exec(block.raw_text)) !== null) {
      const key = m[1].toLowerCase();
      if (key === "tags") continue;
      const value = m[2] ?? "";
      if (config.hide.has(key)) return true;
      if (config.hideEmpty.has(key) && value.trim() === "") return true;
    }
    return false;
  }

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
  // Track which note `lastExternalBody`/`lastSentBody` belong to so the
  // body-sync effect can distinguish a real noteId change (always replace)
  // from a same-note body update (preserve focus by block id).
  let lastBodyNoteId = $state(noteId);

  // True when focusedIndex was set by the page-mount auto-focus effect, not
  // by a user action. Suppresses the empty-block→Insert auto-entry below so
  // landing on a fresh page stays in Normal. Cleared on any user-initiated
  // focus change (click, new-block creation).
  let autoFocused = $state(false);
  // Note IDs we've already auto-focused. Prevents the effect from re-running
  // mid-session if focusedIndex transiently goes null (e.g. blur on click-away).
  let lastAutoFocusedNoteId = $state<string | null>(null);

  // True when the most recent focusedIndex change came from undo / redo —
  // suppresses the empty-block→Insert remount heuristic for one render so
  // restored empty blocks land in Normal, not Insert. Cleared on any user-
  // initiated focus change (click, navigate, new-block creation).
  let restoredFocus = $state(false);

  // Outliner-level undo / redo. One stack per BlockOutliner instance; cleared
  // on noteId change (page nav) and on external body reparse (so stale block
  // IDs don't survive into a snapshot we later try to restore).
  const history = new OutlinerHistory();

  function pushUndo(): void {
    history.push({ blocks, focusedIndex, collapsedBlocks });
  }

  function applySnapshot(s: OutlinerSnapshot): void {
    blocks = s.blocks.map((b) => ({ ...b }));
    focusedIndex = s.focusedIndex;
    // Suppress the empty-block→Insert remount heuristic for the next render
    // tick so a redo/undo that lands on an empty block stays in Normal.
    restoredFocus = true;
    collapsedBlocks = new Set(s.collapsedBlocks);
    // Cancel any in-flight pre-undo PUT and flush the restored body
    // immediately so the server's WS echo carries the restored state
    // (not the pre-undo state). Falls through to debounced save if the
    // parent didn't wire onCancelAndFlush.
    saveBlocksImmediate(blocks);
    persistFold();
  }

  function undoOutliner(): boolean {
    const snap = history.popUndo({ blocks, focusedIndex, collapsedBlocks });
    if (!snap) return false;
    applySnapshot(snap);
    return true;
  }

  function redoOutliner(): boolean {
    const snap = history.popRedo({ blocks, focusedIndex, collapsedBlocks });
    if (!snap) return false;
    applySnapshot(snap);
    return true;
  }

  // Insert-session promotion. Vim treats an entire insert session (i…Esc) as
  // one atomic edit for `u`. We mirror that: cache a pre-edit snapshot on
  // Insert-mode entry, and only commit it to the undo stack on the FIRST
  // keystroke during that session (in handleBlockChange). Bare `iEsc` with
  // no typing leaves no trace.
  let pendingInsertSnapshot: OutlinerSnapshot | null = null;

  function beginInsertSession(): void {
    pendingInsertSnapshot = {
      blocks: blocks.map((b) => ({ ...b })),
      focusedIndex,
      collapsedBlocks: new Set(collapsedBlocks),
    };
  }

  function endInsertSession(): void {
    pendingInsertSnapshot = null;
  }

  // Phase 10.5 — `key:: value` continuation lines are unconditionally
  // hidden in the cm-editor; the bottom drawer (and configurable per-tag
  // chips) are the canonical display surfaces. The earlier per-block
  // `expandedProps` state and chevron toggle were removed in favor of
  // "always hidden inline." Files on disk still contain the property
  // lines as plain Markdown — only the editor's render is compacted.

  // Per-page fold state: block IDs whose subtree is collapsed. Persisted to
  // localStorage keyed by noteId so it survives reloads + page switches.
  function loadFold(id: string): Set<string> {
    if (typeof localStorage === "undefined") return new Set();
    const raw = localStorage.getItem(`tesela:fold:${id}`);
    if (!raw) return new Set();
    try { return new Set(JSON.parse(raw) as string[]); }
    catch { return new Set(); }
  }
  let collapsedBlocks = $state<Set<string>>(loadFold(noteId));
  // Reload fold state when navigating between pages.
  let lastFoldNoteId = noteId;
  $effect(() => {
    if (noteId !== lastFoldNoteId) {
      lastFoldNoteId = noteId;
      collapsedBlocks = loadFold(noteId);
    }
  });
  function persistFold() {
    if (typeof localStorage === "undefined") return;
    localStorage.setItem(`tesela:fold:${noteId}`, JSON.stringify([...collapsedBlocks]));
  }
  function toggleFold(blockId: string) {
    pushUndo();
    const next = new Set(collapsedBlocks);
    if (next.has(blockId)) next.delete(blockId);
    else next.add(blockId);
    collapsedBlocks = next;
    persistFold();
  }

  // Drill-in: show only the target block and its descendants
  const drillRootIndent = $derived.by(() => {
    if (!drillBlockId) return 0;
    return blocks.find(b => b.id === drillBlockId)?.indent_level ?? 0;
  });
  const visibleBlocks = $derived.by(() => {
    // Step 1: apply drill-in filter
    let drilled: ParsedBlock[];
    if (!drillBlockId) {
      drilled = blocks;
    } else {
      const rootIdx = blocks.findIndex(b => b.id === drillBlockId);
      if (rootIdx < 0) {
        drilled = blocks;
      } else {
        const rootIndent = blocks[rootIdx].indent_level;
        const result: ParsedBlock[] = [];
        for (let i = rootIdx; i < blocks.length; i++) {
          if (i > rootIdx && blocks[i].indent_level <= rootIndent) break;
          result.push(blocks[i]);
        }
        drilled = result;
      }
    }
    // Step 2: hide descendants of any collapsed block. We walk in order and
    // skip until we hit indent_level ≤ the collapsed parent's level.
    if (collapsedBlocks.size === 0) return drilled;
    const out: ParsedBlock[] = [];
    let hideUntilIndentLte: number | null = null;
    for (const b of drilled) {
      if (hideUntilIndentLte !== null) {
        if (b.indent_level > hideUntilIndentLte) continue;
        hideUntilIndentLte = null;
      }
      out.push(b);
      if (collapsedBlocks.has(b.id)) hideUntilIndentLte = b.indent_level;
    }
    return out;
  });

  /** Whether the visible block at index `vi` has any children — used to
   *  conditionally render the fold-toggle chevron. A block has children when
   *  the next *non-collapsed* block in `blocks` (not visibleBlocks) sits at a
   *  greater indent_level — we check the underlying blocks array so a folded
   *  block still shows a chevron (so you can unfold it). */
  function hasChildren(block: ParsedBlock): boolean {
    const idx = blocks.findIndex(b => b.id === block.id);
    if (idx < 0 || idx === blocks.length - 1) return false;
    return blocks[idx + 1].indent_level > block.indent_level;
  }

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
    const noteChanged = noteId !== lastBodyNoteId;
    if (!noteChanged && body === lastExternalBody) return;
    lastExternalBody = body;
    lastBodyNoteId = noteId;
    if (noteChanged) {
      // Phase 9.9 follow-up — when noteId changes (page nav within same
      // BlockOutliner instance, e.g. drilling via gd or Esc-back), every
      // old block id has the previous note's prefix and won't exist in the
      // new body. The "preserve focus by id" branch below would always hit
      // its newIdx === -1 early-return and leave `blocks` stale, so the
      // user sees the wrong note's content. Reset everything for the new
      // note instead.
      blocks = parseBlocks(noteId, body);
      lastSentBody = body;
      focusedIndex = null;
      autoFocused = false;
      restoredFocus = false;
      history.clear();
      return;
    }
    if (body === lastSentBody) return;
    const reparsed = parseBlocks(noteId, body);
    if (focusedIndex === null) {
      blocks = reparsed;
      history.clear();
      return;
    }
    // Phase 9.7 — when a block is focused, we used to skip the reparse to
    // avoid yanking the user mid-typing. That made drawer-driven property
    // edits invisible in the outliner until the user re-focused. Preserve
    // focus by block id instead: if the focused block still exists in the
    // reparsed list, swap blocks in place.
    const focusedId = blocks[focusedIndex]?.id;
    const newIdx = focusedId ? reparsed.findIndex((b) => b.id === focusedId) : -1;
    if (newIdx === -1) return; // focused block vanished — keep current state
    blocks = reparsed;
    if (newIdx !== focusedIndex) focusedIndex = newIdx;
    // External body change wipes our snapshots — they reference block IDs
    // that may no longer exist after the reparse.
    history.clear();
  });

  // Clear undo/redo on page navigation. Snapshots are page-local: restoring
  // a snapshot from page A while viewing page B would corrupt B.
  let lastHistoryNoteId = $state<string | null>(null);
  $effect(() => {
    if (lastHistoryNoteId === noteId) return;
    lastHistoryNoteId = noteId;
    history.clear();
  });

  // Auto-focus first block on page load + on noteId change. Lands in Normal
  // mode (autoFocused gates the empty-block→Insert rule below). Re-runs
  // only when noteId changes — guarded so a transient focusedIndex=null
  // (e.g. clicking outside) doesn't yank focus back to block 0.
  //
  // Phase 9.9 follow-up — when the URL has `?fresh=1` (set by the command
  // palette right after creating a brand-new note), suppress the autoFocused
  // gate so the single empty seed block enters INSERT mode automatically.
  // The user typed Cmd+K → "Create" → expected to keep typing immediately;
  // they shouldn't have to press `i` first.
  $effect(() => {
    if (lastAutoFocusedNoteId === noteId) return;
    if (visibleBlocks.length === 0) return;
    lastAutoFocusedNoteId = noteId;
    if (focusedIndex === null) {
      focusedIndex = 0;
      const isFresh = typeof window !== "undefined" &&
        new URL(window.location.href).searchParams.get("fresh") === "1";
      autoFocused = !isFresh;
      if (isFresh && visibleBlocks[0]?.raw_text === "") {
        markRecentlyCreated(visibleBlocks[0].id);
      }
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
    if (s === "canceled" || s === "cancelled") return "✗";
    if (s === "blocked") return "⧖";
    if (s === "paused") return "⏸";
    return "·";
  }

  function statusColorClass(s: string): string {
    if (s === "done") return "text-emerald-400/80";
    if (s === "doing" || s === "in-review") return "text-blue-400/80";
    if (s === "todo") return "text-amber-400/80";
    if (s === "canceled" || s === "cancelled" || s === "blocked") return "text-red-400/70";
    if (s === "paused") return "text-muted-foreground/70";
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

  function buildFullContent(updated: ParsedBlock[]): { full: string; bodyOnly: string } {
    const bodyLines = updated
      .map((b) => {
        const indent = "  ".repeat(b.indent_level);
        const lines = b.raw_text.split("\n");
        const first = `${indent}- ${lines[0]}`;
        const rest = lines.slice(1).map((l: string) => `${indent}  ${l}`);
        return [first, ...rest].join("\n");
      })
      .join("\n");
    return { full: `${frontmatter}${bodyLines}\n`, bodyOnly: `${bodyLines}\n` };
  }

  function saveBlocks(updated: ParsedBlock[]) {
    const { full, bodyOnly } = buildFullContent(updated);
    lastSentBody = bodyOnly;
    onContentChange?.(full);
  }

  /** Like `saveBlocks` but bypasses the debounce — used by `applySnapshot`
   *  so an outliner-undo cancels any in-flight pre-undo PUT and immediately
   *  PUTs the restored body. Falls through to the debounced path if the
   *  parent didn't wire `onCancelAndFlush`. */
  function saveBlocksImmediate(updated: ParsedBlock[]) {
    const { full, bodyOnly } = buildFullContent(updated);
    lastSentBody = bodyOnly;
    if (onCancelAndFlush) onCancelAndFlush(full);
    else onContentChange?.(full);
  }

  function handleBlockChange(blockId: string, newRawText: string) {
    // First keystroke of an insert session: promote the cached pre-edit
    // snapshot onto the undo stack. Programmatic callers (status cycle,
    // tag toggle, etc.) call pushUndo() themselves and aren't in Insert
    // mode, so pendingInsertSnapshot is null for them — no-op.
    if (pendingInsertSnapshot) {
      history.push(pendingInsertSnapshot);
      pendingInsertSnapshot = null;
    }
    const parsedTags = getBlockTags(newRawText);
    // Properties parser sees tags:: too; strip it so it doesn't double-display
    const props = parseProperties(newRawText);
    delete props.tags;
    blocks = blocks.map((b) =>
      b.id === blockId
        ? {
            ...b,
            raw_text: newRawText,
            text: (newRawText.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
            tags: parsedTags,
            properties: props,
          }
        : b,
    );
    saveBlocks(blocks);
  }

  function removeBlockTag(block: ParsedBlock, tagName: string) {
    pushUndo();
    handleBlockChange(block.id, toggleBlockTag(block.raw_text, tagName));
  }

  function handleStatusCycle(vi: number) {
    const block = visibleBlocks[vi];
    if (!block) return;
    pushUndo();
    const current = block.properties.status ?? "";
    const idx = statusCycle.indexOf(current);
    const next = statusCycle[(idx + 1) % statusCycle.length] ?? "";
    // Phase 10.1 follow-up — Cmd+Enter is "make this a task" in the user's
    // model. If the block has no tag yet AND we're cycling INTO a non-empty
    // status (i.e. promoting it to tracked work), auto-add `tags:: Task` so
    // the block shows up in /p/tasks. Cycling back to empty status leaves
    // the existing tag set alone.
    let nextRaw = setBlockStatus(block.raw_text, next);
    const hasAnyTag = block.tags.length > 0;
    if (!hasAnyTag && next !== "") {
      const fillNames = autoFillNamesForTag("Task");
      nextRaw = toggleBlockTag(nextRaw, "Task", fillNames);
    }
    handleBlockChange(block.id, nextRaw);
  }

  function handleNavigate(direction: "up" | "down", count = 1) {
    if (focusedIndex === null) return;
    const atTopEdge = direction === "up" && focusedIndex === 0;
    const atBottomEdge =
      direction === "down" && focusedIndex === visibleBlocks.length - 1;
    // Phase 12.X — at the outliner's edge, hand off to the parent (e.g.
    // JournalView) so j/k can cross day boundaries. The parent decides
    // which sibling outliner to focus; if no listener handles it, focus
    // simply stays put.
    if (atTopEdge || atBottomEdge) {
      if (rootEl) {
        rootEl.dispatchEvent(
          new CustomEvent("tesela:cross-outliner-nav", {
            detail: { direction },
            bubbles: true,
          }),
        );
      }
      return;
    }
    const next = direction === "up"
      ? Math.max(0, focusedIndex - count)
      : Math.min(visibleBlocks.length - 1, focusedIndex + count);
    focusedIndex = next;
    restoredFocus = false;
    // Phase 9.9 — keep the cursor in view as cross-block j/k advances. Use
    // `nearest` so the viewport only scrolls when the new block is offscreen.
    // Phase 12.X — scope the query to THIS outliner's root. The journal
    // view stacks multiple BlockOutliners (one per day) and each one uses
    // its own `data-block-vi` index 0..N. A document-level `querySelector`
    // would match the *first* `[data-block-vi="${next}"]` in DOM order
    // (the topmost day's block), so j/k from any later day would scroll
    // the viewport up to that day instead of advancing within the
    // current day's outliner.
    requestAnimationFrame(() => {
      const el = rootEl?.querySelector(`[data-block-vi="${next}"]`);
      el?.scrollIntoView({ block: "nearest", behavior: "auto" });
    });
  }

  // Phase 9.9 — outliner-level half-page jump (vim Ctrl+U / Ctrl+D
  // convention). Each block is its own cm-editor, so single-block half-page
  // is meaningless; we jump 10 blocks and let scrollIntoView follow.
  const PAGE_JUMP_BLOCKS = 10;
  function handlePageJump(direction: "up" | "down") {
    handleNavigate(direction, PAGE_JUMP_BLOCKS);
  }

  /**
   * Fetch a template note and insert its body as child blocks under the
   * given parent block. Indents are normalized so the template's outermost
   * blocks become children of the parent (not preserving template's absolute
   * indent levels).
   */
  async function insertTemplateAfter(parentBlockId: string, templateNoteId: string) {
    let templateNote: Note;
    try {
      templateNote = await api.getNote(templateNoteId);
    } catch (e) {
      console.error("Failed to fetch template note:", e);
      return;
    }
    const tplBlocks = parseBlocks(templateNoteId, templateNote.body);
    if (tplBlocks.length === 0) return;
    const parentIdx = blocks.findIndex((b) => b.id === parentBlockId);
    if (parentIdx < 0) return;
    pushUndo();
    const parentIndent = blocks[parentIdx].indent_level;
    const minTplIndent = Math.min(...tplBlocks.map((b) => b.indent_level));
    const inserted: ParsedBlock[] = tplBlocks.map((tb, i) => ({
      ...tb,
      id: `${noteId}:tmpl-${Date.now()}-${i}`,
      note_id: noteId,
      // Re-base indent so the template's outermost blocks become children of
      // the current block. Preserves relative nesting within the template.
      indent_level: parentIndent + 1 + (tb.indent_level - minTplIndent),
    }));
    blocks = [
      ...blocks.slice(0, parentIdx + 1),
      ...inserted,
      ...blocks.slice(parentIdx + 1),
    ];
    saveBlocks(blocks);
  }

  function handleEnter(vi: number, textAfterCursor: string = "") {
    const current = visibleBlocks[vi];
    if (!current) return;
    const fullIdx = blocks.findIndex(b => b.id === current.id);
    if (fullIdx < 0) return;
    pushUndo();
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
    autoFocused = false;
    restoredFocus = false;
    markRecentlyCreated(newBlock.id);
  }

  /** Collect the IDs of `block` plus all of its descendants (any blocks
   *  immediately following at a strictly greater indent_level). */
  function subtreeIds(block: ParsedBlock): Set<string> {
    const idx = blocks.findIndex(b => b.id === block.id);
    if (idx < 0) return new Set([block.id]);
    const ids = new Set([block.id]);
    for (let i = idx + 1; i < blocks.length; i++) {
      if (blocks[i].indent_level <= block.indent_level) break;
      ids.add(blocks[i].id);
    }
    return ids;
  }

  function handleIndent(vi: number, direction: "indent" | "outdent") {
    const block = visibleBlocks[vi];
    if (!block) return;
    // Outdent at root is a no-op; otherwise the parent and all descendants
    // shift uniformly so subtree relationships are preserved.
    if (direction === "outdent" && block.indent_level === 0) return;
    pushUndo();
    const delta = direction === "indent" ? 1 : -1;
    const ids = subtreeIds(block);
    blocks = blocks.map(b => ids.has(b.id) ? { ...b, indent_level: Math.max(0, b.indent_level + delta) } : b);
    saveBlocks(blocks);
  }

  function handleBackspace(vi: number) {
    const block = visibleBlocks[vi];
    if (!block || block.raw_text !== "" || blocks.length <= 1) return;
    pushUndo();
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
    pushUndo();
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

  // Tracks the most-recently-created empty block so its auto-INSERT only
  // fires once. Cleared on the next animation frame (after the BlockEditor
  // has mounted and consumed the flag) so a later focus on the same block
  // — via j/k or click — lands in NORMAL, not INSERT. Without this, the
  // old `(empty && focused && !autoFocused && !restoredFocus)` heuristic
  // re-fired on every navigation onto any existing empty block.
  let recentlyCreatedBlockId = $state<string | null>(null);
  function markRecentlyCreated(blockId: string) {
    recentlyCreatedBlockId = blockId;
    requestAnimationFrame(() => {
      if (recentlyCreatedBlockId === blockId) recentlyCreatedBlockId = null;
    });
  }

  // Block clipboard for yy/p (array to support multi-block visual yank)
  let blockClipboard = $state<ParsedBlock[]>([]);

  function handleDeleteBlock(vi: number) {
    if (visibleBlocks.length <= 1) return;
    const block = visibleBlocks[vi];
    if (!block) return;
    pushUndo();
    // Vim convention: dd both deletes AND yanks into the register, so a
    // subsequent p pastes the deleted block.
    blockClipboard = [{ ...block }];
    blocks = blocks.filter(b => b.id !== block.id);
    saveBlocks(blocks);
    // The deleted block's BlockEditor unmounts, firing a blur that
    // (synchronously) nulls focusedIndex via the per-row handler. Defer the
    // refocus to a microtask so it lands AFTER the unmount blur. Also clamp
    // to the new visibleBlocks length.
    queueMicrotask(() => {
      const newLen = visibleBlocks.length;
      if (newLen === 0) focusedIndex = null;
      else focusedIndex = Math.min(Math.max(0, vi - 1), newLen - 1);
    });
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
    pushUndo();
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
    pushUndo();
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
    autoFocused = false;
    restoredFocus = false;
    markRecentlyCreated(newBlock.id);
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
    pushUndo();
    // Vim convention: visual-mode delete also yanks to the register.
    blockClipboard = sorted.map(vi => ({ ...visibleBlocks[vi]! })).filter(b => b.id);
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

  /**
   * Cycle status across all blocks in the visual selection. The next status
   * is computed from the FIRST selected block's current status; all selected
   * blocks then move to that next status (so the operation is idempotent and
   * predictable, not "each block independently advances").
   */
  function bulkCycleStatus() {
    const sorted = [...visualRange].sort((a, b) => a - b);
    if (sorted.length === 0) return;
    const first = visibleBlocks[sorted[0]!];
    if (!first) return;
    pushUndo();
    const current = first.properties.status ?? "";
    const idx = statusCycle.indexOf(current);
    const next = statusCycle[(idx + 1) % statusCycle.length] ?? "";
    const ids = new Set(sorted.map((vi) => visibleBlocks[vi]?.id).filter(Boolean) as string[]);
    blocks = blocks.map((b) => {
      if (!ids.has(b.id)) return b;
      const newRaw = setBlockStatus(b.raw_text, next);
      const props = parseProperties(newRaw);
      delete props.tags;
      return {
        ...b,
        raw_text: newRaw,
        text: (newRaw.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
        tags: getBlockTags(newRaw),
        properties: props,
      };
    });
    saveBlocks(blocks);
    // Stay in visual mode so the user can fire again.
  }

  /**
   * Indent / outdent every block in the visual selection AND all their
   * descendants. Subtree relationships are preserved across the operation;
   * if a child is also explicitly selected, dedup via Set ensures it only
   * shifts once. Outdent clamps at 0.
   */
  function bulkIndent(direction: "indent" | "outdent") {
    if (visualRange.size === 0) return;
    const ids = new Set<string>();
    for (const vi of visualRange) {
      const b = visibleBlocks[vi];
      if (!b) continue;
      for (const id of subtreeIds(b)) ids.add(id);
    }
    if (ids.size === 0) return;
    pushUndo();
    const delta = direction === "indent" ? 1 : -1;
    blocks = blocks.map((b) => ids.has(b.id) ? { ...b, indent_level: Math.max(0, b.indent_level + delta) } : b);
    saveBlocks(blocks);
  }

  /**
   * Toggle a tag across all blocks in the visual selection. If ANY selected
   * block already has the tag, this REMOVES it from all (turn-off-bias);
   * otherwise it ADDS the tag (with auto-fill props) to all that don't have it.
   */
  function bulkToggleTag(tagName: string) {
    const sorted = [...visualRange].sort((a, b) => a - b);
    if (sorted.length === 0) return;
    pushUndo();
    const lower = tagName.toLowerCase();
    const ids = new Set(sorted.map((vi) => visibleBlocks[vi]?.id).filter(Boolean) as string[]);
    const fillNames = autoFillNamesForTag(tagName);
    const anyHas = sorted.some((vi) => {
      const b = visibleBlocks[vi];
      return b && getBlockTags(b.raw_text).some((t) => t.toLowerCase() === lower);
    });
    blocks = blocks.map((b) => {
      if (!ids.has(b.id)) return b;
      const has = getBlockTags(b.raw_text).some((t) => t.toLowerCase() === lower);
      // anyHas=true → we're removing across the selection; skip blocks that don't have it.
      // anyHas=false → we're adding; skip blocks that already have it.
      if (anyHas !== has) return b;
      const newRaw = toggleBlockTag(b.raw_text, tagName, fillNames);
      const props = parseProperties(newRaw);
      delete props.tags;
      return {
        ...b,
        raw_text: newRaw,
        text: (newRaw.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
        tags: getBlockTags(newRaw),
        properties: props,
      };
    });
    saveBlocks(blocks);
  }

  // Listen for "leader → Y" → copy focused block's raw_text to OS clipboard.
  onMount(() => {
    const handler = async () => {
      const block = focusedIndex !== null ? visibleBlocks[focusedIndex] : null;
      if (!block) return;
      try {
        await navigator.clipboard.writeText(block.raw_text);
      } catch (e) {
        console.warn("[tesela] clipboard write failed", e);
      }
    };
    document.addEventListener("tesela:yank-clipboard", handler);
    return () => document.removeEventListener("tesela:yank-clipboard", handler);
  });

  // Modal-close focus restore: ⌘K / leader-menu / slash menu dispatch this
  // when they close without navigating, so the cm-editor for the previously
  // focused block regains DOM focus and j/k routes back here. Scope the
  // query to this outliner's root, and only respond if THIS outliner was
  // the most recently active one — otherwise every BlockOutliner on the
  // journal page would race to refocus and the bottom-most one would win.
  onMount(() => {
    const handler = () => {
      if (focusedIndex === null) return;
      if (!rootEl || !isLastActiveOutliner(rootEl)) return;
      const idx = focusedIndex;
      requestAnimationFrame(() => {
        const cm = rootEl?.querySelector<HTMLElement>(
          `[data-block-vi="${idx}"] .cm-editor .cm-content`,
        );
        cm?.focus();
      });
    };
    document.addEventListener("tesela:restore-focus", handler);
    return () => document.removeEventListener("tesela:restore-focus", handler);
  });

  let rootEl = $state<HTMLDivElement | undefined>();
  let ctxMenu = $state<{ x: number; y: number; blockId: string; blockText: string; blockNoteId: string } | null>(null);

  // Phase 9.7 — Cmd+Z / Cmd+Shift+Z inside cm-editors route here so the
  // unified outliner+insert-session undo stack drives the redo cycle, not
  // cm6's per-keystroke history. Only respond when this outliner contains
  // the currently focused element — there may be multiple BlockOutliner
  // instances on screen (column-view split or JournalView).
  onMount(() => {
    const handles = (fn: () => boolean) => () => {
      const active = document.activeElement;
      if (!(active instanceof HTMLElement)) return;
      if (!rootEl?.contains(active)) return;
      fn();
    };
    const undoHandler = handles(undoOutliner);
    const redoHandler = handles(redoOutliner);
    document.addEventListener("tesela:outliner-undo", undoHandler);
    document.addEventListener("tesela:outliner-redo", redoHandler);
    return () => {
      document.removeEventListener("tesela:outliner-undo", undoHandler);
      document.removeEventListener("tesela:outliner-redo", redoHandler);
    };
  });

  // Phase 10.2 — leader-menu "block" submenu dispatches `tesela:block-action`
  // events. Multiple BlockOutliner instances may be mounted at once (column-
  // view split, JournalView with N daily sections), so we gate by checking
  // whether `rootEl` contains the current `document.activeElement` —
  // ChordMenu doesn't steal focus, so the cm-content of the originally
  // focused block stays the activeElement at action-fire time. Mirrors the
  // existing pattern used for tesela:outliner-undo.
  onMount(() => {
    const handler = (e: Event) => {
      const active = document.activeElement;
      if (!(active instanceof HTMLElement)) return;
      if (!rootEl?.contains(active)) return;
      const detail = (e as CustomEvent).detail as { kind?: string };
      const kind = detail?.kind;
      if (!kind || focusedIndex === null) return;
      const vi = focusedIndex;
      const block = visibleBlocks[vi];
      if (!block) return;
      switch (kind) {
        case "drillIn":      onDrillIn?.(block.id); break;
        case "foldToggle":   toggleFold(block.id); break;
        // Phase 10.5 — propsToggle no longer toggles inline rendering
        // (properties are always hidden inline now). The Space p leader
        // entry was removed; this case stays as a no-op so older
        // dispatches (cached emit calls, plugin code) don't error.
        case "propsToggle":  break;
        case "statusCycle":  handleStatusCycle(vi); break;
        case "delete":       handleDeleteBlock(vi); break;
        case "yank":         handleYankBlock(vi); break;
      }
    };
    document.addEventListener("tesela:block-action", handler);
    return () => document.removeEventListener("tesela:block-action", handler);
  });

  // Tag-picker overlay state for visual-mode bulk tag toggle.
  let showBulkTagPicker = $state(false);
  function openBulkTagPicker() {
    if (!blockVisualMode) return;
    showBulkTagPicker = true;
  }
  const bulkTagOptions = $derived.by(() => {
    return allNotes
      .filter((n) => n.metadata.note_type === "Tag")
      .map((n) => ({ id: n.id, label: n.title }));
  });
</script>

{#if visibleBlocks.length === 0}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="text-sm text-muted-foreground cursor-text py-2 hover:bg-accent/20 rounded px-2"
    onclick={() => {
      pushUndo();
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
      autoFocused = false;
      restoredFocus = false;
      markRecentlyCreated(newBlock.id);
    }}
  >
    Click to start writing…
  </div>
{:else}
  <div class="space-y-0" bind:this={rootEl}>
    {#each visibleBlocks as block, vi (block.id)}
      {@const displayIndent = block.indent_level - drillRootIndent}
      <div
        data-block-vi={vi}
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

        <!-- Fold chevron — only present when the block has children. Spacer
             keeps the bullet column aligned across rows. -->
        {#if hasChildren(block)}
          <!-- svelte-ignore a11y_consider_explicit_label -->
          <button
            class="shrink-0 pt-[12px] pl-1 cursor-pointer text-muted-foreground/40 hover:text-foreground/80 transition-colors {focusedIndex === vi || collapsedBlocks.has(block.id) ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}"
            onclick={(e) => { e.stopPropagation(); toggleFold(block.id); }}
            title={collapsedBlocks.has(block.id) ? "Unfold" : "Fold"}
          >
            {#if collapsedBlocks.has(block.id)}
              <IconChevronRight size={11} stroke={2} />
            {:else}
              <IconChevronDown size={11} stroke={2} />
            {/if}
          </button>
        {:else}
          <span class="shrink-0 pl-1 w-[15px]"></span>
        {/if}

        <!-- Bullet — click to drill in. Two styles via prefs.bulletStyle:
             "dot" (Logseq-like 5px circle) or "arrow" (chevron). pt tuned
             so the visual midpoint matches the cm-line geometric midpoint
             at the editor's default font size. -->
        <!-- svelte-ignore a11y_consider_explicit_label -->
        <button
          class="shrink-0 pl-1 pr-1.5 cursor-pointer transition-opacity {prefs.bulletStyle === 'dot' ? 'pt-[14px]' : 'pt-[10px]'}"
          onclick={(e) => { e.stopPropagation(); onDrillIn?.(block.id); }}
          oncontextmenu={(e) => {
            e.preventDefault();
            ctxMenu = {
              x: e.clientX,
              y: e.clientY,
              blockId: block.id,
              blockText: block.raw_text ?? "",
              blockNoteId: block.note_id,
            };
          }}
          title="Drill in (right-click for more)"
        >
          {#if prefs.bulletStyle === "dot"}
            <span class="block w-[5px] h-[5px] rounded-full transition-colors {focusedIndex === vi ? 'bg-primary' : 'bg-muted-foreground/40 hover:bg-muted-foreground/80'}"></span>
          {:else}
            <IconChevronRight size={12} stroke={2} class="transition-colors {focusedIndex === vi ? 'text-primary' : 'text-muted-foreground/40 hover:text-foreground/80'}" />
          {/if}
        </button>

        <!-- Status indicator (Logseq-style: between bullet and text). Only
             shown when status is set or any tag in the chain declares it. -->
        {#if shouldShowStatus(block)}
          <!-- svelte-ignore a11y_consider_explicit_label -->
          <button
            class="shrink-0 pt-[10px] pr-1.5 cursor-pointer hover:opacity-100 transition-opacity {block.properties.status ? 'opacity-90' : 'opacity-50'}"
            onclick={(e) => { e.stopPropagation(); handleStatusCycle(vi); }}
            title={block.properties.status ? `Status: ${block.properties.status} · click to cycle` : "Click to set status"}
          >
            <span class="block text-[12px] leading-none font-mono w-[14px] text-center {block.properties.status ? statusColorClass(block.properties.status) : 'text-muted-foreground/60'}">
              {block.properties.status ? statusChar(block.properties.status) : "○"}
            </span>
          </button>
        {/if}

        <!-- Content -->
        <div class="flex-1 min-w-0 py-1">
          <BlockEditor
            initialText={block.raw_text}
            onblur={() => {}}
            onfocus={() => {
              focusedIndex = vi;
              autoFocused = false;
              // Cross-day j/k navigation arms a one-shot flag so we land
              // in NORMAL mode on the target block; otherwise it's a
              // user-initiated focus and the auto-INSERT-on-empty rule
              // applies (`restoredFocus = false`).
              restoredFocus = consumeCrossNavFocus();
              if (!isPinnedTab) setLastActiveOutliner(rootEl ?? null);
            }}
            onchange={(text) => handleBlockChange(block.id, text)}
            onnavigate={handleNavigate}
            onescape={() => {}}
            onenter={(textAfter: string) => handleEnter(vi, textAfter)}
            onindent={(dir) => handleIndent(vi, dir)}
            onbackspaceempty={() => handleBackspace(vi)}
            onbackspacemerge={(text: string) => handleBackspaceMerge(vi, text)}
            initialCursorPos={mountHint?.blockId === block.id ? mountHint.pos : undefined}
            startininsert={(mountHint?.blockId === block.id && mountHint.startInInsert) || (block.id === recentlyCreatedBlockId && block.raw_text === "")}
            autofocused={autoFocused}
            onleader={onLeader}
            oncyclestatus={() => blockVisualMode ? bulkCycleStatus() : handleStatusCycle(vi)}
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
            onbulktagpicker={openBulkTagPicker}
            onbulkindent={(dir) => bulkIndent(dir)}
            ontogglefold={() => toggleFold(block.id)}
            ontoggleprops={() => {}}
            onpagejump={handlePageJump}
            inVisualMode={blockVisualMode}
            focused={focusedIndex === vi}
            noteslist={notesList}
            statusChoices={statusChoices}
            hiddenKeys={hiddenKeysFor(block)}
            primaryTag={block.tags[0] ?? block.inherited_tags[0] ?? null}
            autoFillNames={autoFillNamesForTag}
            propertyDefs={propertyDefsFor(block)}
            onInsertTemplate={(templateNoteId) => insertTemplateAfter(block.id, templateNoteId)}
            onUndoOutliner={undoOutliner}
            onRedoOutliner={redoOutliner}
            onBeginInsertSession={beginInsertSession}
            onEndInsertSession={endInsertSession}
          />
        </div>

        <!-- Display chips (right side, before tags) — Phase 10.5
             configurable per-tag pills surfacing selected property values
             (deadline, priority, …). Drawn from each tag's `display_chips`
             frontmatter array; values come from the block's parsed
             properties. Skipped entirely when value is empty/unset.
             Phase 12.4 prepends synthetic chips: subtask rollup ("3/5"),
             blocked-by lock — both purely visual, computed from the
             block list, no markdown footprint. -->
        {#if subtaskRollup(block) || isBlocked(block) || displayChipsFor(block).length > 0}
          {@const _rollup = subtaskRollup(block)}
          {@const _blocked = isBlocked(block)}
          {@const chips = displayChipsFor(block)}
          <div class="shrink-0 flex items-center gap-1 self-center pr-1 py-1">
            {#if _blocked}
              <span
                class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-full bg-amber-500/10 text-amber-500/90 font-medium"
                title="Blocked by an unfinished task"
              >
                <IconLock size={11} stroke={2} />
                <span>blocked</span>
              </span>
            {/if}
            {#if _rollup}
              <span
                class="inline-flex items-center gap-1 text-[10px] px-1.5 py-0.5 rounded-full font-medium {_rollup.done === _rollup.total ? 'bg-emerald-500/10 text-emerald-500/90' : 'bg-muted text-muted-foreground/80'}"
                title="{_rollup.done} of {_rollup.total} subtasks done"
              >
                <IconChecklist size={11} stroke={2} />
                <span>{_rollup.done}/{_rollup.total}</span>
              </span>
            {/if}
            {#each chips as chip}
              <DisplayChip propKey={chip.key} value={chip.value} def={chip.def} />
            {/each}
          </div>
        {/if}

        <!-- Tag pills (right side) -->
        {#if block.tags.length > 0}
          <div class="shrink-0 flex items-center gap-1 self-center pr-2 py-1">
            {#each block.tags as tag}
              <span class="group/tag inline-flex items-center gap-0.5 text-[10px] px-1.5 py-0.5 rounded-full bg-primary/10 text-primary/70 font-medium">
                <a
                  href="/p/{tag.toLowerCase()}"
                  class="hover:text-primary transition-colors"
                  onclick={(e) => e.stopPropagation()}
                >{tag}</a>
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

      <!-- Inline query results (when block has a query:: property) -->
      {#if block.properties.query}
        <div style="padding-left: {displayIndent * 24}px;">
          <QueryBlock {block} onUpdate={(t) => handleBlockChange(block.id, t)} />
        </div>
      {/if}

      <!-- Inline collection (manual block-ref list) -->
      {#if block.properties.collection !== undefined}
        <div style="padding-left: {displayIndent * 24}px;">
          <CollectionBlock {block} onUpdate={(t) => handleBlockChange(block.id, t)} />
        </div>
      {/if}
    {/each}
  </div>
{/if}

{#if ctxMenu}
  <ContextMenu
    x={ctxMenu.x}
    y={ctxMenu.y}
    onclose={() => ctxMenu = null}
    items={[
      {
        label: "Pin to drawer",
        action: () => {
          const preview = ctxMenu!.blockText.trim().slice(0, 40) || "(empty)";
          const id = pinBlock(ctxMenu!.blockNoteId, ctxMenu!.blockId, preview);
          setBottomDrawerOpen(true);
          setBottomTab({ kind: "pinned", id });
        },
      },
    ]}
  />
{/if}

<!-- Bulk tag picker overlay (visual mode bulk-tag op) -->
{#if showBulkTagPicker}
  <div
    class="fixed inset-0 z-50 flex items-start justify-center pt-[20vh] bg-black/30"
    onclick={() => { showBulkTagPicker = false; }}
    onkeydown={(e) => { if (e.key === "Escape") { showBulkTagPicker = false; } }}
    role="dialog"
    aria-label="Bulk tag picker"
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="w-72 max-h-[60vh] overflow-y-auto bg-popover border border-border rounded-md shadow-xl p-2"
      onclick={(e) => e.stopPropagation()}
    >
      <div class="text-[11px] text-muted-foreground/60 px-2 py-1 mb-1">
        Toggle tag on {visualRange.size} blocks
      </div>
      {#each bulkTagOptions as opt (opt.id)}
        <button
          class="w-full text-left text-[12px] px-2 py-1.5 rounded hover:bg-muted/40 transition-colors"
          onclick={() => { bulkToggleTag(opt.label); showBulkTagPicker = false; }}
        >{opt.label}</button>
      {/each}
      {#if bulkTagOptions.length === 0}
        <div class="text-[11px] text-muted-foreground/40 italic px-2 py-1.5">No Tag pages defined</div>
      {/if}
    </div>
  </div>
{/if}
