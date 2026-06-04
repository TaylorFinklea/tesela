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
  import { onMount, onDestroy } from "svelte";
  import { createQuery } from "@tanstack/svelte-query";
  import { parseBlocks } from "$lib/block-parser";
  import { toggleBlockTag, getBlockTags } from "$lib/block-tags";
  import { api } from "$lib/api-client";
  import {
    upsertOpForBlock,
    upsertOpForStructuralBlock,
    mergeOpsForBackspace,
    moveOpsForIds,
    deleteOpsFor,
    diffOpsForSnapshot,
    type BlockOp,
  } from "$lib/block-ops";
  import { BlockOpsSaver } from "$lib/block-ops-saver";
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
  import BlockDateRow from "./BlockDateRow.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import type { HiddenKeysConfig } from "$lib/cm-decorations";
  import { prefs } from "$lib/preferences.svelte";
  import { OutlinerHistory, type OutlinerSnapshot } from "$lib/stores/outliner-history.svelte";
  import { pinBlock, setBottomDrawerOpen, setBottomTab } from "$lib/stores/pane-state.svelte";
  import { togglePinBlock as v5TogglePinBlock } from "$lib/state/shared.svelte";
  import {
    registerPaneOutliner,
    unregisterPaneOutliner,
  } from "$lib/stores/pane-tree.svelte";

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
    paneId,
  }: {
    noteId: string;
    body: string;
    frontmatter: string;
    /** Whole-note save callback (the debounced parent PUT). `baseContent` is
     *  the body the editor last reseeded from for this note — the author's
     *  edit BASE — threaded so the parent can send `base_content` on the PUT
     *  and the server diffs the author's REAL changes (base→new), never re-
     *  asserting an untouched block over a concurrent peer edit. Always passed
     *  by this component; older parents that ignore it stay correct (just
     *  base-less, i.e. today's behaviour). */
    onContentChange?: (fullContent: string, baseContent?: string) => void;
    /** Cancel any pending/in-flight save and PUT immediately. Called from
     *  `applySnapshot` so undo/redo restored bodies win the race against any
     *  in-flight pre-undo PUT. Falls back to the debounced `onContentChange`
     *  path if the parent didn't wire it. `baseContent` is the edit BASE (see
     *  `onContentChange`). */
    onCancelAndFlush?: (fullContent: string, baseContent?: string) => void;
    onleader?: () => void;
    onfocusedblockchange?: (block: ParsedBlock | null) => void;
    drillBlockId?: string;
    onDrillIn?: (blockId: string) => void;
    isPinnedTab?: boolean;
    /** Prism v4 — id of the pane this outliner lives in. When set, the
     *  outliner registers its root element in the pane-tree's outliner
     *  registry so later phases can route events to "the outliner in
     *  pane X". Unset for the legacy chrome (column-view, journal, etc). */
    paneId?: string;
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

  // A blank seed block so every note — even an empty one — presents one
  // editable, focusable bullet (how outliners work), instead of a
  // click-to-create placeholder. The seed is LOCAL-ONLY (`:new-` id, so
  // applyExternalReparse won't drop it) and is NOT persisted until the
  // user actually types — saveBlocks only runs from edit handlers, never
  // from seeding — so empty days stay zero-byte on disk until written.
  function seedEmptyBlock(nid: string): ParsedBlock {
    return {
      id: `${nid}:new-seed`,
      bid: crypto.randomUUID(),
      text: "",
      raw_text: "",
      tags: [],
      inline_tags: [],
      trailing_tags: [],
      inherited_tags: [],
      properties: {},
      indent_level: 0,
      note_id: nid,
      parent_note_type: null,
    } as ParsedBlock;
  }
  function parseBlocksSeeded(nid: string, b: string): ParsedBlock[] {
    const parsed = parseBlocks(nid, b);
    return parsed.length > 0 ? parsed : [seedEmptyBlock(nid)];
  }

  let blocks = $state<ParsedBlock[]>(parseBlocksSeeded(noteId, body));
  let focusedIndex = $state<number | null>(null);
  let lastExternalBody = $state(body);
  let lastSentBody = $state(body);
  /** Wall-clock ms of the last local keystroke / structural edit
   *  (new block, indent, etc.). The body-sync effect defers any
   *  incoming server reparse for ~1200ms after this — long enough
   *  that the user's PUT round-trips and `lastSentBody` catches up,
   *  so the next reparse is a no-op. Without the defer, a WS event
   *  from any OTHER source (iOS background sync; another browser
   *  tab) that arrives mid-typing replaces the focused block's text
   *  with the server's pre-keystroke view + drops any in-flight new
   *  blocks the user just created with Enter. Symptoms Daisy saw:
   *  "cursor hijacked, last character deleted, new block line undone." */
  let lastLocalEditAt = $state(0);
  /** Pending deferred reparse — the body we WOULD have applied if
   *  the user hadn't been mid-typing. Held here so we can flush it
   *  once typing settles instead of dropping the server update on
   *  the floor. */
  let deferredReparseBody: string | null = $state(null);
  let deferredReparseTimer: ReturnType<typeof setTimeout> | null = null;
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
    // Capture the pre-restore tree BEFORE reassigning `blocks` so the
    // undo/redo save can diff prev→restored into block ops (and avoid the
    // whole-body PUT that would re-assert every surviving block).
    const prevBlocks = blocks;
    blocks = s.blocks.map((b) => ({ ...b }));
    focusedIndex = s.focusedIndex;
    // Suppress the empty-block→Insert remount heuristic for the next render
    // tick so a redo/undo that lands on an empty block stays in Normal.
    restoredFocus = true;
    collapsedBlocks = new Set(s.collapsedBlocks);
    // Cancel any in-flight pre-undo PUT and persist the restored state. Prefer
    // a block-ops diff (prev→restored) so only the blocks the restore actually
    // changed are touched — a concurrent peer edit to an untouched block then
    // survives. Falls back to an immediate whole-body PUT (WITH base) only when
    // the diff can't be expressed (a local-only block on either side).
    saveSnapshotRestore(prevBlocks, blocks);
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

  /** Locally-created block ids have a `:new-<timestamp>` or `:paste-`
   *  suffix; they exist in `blocks` but not in any server-canonical
   *  body until the next PUT round-trips back as a WS broadcast. We
   *  use this to distinguish "in-flight local block" from "remote
   *  delete" in `applyExternalReparse` — the former must NEVER be
   *  dropped, the latter is the user's intent. Server-side ids carry
   *  `<noteId>:<lineNumber>` shape (no `new-` or `paste-` infix). */
  function isLocalOnlyId(id: string): boolean {
    return id.includes(":new-") || id.includes(":paste-");
  }

  /** True when the editor holds local structural edits the server hasn't
   *  confirmed yet: any `:new-` / `:paste-` block is present in `blocks` but
   *  not in any server-canonical body until its PUT round-trips back as a WS
   *  broadcast (the echo carries the same block re-serialised with a
   *  canonical `<noteId>:<line>` id, so the local-only id is gone).
   *
   *  This is the dirty signal the body-sync reseed must respect — unlike the
   *  per-focused-block guard in `applyExternalReparse`, it holds even after
   *  the user moves focus OFF a freshly-added bullet onto an existing block.
   *  That focus-moved case is exactly how the daily LIST query (the broad,
   *  un-suppressed refresh) reseeds `body` and drops not-yet-saved new
   *  bullets — mirrors the iOS `pendingRemoteRefresh`/`isEditingBlock` defer.
   *
   *  Self-clearing (no "dirty forever" deadlock): the only thing that mints
   *  local-only ids is the user adding blocks; the user's own debounced save
   *  PUTs them, and the server's echo (byte-identical to `lastSentBody`)
   *  reaches `applyExternalReparse` via the `targetBody === lastSentBody`
   *  fast-path that bypasses this guard entirely, so the held state resolves
   *  the moment our save round-trips. */
  function hasUnsavedLocalEdits(): boolean {
    return blocks.some((b) => isLocalOnlyId(b.id));
  }

  /** Apply an external body reparse. Extracted from the $effect below
   *  so the deferred-flush timer can call it without re-triggering
   *  the effect with stale dependencies. */
  function applyExternalReparse(targetBody: string) {
    // **Dirty guard.** Hold any remote reseed while the editor has unsaved
    // local structural edits (new bullets not yet round-tripped, or a save
    // still in flight). Re-arm the deferred timer so the held body is RE-
    // TRIED once the editor goes clean — never dropped on the floor. Without
    // this, a daily LIST-query refetch (the broad refresh, which is NOT
    // own-echo-suppressed) reseeds `body` with the pre-edit server view and
    // clobbers brand-new bullets the user just added, even after they've
    // moved focus off the new block. The genuine-remote-update path is
    // preserved: when the editor is clean this is a no-op and the reseed
    // applies immediately.
    if (targetBody !== lastSentBody && hasUnsavedLocalEdits()) {
      deferredReparseBody = targetBody;
      if (deferredReparseTimer) clearTimeout(deferredReparseTimer);
      deferredReparseTimer = setTimeout(() => {
        deferredReparseTimer = null;
        const pending = deferredReparseBody;
        deferredReparseBody = null;
        if (pending !== null) applyExternalReparse(pending);
      }, 400);
      return;
    }
    lastExternalBody = targetBody;
    if (targetBody === lastSentBody) return;
    const reparsed = parseBlocksSeeded(noteId, targetBody);
    if (focusedIndex === null) {
      blocks = reparsed;
      history.clear();
      return;
    }
    // **In-flight new-block protection.** If the focused block has a
    // local-only id (just created via Enter or paste), it cannot be
    // in the server-canonical body yet — the PUT carrying it is in
    // flight or queued behind the debounce. Adopting `reparsed` here
    // would drop the new block (Daisy reported: "still deletes my new
    // line block almost instantly"). Skip this reparse entirely; the
    // next body change after the PUT round-trips will be a no-op
    // because by then `body === lastSentBody`.
    const focusedId = blocks[focusedIndex]?.id ?? "";
    if (isLocalOnlyId(focusedId) && targetBody !== lastSentBody) {
      // Mark the body we couldn't apply so the deferred timer doesn't
      // fire on it either. (It'll keep being re-checked as new WS
      // events arrive and update `body`.)
      return;
    }
    const newIdx = focusedId ? reparsed.findIndex((b) => b.id === focusedId) : -1;
    if (newIdx === -1) {
      // Focused block disappeared. If it was a local-only id we
      // already returned above; reaching here means the server-canonical
      // block is genuinely gone (remote delete, or local delete whose
      // server round-trip just landed). Keep the cursor visible by
      // clamping the old index into the new list.
      blocks = reparsed;
      if (reparsed.length === 0) focusedIndex = null;
      else focusedIndex = Math.min(Math.max(focusedIndex, 0), reparsed.length - 1);
      history.clear();
      return;
    }
    const localFocused = blocks[focusedIndex];
    const merged = reparsed.map((b, i) => i === newIdx ? {
      ...b,
      raw_text: localFocused.raw_text,
      text: localFocused.text,
      tags: localFocused.tags,
      properties: localFocused.properties,
    } : b);
    blocks = merged;
    if (newIdx !== focusedIndex) focusedIndex = newIdx;
    history.clear();
  }

  $effect(() => {
    const noteChanged = noteId !== lastBodyNoteId;
    if (!noteChanged && body === lastExternalBody) return;
    // The outliner is about to switch notes (drill / Esc-back within the same
    // instance) and discard the old note's block state. Flush its pending
    // coalesced block-ops first so an un-fired debounce timer doesn't lose the
    // last edit to the note we're leaving.
    if (noteChanged) blockOpsSaver.flush(lastBodyNoteId);
    lastBodyNoteId = noteId;
    if (noteChanged) {
      lastExternalBody = body;
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
    // Mid-typing reparse protection. If the user has typed (or
    // pressed Enter for a new block, etc.) within the last 1.2s, a
    // reparse triggered by an *unrelated* WS event (iOS background
    // sync, another tab) would clobber the in-flight local edit
    // because the server's view doesn't include those keystrokes
    // yet. Defer this reparse to a timer that fires after typing
    // settles; the next typed character keeps pushing the deadline
    // out, so as long as the user is actively typing we never apply
    // a stale server reparse. The MERGE branch in applyExternalReparse
    // is still a safety net for the focused block, but defer is
    // strictly better when we can do it.
    const cooldownMs = 1200;
    const sinceEdit = Date.now() - lastLocalEditAt;
    if (sinceEdit < cooldownMs && focusedIndex !== null) {
      deferredReparseBody = body;
      if (deferredReparseTimer) clearTimeout(deferredReparseTimer);
      deferredReparseTimer = setTimeout(() => {
        deferredReparseTimer = null;
        const pending = deferredReparseBody;
        deferredReparseBody = null;
        if (pending !== null) applyExternalReparse(pending);
      }, cooldownMs - sinceEdit + 50);
      return;
    }

    applyExternalReparse(body);
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
        // Strip any stray bid marker that crept into raw_text (rare but
        // possible if the editor accepted a paste containing one) before
        // we re-emit the bid below — without this we could end up with
        // two bid markers on the same line.
        const firstText = (lines[0] ?? "").replace(/\s*<!--\s*bid:[0-9a-fA-F-]{32,36}\s*-->/g, "");
        // Re-emit the bid marker if the block has one. Brand-new local
        // blocks (no bid yet) get stamped by the server's
        // `stamp_block_ids` pass on receipt.
        const bidSuffix = b.bid ? ` <!-- bid:${b.bid} -->` : "";
        const first = `${indent}- ${firstText}${bidSuffix}`;
        const rest = lines.slice(1).map((l: string) => `${indent}  ${l}`);
        return [first, ...rest].join("\n");
      })
      .join("\n");
    return { full: `${frontmatter}${bodyLines}\n`, bodyOnly: `${bodyLines}\n` };
  }

  /** The full-note edit BASE for a whole-body PUT's `base_content`: the
   *  current `frontmatter` plus the body the editor last reseeded from
   *  (`lastExternalBody`). Mirrors how `buildFullContent` prefixes the
   *  frontmatter so `base_content` and `content` are diffed on the same
   *  whole-note shape server-side (frontmatter identical in both → only the
   *  author's real block-text changes are emitted). `lastExternalBody` is the
   *  ONLY body-state set exclusively from a server-canonical reparse — never
   *  optimistically to the new local body — so it is genuinely "what the
   *  author started this edit from", the correct base for the author-change
   *  diff. Returns `undefined` only before any body has been seeded (defensive;
   *  in practice `lastExternalBody` is initialised to the `body` prop). */
  function baseForPut(): string | undefined {
    if (lastExternalBody === undefined || lastExternalBody === null) return undefined;
    return `${frontmatter}${lastExternalBody}`;
  }

  function saveBlocks(updated: ParsedBlock[]) {
    const { full, bodyOnly } = buildFullContent(updated);
    // Capture the edit BASE (the body we last reseeded from) BEFORE advancing
    // `lastSentBody` to the new body. `lastExternalBody` is the only state set
    // exclusively from a server-canonical body (`applyExternalReparse` /
    // noteId change) and never optimistically to the local new body — so it is
    // genuinely "what the author started this edit from". The parent forwards
    // it as `base_content` so the server base-diffs the author's real changes,
    // never re-asserting an untouched block over a concurrent peer edit.
    const base = baseForPut();
    lastSentBody = bodyOnly;
    // Any direct whole-body PUT (structural edits: backspace-delete/merge,
    // Enter-split, paste, dd) SUPERSEDES a pending coalesced block-ops batch
    // for this note: the PUT body is built from the same accumulated `blocks`,
    // so it already carries the queued text edits. Cancel the pending ops
    // (timer + any in-flight POST, no flush) so we don't double-send — one
    // path per save. `supersedeWithBody` is a no-op cancel when nothing is
    // pending, so the common (no prior typing) case is unaffected.
    blockOpsSaver.supersedeWithBody(noteId, () => onContentChange?.(full, base));
  }

  /** Block-granular save path (sync redesign 2026-06-02). Sends ONLY the
   *  supplied ops to `POST /notes/{id}/blocks`, which never re-asserts blocks
   *  the user didn't touch — the structural fix for the whole-body clobber.
   *
   *  **Dual-write-path invariant.** This path is mutually exclusive with the
   *  whole-body PUT (`saveBlocks` → `onContentChange` → parent debounce): it
   *  deliberately does NOT call `onContentChange`, so a text/indent edit never
   *  also enqueues a body PUT for the same note+window. It DOES advance
   *  `lastSentBody` to the post-edit body so the own-echo dirty-guard
   *  (`applyExternalReparse`'s `targetBody === lastSentBody` fast-path) still
   *  recognises the server's echo of THIS write and converges cleanly — the
   *  same contract `saveBlocks` upholds. Both paths call `recordLocalSave`
   *  (inside `api.upsertBlocks` / `api.updateNote`).
   *
   *  `ops` may contain `null` entries (a block with no server bid, or a
   *  brand-new local-only insert that isn't a block-op candidate yet). If ANY
   *  entry is `null` the operation can't be fully expressed block-granularly,
   *  so — to keep one-path-per-save and never silently drop a sub-edit — the
   *  ENTIRE save falls back to the whole-body PUT (`saveBlocks`). The common
   *  case (a real on-disk block) sends a clean ops batch and never PUTs.
   *
   *  Synthetic (not-yet-on-disk) days are caught by the same gate: their seed
   *  block is local-only, so its first edit yields a `null` op → whole-body
   *  create/PUT path materialises the file, after which subsequent in-place
   *  edits carry a canonical id and flow block-granularly. */
  function saveBlocksViaOps(updated: ParsedBlock[], ops: (BlockOp | null)[]) {
    // Advance the dirty-guard baseline to the post-edit body so the server's
    // echo of this write is recognised as our own and applies cleanly. Both
    // the ops path and the PUT fallback below converge to this same body.
    const { bodyOnly } = buildFullContent(updated);
    lastSentBody = bodyOnly;
    // Capture the note this save belongs to: `noteId` can change under us
    // (drill / Esc-back) before a debounced flush fires, so the coalesced POST
    // must target the note that was current when the edit happened.
    const targetNoteId = noteId;
    // **Brand-new-note guard (Stage 3).** `POST /notes/{id}/blocks` 404s when
    // the note has no file on disk yet — a synthetic daily the user just typed
    // into hasn't been materialized. The whole-body PUT path is the ONLY one
    // that creates the note (JournalView's `needsCreate` lazy-create, keyed off
    // the per-day `isSynthetic` closure on `onContentChange`). A note that has
    // never round-tripped carries ZERO server-canonical block ids: the parser
    // mints `${noteId}:${lineNumber}` ids (numeric trailing segment), whereas
    // every client-side insert id (`:new-`/`:paste-`/`:split-`/`:merged-`/
    // `:tmpl-`/seed) has a non-numeric trailing segment. (`isLocalOnlyId` only
    // covers `:new-`/`:paste-`, so a split/merge on a brand-new note would slip
    // past it — hence the stricter numeric-id check here.) If NO block is
    // canonical, force the whole-body PUT so the note is created first; the next
    // structural edit (once the refetch reseeds line-based ids) flows block-
    // granularly.
    if (!updated.some((b) => /:\d+$/.test(b.id))) {
      saveBlocks(updated);
      return;
    }
    if (ops.length === 0 || ops.some((o) => o === null)) {
      // Not fully block-granular (no eligible op, or a mixed batch containing a
      // not-yet-saved local block). Use the whole-body PUT for the whole edit so
      // nothing is dropped — but now WITH the edit base (`saveBlocks` →
      // `onContentChange(full, base)`), so even this fallback diffs the author's
      // real changes server-side and never re-asserts an untouched block over a
      // concurrent peer edit. `saveBlocks` supersedes any pending coalesced
      // block-ops batch (cancels its timer + in-flight POST) so we never double-
      // send. One path per save.
      saveBlocks(updated);
      return;
    }
    const concrete = ops as BlockOp[];
    // Coalesce rapid same-note edits into one trailing-edge POST; abort the
    // superseded in-flight POST. The flush passes the controller's signal to
    // `api.upsertBlocks` and swallows the resulting AbortError (expected, not
    // a failure — no PUT fallback on abort, which would double-write). The
    // genuine-failure fallback PUTs the latest accumulated body.
    blockOpsSaver.enqueue(targetNoteId, concrete);
  }

  /** Block-granular delete path (sync redesign 2026-06-02, S4 — closes the
   *  LAST whole-body-PUT clobber path). A pure deletion (backspace into an
   *  empty block, `dd`, visual-mode multi-delete) used to PUT the entire note
   *  body (re-asserting every surviving block — stale, clobber-prone) AND fire
   *  a separate `api.deleteBlock`. This sends ONLY `{kind:"delete", bid}` ops
   *  for the removed blocks the server has seen, so no surviving block is
   *  re-asserted — a concurrent peer edit to one of them survives.
   *
   *  `updated` is the post-removal block tree (advances the own-echo dirty-
   *  guard baseline, exactly like `saveBlocksViaOps`). `deleted` is the set of
   *  `ParsedBlock`s removed by this op (their `bid`/`id` are read to build the
   *  delete ops). A removed block that was never round-tripped (local-only id
   *  or no `bid`) yields NO op — dropping it locally is the whole deletion —
   *  and when EVERY removed block is local-only the batch is empty, in which
   *  case nothing is sent server-side and (critically) we do NOT fall back to
   *  the whole-body PUT. One path per save: delete ops flow through the same
   *  coalescing saver as text/indent edits (`recordLocalSave` fires inside
   *  `api.upsertBlocks`). */
  function saveDeletesViaOps(updated: ParsedBlock[], deleted: ParsedBlock[]) {
    // Advance the dirty-guard baseline to the post-delete body so the server's
    // echo of this write is recognised as our own and applies cleanly.
    const { bodyOnly } = buildFullContent(updated);
    lastSentBody = bodyOnly;
    const targetNoteId = noteId;
    const ops = deleteOpsFor(deleted);
    // Every removed block was local-only (never on the server): the local
    // removal IS the whole deletion. Sending nothing — and NOT a whole-body
    // PUT — is correct; a PUT would re-assert every surviving block.
    if (ops.length === 0) return;
    // Coalesce with any pending same-note edits into one trailing-edge POST.
    blockOpsSaver.enqueue(targetNoteId, ops);
  }

  /** One coalescing saver per outliner instance. The flush POSTs the latest
   *  coalesced ops to `api.upsertBlocks(noteId, ops, signal)`; on a genuine
   *  (non-abort) failure it PUTs the latest body via `saveBlocks` so the edit
   *  still persists. `lastSentBody` already tracks the latest post-edit body
   *  (advanced on every `saveBlocksViaOps` call), so the fallback re-PUTs the
   *  current `blocks` — the same converged state the ops would have produced.
   *  The PUT now carries the edit base (`saveBlocks` → `onContentChange(full,
   *  base)`), so a retry after a failed POST diffs the author's real changes
   *  and can't clobber a concurrent peer edit. */
  const blockOpsSaver = new BlockOpsSaver(
    (targetNoteId, ops, signal) => api.upsertBlocks(targetNoteId, ops, signal),
    (targetNoteId) => {
      // Loss-avoidance fallback for a genuine (non-abort) POST failure. Only
      // PUT when the failed note is still the one on screen — `blocks` holds
      // the current note's body, so PUTting it after the outliner has switched
      // notes would write the wrong note. (A stale note's failed flush is rare
      // — it would need a note switch to race a network error — and the
      // server's converged state plus the next edit reconcile it.)
      if (targetNoteId !== noteId) {
        console.warn("upsertBlocks failed for a note no longer on screen; skipping PUT fallback");
        return;
      }
      console.warn("upsertBlocks failed; falling back to whole-body PUT");
      saveBlocks(blocks);
    },
  );

  // Flush any pending coalesced block-ops immediately when the user leaves
  // this outliner (focus moves out) — a save MUST land when the user leaves
  // the block, not wait on an un-fired debounce timer. Mirrors the
  // outlinerHasFocus focusout handler below.
  function flushBlockOpsOnBlur() {
    blockOpsSaver.flush(noteId);
  }

  // Flush on teardown so a destroyed outliner (page nav away, pane close)
  // never loses the last edit to a debounce timer that never fired.
  onDestroy(() => blockOpsSaver.flushAll());

  /** Persist an outliner undo/redo restore (`applySnapshot`). Prefer a block-
   *  ops diff of `prev → restored` so ONLY the blocks the restore actually
   *  changed are written — a concurrent peer edit to an untouched block then
   *  survives (the whole-body PUT this replaces re-asserted every surviving
   *  block from a possibly-stale view, the last clobber vector). Both the ops
   *  path and the PUT fallback advance `lastSentBody` to the restored body so
   *  the server's own-echo of this write applies cleanly.
   *
   *  Falls back to an immediate whole-body PUT (WITH base, so even the fallback
   *  can't clobber) only when the diff can't be expressed block-granularly — a
   *  brand-new local-only block (no server bid) on either side, or a brand-new
   *  note with no canonical block ids yet. The PUT goes through
   *  `onCancelAndFlush` (cancel any in-flight pre-undo PUT, write immediately)
   *  so the restored state wins the WS-echo race; it falls through to the
   *  debounced `onContentChange` when the parent didn't wire `onCancelAndFlush`.
   *  One path per save: either ops OR the PUT, never both. */
  function saveSnapshotRestore(prevBlocks: ParsedBlock[], restored: ParsedBlock[]) {
    const { full, bodyOnly } = buildFullContent(restored);
    // Capture the base BEFORE advancing `lastSentBody` (see `saveBlocks`).
    const base = baseForPut();
    lastSentBody = bodyOnly;
    // Brand-new note (no canonical block id) can't take block ops — POST
    // /blocks 404s until the file exists. Force the whole-body PUT (a create
    // has no peer to clobber; base is sent regardless and harmless).
    const hasCanonical = restored.some((b) => /:\d+$/.test(b.id)) ||
      prevBlocks.some((b) => /:\d+$/.test(b.id));
    const ops = hasCanonical ? diffOpsForSnapshot(prevBlocks, restored) : null;
    if (ops === null) {
      // Not fully block-granular — PUT the whole restored body (with base).
      if (onCancelAndFlush) onCancelAndFlush(full, base);
      else onContentChange?.(full, base);
      return;
    }
    if (ops.length === 0) return; // restore was a no-op (e.g. only fold state).
    // Cancel any pending/in-flight block-ops batch for this note so the
    // restore's ops aren't double-sent alongside a stale coalesced batch, then
    // enqueue the restore diff and flush it immediately (undo/redo must win the
    // WS-echo race against any in-flight pre-undo write, mirroring the old
    // `onCancelAndFlush` immediacy). One path per save.
    blockOpsSaver.supersedeWithBody(noteId, () => {
      blockOpsSaver.enqueue(noteId, ops);
      blockOpsSaver.flush(noteId);
    });
  }

  function handleBlockChange(blockId: string, newRawText: string) {
    // Mark "actively editing" so the body-sync effect knows to defer
    // any incoming server reparse until typing settles. See
    // lastLocalEditAt's docstring for the why.
    lastLocalEditAt = Date.now();
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
    // Block-granular path: an in-place text edit touches exactly ONE block, so
    // emit a single `upsert` op rather than PUTting the whole body (which
    // re-asserts — and clobbers — concurrent peer edits to other blocks).
    // `upsertOpForBlock` returns null when the block has no server bid yet (a
    // brand-new local insert), in which case `saveBlocksViaOps` falls back to
    // the whole-body PUT so the edit still persists. One path per save.
    saveBlocksViaOps(blocks, [upsertOpForBlock(blocks, blockId)]);
  }

  /** C2.3 — a text edit that went through the block's LoroText binding (local
   *  splice broadcast over the WS, OR a remote splice applied into the editor),
   *  NOT the whole-text HTTP path. Update the block's ParsedBlock state so
   *  structure/display/tags stay consistent with the live text, but DO NOT save
   *  (the Loro delta is the persistence/sync path for bound text edits). Mirrors
   *  `handleBlockChange`'s state-only update — minus `saveBlocksViaOps` and the
   *  insert-session undo promotion (Loro owns text history for bound blocks). */
  function handleLoroText(blockId: string, newRawText: string) {
    // Mark "actively editing" so the body-sync effect still defers an unrelated
    // server reparse while the user types into a bound block.
    lastLocalEditAt = Date.now();
    const parsedTags = getBlockTags(newRawText);
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
    // Advance the own-echo dirty-guard baseline so the server's WS echo of the
    // CHANGED note body (the relay re-materializes + broadcasts note_updated for
    // bound splices too) is recognised as our own and doesn't reseed/clobber.
    const { bodyOnly } = buildFullContent(blocks);
    lastSentBody = bodyOnly;
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

  // Phase 13 — `[` / `]` jump to previous / next top-level block. Skips
  // past any nested children, so in a heavily-outlined note the user
  // can land on the next sibling-level item without j-mashing through
  // every leaf. If we're already at the boundary (no further top-level
  // block in that direction), focus stays put; no edge-handoff event,
  // since cross-outliner top-level navigation isn't a natural extension
  // of this affordance.
  function handleNavigateTopLevel(direction: "up" | "down") {
    if (focusedIndex === null) return;
    const start = focusedIndex;
    if (direction === "down") {
      for (let i = start + 1; i < visibleBlocks.length; i++) {
        if (visibleBlocks[i].indent_level === 0) {
          focusedIndex = i;
          restoredFocus = false;
          requestAnimationFrame(() => {
            const el = rootEl?.querySelector(`[data-block-vi="${i}"]`);
            el?.scrollIntoView({ block: "nearest", behavior: "auto" });
          });
          return;
        }
      }
    } else {
      for (let i = start - 1; i >= 0; i--) {
        if (visibleBlocks[i].indent_level === 0) {
          focusedIndex = i;
          restoredFocus = false;
          requestAnimationFrame(() => {
            const el = rootEl?.querySelector(`[data-block-vi="${i}"]`);
            el?.scrollIntoView({ block: "nearest", behavior: "auto" });
          });
          return;
        }
      }
    }
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
      // Fresh bid so each inserted block is a distinct row on the server side.
      // Without this they'd inherit the template note's own bids and collide
      // with the template's rows as the same logical block. Mirrors the fresh
      // bid mint in `handlePasteBlock`.
      bid: crypto.randomUUID(),
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
    // Block-granular path: upsert one op per inserted block by its fresh
    // client-minted bid — NOT a whole-body PUT that re-asserts every block and
    // clobbers concurrent peer edits. MID-note inserts land at the document END
    // on peers in v1 (engine ignores order_key — see `handleEnter`'s caveat);
    // loss-free is the invariant, position is the documented follow-up. One
    // path per save.
    saveBlocksViaOps(
      blocks,
      inserted.map((b) => upsertOpForStructuralBlock(blocks, b.id)),
    );
  }

  function handleEnter(vi: number, textAfterCursor: string = "") {
    lastLocalEditAt = Date.now();
    const current = visibleBlocks[vi];
    if (!current) return;
    const fullIdx = blocks.findIndex(b => b.id === current.id);
    if (fullIdx < 0) return;
    pushUndo();
    // Mint a stable canonical UUID for the bid up front. Without it,
    // the first save would emit a bid-less line, server's
    // stamp_block_ids would assign one, and every SUBSEQUENT save
    // (while web's local state still carries the local-only id and
    // no bid) would trigger another stamp → another BlockUpsert with
    // a different UUID → `apply_block_upsert` would append a new
    // row instead of updating. Stamping client-side breaks the loop.
    const newBlock: ParsedBlock = {
      id: `${noteId}:new-${Date.now()}`,
      bid: crypto.randomUUID(),
      text: (textAfterCursor.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
      raw_text: textAfterCursor,
      tags: [],
      inline_tags: [],
      trailing_tags: [],
      inherited_tags: [],
      properties: {},
      indent_level: current.indent_level,
      note_id: noteId,
      parent_note_type: null,
    };
    let structuralIds: string[];
    if (textAfterCursor) {
      // Split: current keeps its existing bid (inherited via spread);
      // newBlock got its own above. Both blocks are now stable.
      const updatedCurrent: ParsedBlock = { ...current, id: `${noteId}:split-${Date.now() + 1}` };
      mountHint = { blockId: newBlock.id, pos: 0, startInInsert: true };
      blocks = [...blocks.slice(0, fullIdx), updatedCurrent, newBlock, ...blocks.slice(fullIdx + 1)];
      // A split changes TWO blocks: the original's text shrank to the pre-
      // cursor portion, and the new block carries the post-cursor portion.
      // Upsert BOTH so the original's new (shorter) text persists.
      structuralIds = [updatedCurrent.id, newBlock.id];
    } else {
      blocks = [...blocks.slice(0, fullIdx + 1), newBlock, ...blocks.slice(fullIdx + 1)];
      structuralIds = [newBlock.id];
    }
    // Block-granular path: a new block (and, for a split, the edited original)
    // is upserted by its client-minted bid — NOT a whole-body PUT that would
    // re-assert every block and clobber concurrent peer edits. The structural
    // upsert carries an `after_bid` predecessor hint, so a mid-note split's
    // new half lands ADJACENT to its sibling on peers (engine `create_at`),
    // not at document end. One path per save.
    saveBlocksViaOps(
      blocks,
      structuralIds.map((sid) => upsertOpForStructuralBlock(blocks, sid)),
    );
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
    lastLocalEditAt = Date.now();
    const block = visibleBlocks[vi];
    if (!block) return;
    // Outdent at root is a no-op; otherwise the parent and all descendants
    // shift uniformly so subtree relationships are preserved.
    if (direction === "outdent" && block.indent_level === 0) return;
    // Logseq rule: a block can only indent to become a child of the block
    // directly above it — at most ONE level deeper than that predecessor. No
    // predecessor (vi 0), or already at predecessor-depth + 1 → no-op (this is
    // what stops indenting a block past a valid parent-child relationship).
    if (direction === "indent") {
      const prev = visibleBlocks[vi - 1];
      if (!prev || block.indent_level > prev.indent_level) return;
    }
    pushUndo();
    const delta = direction === "indent" ? 1 : -1;
    const ids = subtreeIds(block);
    blocks = blocks.map(b => ids.has(b.id) ? { ...b, indent_level: Math.max(0, b.indent_level + delta) } : b);
    // Block-granular path: an indent/outdent changes only `indent_level` (and
    // thus parent) on a known set of block ids → one `move` op each, NOT a
    // whole-body PUT. One path per save.
    saveBlocksViaOps(blocks, moveOpsForIds(blocks, ids));
  }

  function handleBackspace(vi: number) {
    lastLocalEditAt = Date.now();
    const block = visibleBlocks[vi];
    if (!block || block.raw_text !== "" || blocks.length <= 1) return;
    pushUndo();
    blocks = blocks.filter(b => b.id !== block.id);
    // Block-granular delete: emit a single `{kind:"delete", bid}` op for the
    // removed block (no whole-body PUT, which would re-assert every surviving
    // block). A local-only block (never round-tripped) yields no op — the
    // local removal is the whole deletion. Replaces the old `saveBlocks` PUT +
    // separate `api.deleteBlock` pair. One path per save.
    saveDeletesViaOps(blocks, [block]);
    if (focusedIndex !== null && focusedIndex > 0) focusedIndex = focusedIndex - 1;
  }

  function handleBackspaceMerge(vi: number, currentText: string) {
    lastLocalEditAt = Date.now();
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
    focusedIndex = vi - 1;
    // Block-granular path: a merge is the survivor's text changing PLUS the
    // absorbed block being deleted. Send BOTH ops in one converged POST so the
    // server applies them together — the file materializes (and the single WS
    // fan-out fires) only after both land, so there's no half-applied window
    // where the survivor's new text is visible with the absorbed block still
    // present. The absorbed `delete` is included only when `current` was a
    // server-known (non-local-only) block; a never-round-tripped local-only
    // block has no server row, so upserting the survivor alone IS the whole
    // merge. If the survivor can't be expressed as an upsert (no bid),
    // `mergeOpsForBackspace`/`upsertOpForStructuralBlock` returns null →
    // `saveBlocksViaOps` falls back to the whole-body PUT. One path per save.
    if (!isLocalOnlyId(current.id) && current.bid) {
      // `mergeOpsForBackspace` returns null when the survivor isn't a clean
      // upsert; map that to a `[null]` batch so `saveBlocksViaOps` takes its
      // whole-body-PUT fallback rather than POSTing a half-batch.
      saveBlocksViaOps(
        blocks,
        mergeOpsForBackspace(blocks, mergedBlock.id, current.bid) ?? [null],
      );
    } else {
      saveBlocksViaOps(blocks, [
        upsertOpForStructuralBlock(blocks, mergedBlock.id),
      ]);
    }
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
    lastLocalEditAt = Date.now();
    if (visibleBlocks.length <= 1) return;
    const block = visibleBlocks[vi];
    if (!block) return;
    pushUndo();
    // Vim convention: dd both deletes AND yanks into the register, so a
    // subsequent p pastes the deleted block.
    blockClipboard = [{ ...block }];
    blocks = blocks.filter(b => b.id !== block.id);
    // Block-granular delete (sync redesign 2026-06-02, S4): emit a single
    // `{kind:"delete", bid}` op instead of the old whole-body PUT
    // (`saveBlocks`) + separate `api.deleteBlock` pair. The PUT re-asserted
    // every surviving block from a possibly-stale view, clobbering concurrent
    // peer edits; the delete op touches only the removed block. A local-only
    // block (never round-tripped) yields no op — the local removal is the
    // whole deletion. One path per save.
    saveDeletesViaOps(blocks, [block]);
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
      // Fresh bid so each pasted copy is a distinct block on the
      // server side. Without this they'd inherit the source block's
      // bid and collide as the same logical row.
      bid: crypto.randomUUID(),
    }));
    blocks = [...blocks.slice(0, fullIdx + 1), ...pasted, ...blocks.slice(fullIdx + 1)];
    // Block-granular path: upsert one op per pasted block by its fresh client-
    // minted bid (see `handleEnter` for the mid-insert ordering caveat). One
    // path per save.
    saveBlocksViaOps(
      blocks,
      pasted.map((p) => upsertOpForStructuralBlock(blocks, p.id)),
    );
    focusedIndex = vi + pasted.length;
  }

  function handleNewBlockAbove(vi: number) {
    lastLocalEditAt = Date.now();
    const current = visibleBlocks[vi];
    if (!current) return;
    const fullIdx = blocks.findIndex(b => b.id === current.id);
    if (fullIdx < 0) return;
    pushUndo();
    const newBlock: ParsedBlock = {
      id: `${noteId}:new-${Date.now()}`,
      bid: crypto.randomUUID(),
      text: "",
      raw_text: "",
      tags: [],
      inline_tags: [],
      trailing_tags: [],
      inherited_tags: [],
      properties: {},
      indent_level: current.indent_level,
      note_id: noteId,
      parent_note_type: null,
    };
    blocks = [...blocks.slice(0, fullIdx), newBlock, ...blocks.slice(fullIdx)];
    // Block-granular path: upsert just the new block by its client-minted bid
    // (see `handleEnter` for the mid-insert ordering caveat). One path per save.
    saveBlocksViaOps(blocks, [upsertOpForStructuralBlock(blocks, newBlock.id)]);
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
    // Capture the removed blocks (with their bids) BEFORE filtering so the
    // block-granular delete batch can read them.
    const deleted = blocks.filter(b => ids.has(b.id));
    blocks = blocks.filter(b => !ids.has(b.id));
    // Block-granular multi-delete (sync redesign 2026-06-02, S4): one
    // `{kind:"delete", bid}` op per removed server-known block, batched into a
    // single POST /blocks call — replaces the old whole-body PUT (`saveBlocks`)
    // that re-asserted every surviving block from a stale view. Local-only
    // removed blocks contribute no op (their local removal is the whole
    // deletion). One path per save.
    saveDeletesViaOps(blocks, deleted);
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
    // Block-granular path: a status cycle changes ONLY the affected blocks'
    // text (the status marker), so emit one `upsert` op per selected block —
    // NOT a whole-body PUT that re-asserts every surviving block and clobbers
    // concurrent peer edits. If any selected block isn't a block-op candidate
    // (no server bid / brand-new local insert), `saveBlocksViaOps` falls back
    // to the whole-body PUT for the entire batch. One path per save.
    saveBlocksViaOps(
      blocks,
      [...ids].map((id) => upsertOpForBlock(blocks, id)),
    );
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
    // Depth cap (mirrors handleIndent): the topmost selected block can only
    // indent to predecessor-depth + 1; if it can't, no-op the whole bulk shift.
    if (direction === "indent") {
      const minVi = Math.min(...visualRange);
      const top = visibleBlocks[minVi];
      const prev = visibleBlocks[minVi - 1];
      if (!top || !prev || top.indent_level > prev.indent_level) return;
    }
    pushUndo();
    const delta = direction === "indent" ? 1 : -1;
    blocks = blocks.map((b) => ids.has(b.id) ? { ...b, indent_level: Math.max(0, b.indent_level + delta) } : b);
    // Block-granular path: same as `handleIndent` but for the visual
    // selection's subtrees → one `move` op per affected block. One path/save.
    saveBlocksViaOps(blocks, moveOpsForIds(blocks, ids));
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
    // Track which blocks actually changed so we emit a `move`-free upsert op
    // ONLY for them — a block skipped by the add/remove-bias guard below keeps
    // its old text and must not be re-asserted (that would clobber a peer's
    // concurrent edit to it).
    const changedIds = new Set<string>();
    blocks = blocks.map((b) => {
      if (!ids.has(b.id)) return b;
      const has = getBlockTags(b.raw_text).some((t) => t.toLowerCase() === lower);
      // anyHas=true → we're removing across the selection; skip blocks that don't have it.
      // anyHas=false → we're adding; skip blocks that already have it.
      if (anyHas !== has) return b;
      const newRaw = toggleBlockTag(b.raw_text, tagName, fillNames);
      const props = parseProperties(newRaw);
      delete props.tags;
      changedIds.add(b.id);
      return {
        ...b,
        raw_text: newRaw,
        text: (newRaw.split("\n")[0] ?? "").replace(/#([A-Za-z0-9_/-]+)/g, "").trim(),
        tags: getBlockTags(newRaw),
        properties: props,
      };
    });
    // Block-granular path: a tag toggle changes ONLY the text of the blocks it
    // actually flipped, so emit one `upsert` op per changed block instead of a
    // whole-body PUT that re-asserts every surviving block and clobbers
    // concurrent peer edits. If no block changed, there is nothing to save. Any
    // non-candidate block (no server bid / brand-new local insert) makes
    // `saveBlocksViaOps` fall back to the whole-body PUT. One path per save.
    if (changedIds.size === 0) return;
    saveBlocksViaOps(
      blocks,
      [...changedIds].map((id) => upsertOpForBlock(blocks, id)),
    );
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

  /** True when DOM focus is anywhere inside this outliner's subtree.
   *  Drives the "focused row" background highlight — without this gate,
   *  every outliner instance (journal view stacks one per day) keeps
   *  its `focusedIndex` row highlighted in parallel, making it look
   *  like the cursor is in multiple places at once. */
  let outlinerHasFocus = $state(false);
  onMount(() => {
    if (!rootEl) return;
    const onIn = () => { outlinerHasFocus = true; };
    const onOut = (ev: FocusEvent) => {
      // `focusout` fires before the new focus target is installed, so
      // `relatedTarget` is the destination. If it's still inside this
      // outliner, focus didn't really leave us; keep `hasFocus` true.
      const next = ev.relatedTarget;
      if (next instanceof Node && rootEl?.contains(next)) return;
      // `focusout` fires during Svelte effect teardown on every structural
      // delete/split (the destroyed block node blurs), so a synchronous
      // `$state` write here throws `state_unsafe_mutation` (Svelte 5.55).
      // Defer to a microtask and re-check live activeElement so we self-
      // correct if focus actually stayed inside (e.g. the split's new
      // BlockEditor grabs focus on remount). Mirrors the queueMicrotask
      // idiom used for the focusedIndex refocus in handleDeleteBlock.
      queueMicrotask(() => {
        if (!rootEl || !rootEl.contains(document.activeElement)) outlinerHasFocus = false;
      });
      // Focus genuinely left this outliner — land any pending coalesced
      // block-ops write now instead of waiting on the debounce timer.
      flushBlockOpsOnBlur();
    };
    rootEl.addEventListener("focusin", onIn);
    rootEl.addEventListener("focusout", onOut);
    return () => {
      rootEl?.removeEventListener("focusin", onIn);
      rootEl?.removeEventListener("focusout", onOut);
    };
  });

  // Prism v4 — register this outliner's root element with the pane-tree
  // registry so later phases can target events at "the outliner in pane
  // X". No-op for the legacy chrome (paneId unset).
  onMount(() => {
    if (!paneId || !rootEl) return;
    registerPaneOutliner(paneId, rootEl);
    return () => unregisterPaneOutliner(paneId);
  });

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
        bid: crypto.randomUUID(),
        text: "",
        raw_text: "",
        tags: [],
        inline_tags: [],
        trailing_tags: [],
        inherited_tags: [],
        properties: {},
        indent_level: 0,
        note_id: noteId,
        parent_note_type: null,
      };
      blocks = [newBlock];
      focusedIndex = 0;
      autoFocused = false;
      restoredFocus = false;
      markRecentlyCreated(newBlock.id);
    }}
  >
    <span class="text-muted-foreground/50">i</span>
    <span class="ml-1.5 text-muted-foreground/40">to insert</span>
  </div>
{:else}
  <div class="space-y-0" bind:this={rootEl}>
    {#each visibleBlocks as block, vi (block.id)}
      {@const displayIndent = block.indent_level - drillRootIndent}
      <div
        data-block-vi={vi}
        class="group flex items-start transition-all relative
          {outlinerHasFocus && focusedIndex === vi ? 'bg-accent/40' : ''}
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
            class="shrink-0 pt-[12px] pl-1 cursor-pointer text-muted-foreground/40 hover:text-foreground/80 transition-colors {(outlinerHasFocus && focusedIndex === vi) || collapsedBlocks.has(block.id) ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'}"
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
            <span class="block w-[5px] h-[5px] rounded-full transition-colors {outlinerHasFocus && focusedIndex === vi ? 'bg-primary' : 'bg-muted-foreground/40 hover:bg-muted-foreground/80'}"></span>
          {:else}
            <IconChevronRight size={12} stroke={2} class="transition-colors {outlinerHasFocus && focusedIndex === vi ? 'text-primary' : 'text-muted-foreground/40 hover:text-foreground/80'}" />
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
            bid={block.bid ?? undefined}
            onlorotext={(text) => handleLoroText(block.id, text)}
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
            onnavigatetoplevel={handleNavigateTopLevel}
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
            {isPinnedTab}
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
              <DisplayChip propKey={chip.key} value={chip.value} def={chip.def} blockId={block.id} />
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

      <!-- Properties row: date/recurrence strip beneath the block line (Task 5/6) -->
      {#if block.properties.scheduled || block.properties.deadline || block.properties.recurring}
        <div style="padding-left: {displayIndent * 24}px;">
          <BlockDateRow {block} onUpdate={(t) => handleBlockChange(block.id, t)} />
        </div>
      {/if}

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
        label: "Pin this block",
        action: () => {
          const preview = ctxMenu!.blockText.trim().slice(0, 40) || "(empty)";
          v5TogglePinBlock(ctxMenu!.blockNoteId, ctxMenu!.blockId, preview);
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
