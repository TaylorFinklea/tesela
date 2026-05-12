<script module lang="ts">
  import { EditorView } from "@codemirror/view";
  import { Vim } from "@replit/codemirror-vim";
  import { gotoNote as moduleGotoNote } from "$lib/stores/active-pane-nav.svelte";

  // Shared context always pointing to the currently focused block editor.
  // Vim actions are registered ONCE globally (Vim.defineAction is a singleton
  // registry — per-instance calls overwrite each other, causing stale closures
  // when a block unmounts). Actions read from this ctx at call time instead.
  const vimCtx: {
    view: EditorView | null;
    navigate: ((dir: "up" | "down") => void) | null;
    pageJump: ((dir: "up" | "down") => void) | null;
    deleteBlock: (() => void) | null;
    yankBlock: (() => void) | null;
    pasteBlock: (() => void) | null;
    newBlockBelow: (() => void) | null;
    newBlockAbove: (() => void) | null;
    indent: ((dir: "indent" | "outdent") => void) | null;
    leader: (() => void) | null;
    drillIn: (() => void) | null;
    enterVisualMode: (() => void) | null;
    exitVisualMode: (() => void) | null;
    visualMode: boolean;
    visualNav: ((dir: "up" | "down") => void) | null;
    visualDelete: (() => void) | null;
    visualYank: (() => void) | null;
    bulkTagPicker: (() => void) | null;
    bulkIndent: ((dir: "indent" | "outdent") => void) | null;
    toggleFold: (() => void) | null;
    toggleProps: (() => void) | null;
    undoOutliner: (() => boolean) | null;
    redoOutliner: (() => boolean) | null;
    beginInsertSession: (() => void) | null;
    endInsertSession: (() => void) | null;
    cycleDrawerTab: ((direction: 1 | -1) => void) | null;
  } = {
    view: null, navigate: null, pageJump: null,
    deleteBlock: null, yankBlock: null,
    pasteBlock: null, newBlockBelow: null, newBlockAbove: null,
    indent: null, leader: null,
    drillIn: null, enterVisualMode: null, exitVisualMode: null,
    visualMode: false, visualNav: null, visualDelete: null, visualYank: null,
    bulkTagPicker: null, bulkIndent: null, toggleFold: null,
    toggleProps: null,
    undoOutliner: null, redoOutliner: null,
    beginInsertSession: null, endInsertSession: null,
    cycleDrawerTab: null,
  };

  // We deliberately re-register on every editor mount. Each call unshifts
  // the mapping to the front of cm-vim's defaultKeymap, ensuring our user
  // mappings always win over later-registered defaults from cm-vim itself.
  // The cost is some duplicate entries in the keymap array, but matching is
  // O(n) and n stays small relative to the default keymap length.
  function initVimActions() {

    // Phase 9.9 follow-up — j/k navigate by VISUAL line (not logical line),
    // so a long wrapped paragraph advances per-row instead of jumping to the
    // next paragraph. Matches the arrow-key behavior the user already had.
    // Also skips lines that the cm-decorations layer has hidden via
    // `display: none` (collapsed properties / `tags::` line). We probe
    // candidate positions with `view.moveVertically` and only commit the
    // dispatch when we land on a visible line; if no visible line is
    // reachable, we return false so the caller can cross-block.
    function lineElementAt(v: EditorView, pos: number): HTMLElement | null {
      const dom = v.domAtPos(pos);
      let node: Node | null = dom?.node ?? null;
      while (node && !(node instanceof HTMLElement && node.classList.contains("cm-line"))) {
        node = node.parentNode;
      }
      return node instanceof HTMLElement ? node : null;
    }
    function isHiddenLine(el: HTMLElement | null): boolean {
      if (!el) return false;
      return el.classList.contains("cm-tesela-hidden-prop-line")
        || el.classList.contains("cm-tesela-tags-line");
    }
    function visualLineMove(forward: boolean): boolean {
      const v = vimCtx.view;
      if (!v) return false;
      const beforeRange = v.state.selection.main;
      const beforeCoords = v.coordsAtPos(beforeRange.head);
      let candidate = beforeRange;
      for (let i = 0; i < 64; i++) {
        const next = v.moveVertically(candidate, forward);
        if (next.head === candidate.head) break; // hit edge of doc
        candidate = next;
        if (!isHiddenLine(lineElementAt(v, next.head))) {
          v.dispatch({ selection: candidate });
          const afterCoords = v.coordsAtPos(candidate.head);
          if (!beforeCoords || !afterCoords) return true;
          return Math.abs(afterCoords.top - beforeCoords.top) > 1;
        }
      }
      return false;
    }

    Vim.defineAction("moveDownOrNextBlock", () => {
      if (vimCtx.visualMode) { vimCtx.visualNav?.("down"); return; }
      if (!visualLineMove(true)) vimCtx.navigate?.("down");
    });
    Vim.mapCommand("j", "action", "moveDownOrNextBlock", {}, { context: "normal" });

    Vim.defineAction("moveUpOrPrevBlock", () => {
      if (vimCtx.visualMode) { vimCtx.visualNav?.("up"); return; }
      if (!visualLineMove(false)) vimCtx.navigate?.("up");
    });
    Vim.mapCommand("k", "action", "moveUpOrPrevBlock", {}, { context: "normal" });

    Vim.defineAction("openLeaderMenu", () => { vimCtx.leader?.(); });
    Vim.mapCommand("<Space>", "action", "openLeaderMenu", {}, { context: "normal" });

    // Phase 9.9 — Ctrl+U / Ctrl+D as outliner page-jump are wired via the
    // cm6-level blockKeymap below (component script), not through vim
    // mapCommand. Reason: cm6's standardKeymap binds `Ctrl-d` to
    // `deleteCharForward` on macOS at a precedence that wins over cm-vim's
    // domEventHandlers, so a Vim.mapCommand entry never gets a chance.

    // Phase 10.2 follow-up — `g` is now a leader prefix that opens the
    // chord menu pre-descended into "Go to". The previous `gd`
    // (followWikiLink) and `gp` (toggleProps) two-key chords are folded
    // into the popup as `g f` and `Space b p` respectively. The cm6
    // keymap below intercepts the bare `g` in NORMAL and dispatches
    // `tesela:open-leader-at` — see `blockKeymap` further down.
    //
    // The `followWikiLink` action body is preserved as a vim action for
    // any caller that still wants to invoke wiki-follow programmatically
    // (e.g. the chord menu's `g f` entry, via tesela:block-action).
    Vim.defineAction("followWikiLink", () => {
      const v = vimCtx.view;
      if (!v) return;
      const pos = v.state.selection.main.head;
      const doc = v.state.doc.toString();
      for (const m of doc.matchAll(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g)) {
        const start = m.index ?? -1;
        if (start < 0) continue;
        if (pos >= start && pos <= start + m[0].length) {
          const target = m[1].trim();
          if (target) moduleGotoNote(target);
          return;
        }
      }
    });

    // We can't use Vim.mapCommand("dd"/"yy"/">>"/"<<", "action", ...) because
    // cm-vim's default `d`, `y`, `>`, `<` are operators that match the FIRST
    // keypress fully, entering operator-pending state. Any user "action"
    // mapping with context: "normal" gets filtered out at that point (cm-vim
    // commandMatches: `inputState.operator && command.type == 'action'` and
    // the context check both reject it). The operator's "press-twice → linewise"
    // path is the ONLY way the second key is recognized, so we hijack it by
    // redefining the operators themselves.
    //
    // Non-linewise operator usage (`dw`, `d$`, etc.) still works because we
    // fall through to a minimal text-replace that handles the selected ranges.
    // cm-vim's evalInput expands the cursor to a line-wide selection BEFORE
    // calling the operator (so default operators see `ranges` covering the
    // whole line). Default operators then RETURN a cursor position, and
    // cm-vim runs `cm.setCursor(returnVal)` to collapse the selection back.
    // If we return undefined, the expanded selection stays — and cm-vim's
    // mode tracker reports "VISUAL" because it sees a multi-char selection.
    // Returning `oldAnchor` (the original cursor before line-expansion) is
    // the standard collapse signal.
    Vim.defineOperator("delete", (cm: any, args: any, ranges: any, oldAnchor: any) => {
      if (args?.linewise) {
        if (vimCtx.visualMode) vimCtx.visualDelete?.();
        else vimCtx.deleteBlock?.();
        return oldAnchor;
      }
      for (let i = ranges.length - 1; i >= 0; i--) {
        cm.replaceRange("", ranges[i].anchor, ranges[i].head);
      }
      return oldAnchor;
    });

    Vim.defineOperator("yank", (_cm: any, args: any, _ranges: any, oldAnchor: any) => {
      if (args?.linewise) {
        if (vimCtx.visualMode) vimCtx.visualYank?.();
        else vimCtx.yankBlock?.();
        return oldAnchor;
      }
      // Non-linewise yank is a no-op here — we don't have access to vim's
      // register controller from this scope. Users wanting partial-line yank
      // can use OS clipboard.
      return oldAnchor;
    });

    Vim.defineOperator("indent", (_cm: any, args: any, _ranges: any, oldAnchor: any) => {
      if (args?.linewise) {
        const dir: "indent" | "outdent" = args.indentRight ? "indent" : "outdent";
        if (vimCtx.visualMode) vimCtx.bulkIndent?.(dir);
        else vimCtx.indent?.(dir);
        return oldAnchor;
      }
      // Non-linewise indent is a no-op — `>w` doesn't make sense for a
      // single-line block.
      return oldAnchor;
    });

    Vim.defineAction("pasteBlock", () => { vimCtx.pasteBlock?.(); });
    Vim.mapCommand("p", "action", "pasteBlock", {}, { context: "normal" });

    Vim.defineAction("newBlockBelow", () => { vimCtx.newBlockBelow?.(); });
    Vim.mapCommand("o", "action", "newBlockBelow", {}, { context: "normal" });

    Vim.defineAction("newBlockAbove", () => { vimCtx.newBlockAbove?.(); });
    Vim.mapCommand("O", "action", "newBlockAbove", {}, { context: "normal" });

    // `Y` defaults to operatorMotion (yank to line). Our user mapping is
    // unshifted to the front of the keymap so it wins the iteration. Routes
    // through yankBlock action which respects block-visual mode.
    Vim.defineAction("yankBlockSingle", () => {
      if (vimCtx.visualMode) vimCtx.visualYank?.();
      else vimCtx.yankBlock?.();
    });
    Vim.mapCommand("Y", "action", "yankBlockSingle", {}, { context: "normal" });

    Vim.defineAction("blockVisualMode", () => { vimCtx.enterVisualMode?.(); });
    Vim.mapCommand("V", "action", "blockVisualMode", {}, { context: "normal" });

    Vim.defineAction("drillIntoBlock", () => { vimCtx.drillIn?.(); });
    Vim.mapCommand("<CR>", "action", "drillIntoBlock", {}, { context: "normal" });

    // `T` in block-visual mode opens the bulk tag picker. In normal mode it
    // no-ops (overrides vim's "find char backward till" — uncommon enough in
    // an outliner that the trade-off is fine).
    Vim.defineAction("bulkTagPickerOrNoop", () => {
      if (vimCtx.visualMode) vimCtx.bulkTagPicker?.();
    });
    Vim.mapCommand("T", "action", "bulkTagPickerOrNoop", {}, { context: "normal" });

    // Fold / unfold the focused block's subtree. za (toggle) is the standard
    // vim fold mapping; zc/zo are also accepted as aliases (we don't track
    // closed-vs-open separately, so all three toggle).
    Vim.defineAction("toggleFold", () => { vimCtx.toggleFold?.(); });
    Vim.mapCommand("za", "action", "toggleFold", {}, { context: "normal" });
    Vim.mapCommand("zc", "action", "toggleFold", {}, { context: "normal" });
    Vim.mapCommand("zo", "action", "toggleFold", {}, { context: "normal" });

    // Outliner-level undo / redo. Tries the structural-mutation stack first
    // (block delete/paste/indent/fold/status/tag/etc); when empty, falls
    // through to cm-editor's history so intra-block typing-undo still works.
    // The fall-through means `u` always does *something* in normal mode,
    // which matches user expectation across both kinds of changes.
    Vim.defineAction("undoBlockOp", (cm: any) => {
      const handled = vimCtx.undoOutliner?.() ?? false;
      if (!handled) cm?.execCommand?.("undo");
    });
    Vim.mapCommand("u", "action", "undoBlockOp", {}, { context: "normal" });

    Vim.defineAction("redoBlockOp", (cm: any) => {
      const handled = vimCtx.redoOutliner?.() ?? false;
      if (!handled) cm?.execCommand?.("redo");
    });
    Vim.mapCommand("<C-r>", "action", "redoBlockOp", {}, { context: "normal" });

    // Drawer-tab cycling from inside a pinned-tab editor.
    // cycleDrawerTab is set to null for non-pinned editors so these actions
    // no-op when the user is in the main focus-area editor.
    Vim.defineAction("nextDrawerTab", () => { vimCtx.cycleDrawerTab?.(1); });
    Vim.mapCommand("gt", "action", "nextDrawerTab", {}, { context: "normal" });

    Vim.defineAction("prevDrawerTab", () => { vimCtx.cycleDrawerTab?.(-1); });
    // cm-vim parses multi-char sequences; capital T has a shorter mapping ("T")
    // that can shadow the second character of "gT".  Use the canonical shift
    // encoding "g<S-t>" so cm-vim treats it as a distinct chord.
    Vim.mapCommand("g<S-t>", "action", "prevDrawerTab", {}, { context: "normal" });
  }
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { Annotation, Compartment, EditorState, Transaction } from "@codemirror/state";

  // Tags transactions dispatched by the prop→cm6 sync $effect (e.g. when
  // outliner-undo restores blocks[i].body). The updateListener skips these
  // so they don't loop back through onChange as fake user edits.
  const externalSync = Annotation.define<boolean>();
  import { keymap, drawSelection } from "@codemirror/view";
  import { defaultKeymap, history, historyKeymap } from "@codemirror/commands";
  import { vim, getCM } from "@replit/codemirror-vim";
  import {
    teselaDecorations,
    teselaDecorationTheme,
    hiddenPropertyKeysFacet,
    primaryTagFacet,
    type HiddenKeysConfig,
  } from "$lib/cm-decorations";
  import { toggleBlockTag, getBlockTags, upsertBlockProperty } from "$lib/block-tags";
  import type { PropertyDefinition } from "$lib/property-registry";
  import { assignChords } from "$lib/chord-keys";

  // `i` is reserved as the chord-menu's filter trigger (see ChordMenu).
  // Reserving here keeps the assigner from handing it out to any node, so
  // pressing `i` always opens search regardless of which menu the user is
  // in or what tag-properties they've defined.
  const SLASH_RESERVED: ReadonlySet<string> = new Set(["i"]);
  import { setVimMode, getVimMode, cycleBottomDrawerTab } from "$lib/stores/pane-state.svelte";
  import { api } from "$lib/api-client";
  import { goto } from "$app/navigation";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";
  import ChordMenu, { type ChordNode } from "./ChordMenu.svelte";
  import AutocompleteMenu, { type AutocompleteItem } from "./AutocompleteMenu.svelte";
  import DatePicker from "./DatePicker.svelte";

  let {
    initialText,
    onblur: onBlur,
    onfocus: onFocus,
    onchange: onChange,
    onnavigate: onNavigate,
    onescape: onEscape,
    onenter: onEnter,
    onindent: onIndent,
    onbackspaceempty: onBackspaceEmpty,
    onbackspacemerge: onBackspaceMerge,
    initialCursorPos,
    startininsert: startInInsert,
    autofocused: autoFocused = false,
    onslashcommand: onSlashCommand,
    onleader: onLeader,
    ondeleteblock: onDeleteBlock,
    onyankblock: onYankBlock,
    onpasteblock: onPasteBlock,
    onnewblockbelow: onNewBlockBelow,
    onnewblockabove: onNewBlockAbove,
    oncyclestatus: onCycleStatus,
    ondrillIn: onDrillIn,
    onentervisualmode: onEnterVisualMode,
    onexitvisualmode: onExitVisualMode,
    onvisualnav: onVisualNav,
    onvisualdelete: onVisualDelete,
    onvisualyank: onVisualYank,
    onbulktagpicker: onBulkTagPicker,
    onbulkindent: onBulkIndent,
    ontogglefold: onToggleFold,
    ontoggleprops: onToggleProps,
    onpagejump: onPageJump,
    onUndoOutliner,
    onRedoOutliner,
    onBeginInsertSession,
    onEndInsertSession,
    inVisualMode,
    focused,
    noteslist: notesList,
    statusChoices,
    hiddenKeys,
    primaryTag,
    autoFillNames,
    propertyDefs,
    onInsertTemplate,
    isPinnedTab = false,
  }: {
    initialText: string;
    onblur: () => void;
    onfocus?: () => void;
    onchange: (text: string) => void;
    onnavigate?: (direction: "up" | "down") => void;
    onescape?: () => void;
    onenter?: (textAfterCursor: string) => void;
    onindent?: (direction: "indent" | "outdent") => void;
    onbackspaceempty?: () => void;
    onbackspacemerge?: (text: string) => void;
    initialCursorPos?: number;
    startininsert?: boolean;
    autofocused?: boolean;
    onslashcommand?: (command: string) => void;
    onleader?: () => void;
    ondeleteblock?: () => void;
    onyankblock?: () => void;
    onpasteblock?: () => void;
    onnewblockbelow?: () => void;
    onnewblockabove?: () => void;
    oncyclestatus?: () => void;
    ondrillIn?: () => void;
    onentervisualmode?: () => void;
    onexitvisualmode?: () => void;
    onvisualnav?: (dir: "up" | "down") => void;
    onvisualdelete?: () => void;
    onvisualyank?: () => void;
    onbulktagpicker?: () => void;
    onbulkindent?: (direction: "indent" | "outdent") => void;
    ontogglefold?: () => void;
    ontoggleprops?: () => void;
    onpagejump?: (direction: "up" | "down") => void;
    onUndoOutliner?: () => boolean;
    onRedoOutliner?: () => boolean;
    onBeginInsertSession?: () => void;
    onEndInsertSession?: () => void;
    inVisualMode?: boolean;
    focused?: boolean;
    noteslist?: Array<{ id: string; title: string; tags: string[]; note_type?: string | null }>;
    statusChoices?: string[];
    /** Per-block list of property keys to hide in the editor (computed by
     *  BlockOutliner from inherited tag-property defs). */
    hiddenKeys?: HiddenKeysConfig;
    /** Phase 9.4 — primary tag (kind) for the kind-glyph badge prefix.
     *  Comes from `block.tags[0]` in BlockOutliner. `null` for blocks with no
     *  tag chain (no badge rendered). */
    primaryTag?: string | null;
    /** Resolves a tag name to its auto-fill property names (visible-by-default
     *  property defs). Used when toggling a tag ON to append empty `key:: `
     *  continuation lines for each property. */
    autoFillNames?: (tagName: string) => string[];
    /** Phase 10.4 — full PropertyDefinition list for this block's tag chain.
     *  Drives the `/p` chord submenu so the user can pick a real property key
     *  (with type-aware value entry) instead of editing literal `key:: value`
     *  text. Empty when the block has no tagged properties. */
    propertyDefs?: PropertyDefinition[];
    /** Called when /template picks a template — receives the template note's
     *  ID. The BlockOutliner fetches its body and inserts the parsed blocks as
     *  children of the current block. */
    onInsertTemplate?: (templateNoteId: string) => void;
    /** When true, this editor is inside a pinned drawer tab. Enables gt/gT
     *  vim actions for cycling drawer tabs from within the editor. */
    isPinnedTab?: boolean;
  } = $props();

  const hiddenKeysCompartment = new Compartment();
  const primaryTagCompartment = new Compartment();

  let container: HTMLDivElement;
  let view = $state<EditorView | null>(null);

  // Slash menu state — Phase 10.3: chord-leader popover via ChordMenu.
  // No filter/typing-narrow; single-letter chords run actions immediately.
  let showSlashMenu = $state(false);
  let slashPosition = $state({ x: 0, y: 0 });
  let slashStartPos = $state<number>(-1);

  // Autocomplete state (for # tags and [[ wiki-links)
  let showAutocomplete = $state(false);
  let autocompleteFilter = $state("");
  let autocompletePosition = $state({ x: 0, y: 0 });
  let autocompleteRef = $state<AutocompleteMenu | null>(null);
  let autocompleteStartPos = $state<number>(-1);
  let autocompleteType = $state<"tag" | "link" | "tagmanage" | "templatepick">("tag");
  let tagManageItems = $state<AutocompleteItem[]>([]);
  let templatePickItems = $state<AutocompleteItem[]>([]);

  // Date picker state
  let showDatePicker = $state(false);
  let datePickerPosition = $state({ x: 0, y: 0 });
  let datePickerCursor = $state<number>(-1); // where to insert the [[YYYY-MM-DD]]
  /**
   * Phase 10.4 — when set, the date picker writes a `\n<key>:: [[YYYY-MM-DD]]`
   * continuation onto the block (driven by `/p` → date-typed property)
   * instead of inserting `[[YYYY-MM-DD]]` at the cursor (the standard `/d`
   * flow). Cleared on close.
   */
  let datePickerPropertyKey = $state<string | null>(null);

  const autocompleteItems: AutocompleteItem[] = $derived(
    (notesList ?? []).map((n) => ({
      id: n.id,
      label: n.title,
      secondary: n.tags.length > 0 ? n.tags[0] : undefined,
    })),
  );

  function applyAutocomplete(item: AutocompleteItem) {
    if (!view || autocompleteStartPos < 0) return;
    const doc = view.state.doc.toString();

    if (autocompleteType === "tagmanage") {
      // Strip the user's typed filter text (between autocompleteStartPos and cursor)
      // before toggling — otherwise the filter chars end up as block content.
      const cursorPos = view.state.selection.main.head;
      const cleaned = doc.slice(0, autocompleteStartPos) + doc.slice(cursorPos);
      const fillNames = autoFillNames?.(item.label) ?? [];
      const newText = toggleBlockTag(cleaned, item.label, fillNames);
      view.dispatch({
        changes: { from: 0, to: doc.length, insert: newText },
        selection: { anchor: Math.min(autocompleteStartPos, newText.length) },
      });
      onChange(newText);
      // Refresh active indicators and keep menu open
      const activeTags = new Set(getBlockTags(newText).map((t) => t.toLowerCase()));
      tagManageItems = tagManageItems.map((t) => ({
        ...t,
        secondary: activeTags.has(t.label.toLowerCase()) ? "✓" : undefined,
      }));
      autocompleteFilter = "";
      return;
    }

    if (autocompleteType === "templatepick") {
      // Strip any filter text the user typed, then dispatch the template
      // insert up to BlockOutliner. We pass the template note's ID; the parent
      // is responsible for fetching the body and inserting child blocks.
      const cursorPos = view.state.selection.main.head;
      const cleaned = doc.slice(0, autocompleteStartPos) + doc.slice(cursorPos);
      view.dispatch({
        changes: { from: 0, to: doc.length, insert: cleaned },
        selection: { anchor: Math.min(autocompleteStartPos, cleaned.length) },
      });
      onChange(cleaned);
      onInsertTemplate?.(item.id);
      showAutocomplete = false;
      autocompleteFilter = "";
      autocompleteStartPos = -1;
      return;
    }

    const cursorPos = view.state.selection.main.head;
    const before = doc.slice(0, autocompleteStartPos);
    const after = doc.slice(cursorPos);

    let insert: string;
    if (autocompleteType === "tag") {
      insert = before + "#" + item.label + after;
    } else {
      insert = before + "[[" + item.label + "]]" + after;
    }

    view.dispatch({
      changes: { from: 0, to: doc.length, insert },
      selection: { anchor: insert.length - after.length },
    });
    onChange(insert);
    showAutocomplete = false;
    autocompleteFilter = "";
    autocompleteStartPos = -1;
  }

  function statusIcon(s: string): string {
    if (s === "done") return "✓";
    if (s === "doing") return "◎";
    return "☐";
  }

  /**
   * Phase 10.3 — chord-leader tree for the in-block `/` menu. Each leaf
   * fires `applySlash(<id>)`, which preserves the existing per-command
   * logic (tag toggle / heading / property / link / date / query /
   * widget / collection / template). One submenu (`s` Status) keys off
   * the dynamically-loaded `statusChoices`.
   *
   * Chord assignments — keep first-letter intuitive where possible:
   *   t Task         T Tag picker     s Status (sub)
   *   h Heading      p Property       l Link    d Date
   *   q Query        w Widget         c Collection   m Template
   */
  /**
   * Assign unique single-character chord keys to a list of choices. For
   * each choice, try preferred-letter aliases first ("doing"→`i` for
   * in-progress, "in-review"→`r`), then walk the choice's letters in
   * order and pick the first one not yet claimed. Falls back to digits
   * 1-9 when the choice has no unclaimed letters. The user's Status
   * property choices come from a config file (`notes/status.md`) so
   * collisions like {done, dude} are entirely possible.
   */
  function assignStatusKeys(choices: string[]): string[] {
    const used = new Set<string>();
    const aliases: Record<string, string> = { doing: "i", "in-review": "r" };
    return choices.map((c) => {
      const lower = c.toLowerCase();
      const candidates: string[] = [];
      if (aliases[lower]) candidates.push(aliases[lower]);
      for (const ch of lower) if (/[a-z]/.test(ch)) candidates.push(ch);
      for (let i = 1; i <= 9; i++) candidates.push(String(i));
      for (const k of candidates) {
        if (!used.has(k)) { used.add(k); return k; }
      }
      return "?"; // unreachable — 9 digits + 26 letters covers any realistic count
    });
  }

  /**
   * Phase 10.4 — write a `key:: value` continuation onto the current block,
   * cleaning up the `/`-trigger text the same way `applySlash` does. Used by
   * the new `/p` chord submenu so each property pick lands a real key/value
   * pair instead of forcing the user into the bottom props panel.
   */
  /**
   * Open the DatePicker bound to a property key. The `/p…` slash-trigger
   * text is stripped immediately (so the chord-close path doesn't strand
   * a `/`); picking a date then upserts the property via
   * `upsertBlockProperty` — same write path as the chord-leaf flow.
   */
  function openDatePickerForProperty(key: string) {
    if (!view || slashStartPos < 0) return;
    const doc = view.state.doc.toString();
    const cursorPos = view.state.selection.main.head;
    let triggerStart = slashStartPos;
    while (triggerStart > 0 && (doc[triggerStart - 1] === " " || doc[triggerStart - 1] === "\t")) {
      triggerStart--;
    }
    const cleaned = doc.slice(0, triggerStart) + doc.slice(cursorPos);
    view.dispatch({
      changes: { from: 0, to: doc.length, insert: cleaned },
      selection: { anchor: triggerStart },
    });
    onChange(cleaned);
    showSlashMenu = false;
    datePickerPropertyKey = key;
    datePickerCursor = triggerStart;
    const coords = view.coordsAtPos(Math.min(triggerStart, view.state.doc.length));
    datePickerPosition = coords
      ? { x: coords.left, y: coords.bottom + 4 }
      : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
    showDatePicker = true;
  }

  /**
   * Phase 10.5 — upsert a property on the block: strip the `/p…` trigger
   * text, then either replace the existing `<key>:: …` continuation line
   * or append a new one. Keys persist as lowercase. Routing through
   * `upsertBlockProperty` (instead of raw append) is what keeps the doc
   * and the bottom drawer in lock-step — both surfaces edit the same line.
   */
  function writePropertyContinuation(key: string, value: string) {
    if (!view || slashStartPos < 0) return;
    const doc = view.state.doc.toString();
    const cursorPos = view.state.selection.main.head;
    // Drop the `/p…` chars and any horizontal whitespace immediately
    // preceding the slash so the trigger doesn't leave a trailing space
    // on the block-content line.
    let triggerStart = slashStartPos;
    while (triggerStart > 0 && (doc[triggerStart - 1] === " " || doc[triggerStart - 1] === "\t")) {
      triggerStart--;
    }
    const cleaned = doc.slice(0, triggerStart) + doc.slice(cursorPos);
    const next = upsertBlockProperty(cleaned, key, value);
    view.dispatch({
      changes: { from: 0, to: doc.length, insert: next },
      selection: { anchor: triggerStart },
    });
    onChange(next);
    showSlashMenu = false;
    slashStartPos = -1;
    view.focus();
  }
  /**
   * Phase 10.4 — build the `/p` Property chord submenu from the block's
   * tag-property defs. Each def becomes one row; type drives value entry:
   *   - select / multi-select: chord submenu of choices
   *   - checkbox: chord submenu (t/f)
   *   - date: opens DatePicker bound to this property key
   *   - text / number / url / email / phone (and unknown): inline input
   *     mode in the popover (Enter commits)
   * When no defs are available (block has no tagged properties), returns a
   * single "Manual" leaf that falls back to the original literal `key::
   * value` insertion so the user can still scaffold a property by hand.
   */
  /**
   * Phase 12.2 — build a value-submenu (or leaf action) for one property.
   * Pure factory: assumes the caller already picked the parent's chord key.
   * For select / multi-select properties, value chord keys honor the
   * property page's `value_chord_keys:` map first (with conflict detection
   * via `assignChords`), then fall back to first-letter.
   */
  function buildPropertyNode(def: PropertyDefinition, parentKey: string): ChordNode {
    const node: ChordNode = { key: parentKey, label: def.name, hint: def.value_type };
    if (def.value_type === "select" || def.value_type === "multi-select") {
      const valueAssignments = assignChords(
        def.choices.map((c) => ({
          name: c,
          preferred: def.value_chord_keys[c.toLowerCase()] ?? null,
        })),
        { reserved: SLASH_RESERVED },
      );
      node.children = def.choices.map((c, i) => {
        const a = valueAssignments[i];
        const child: ChordNode = {
          key: a.key,
          label: c,
          action: () => writePropertyContinuation(def.name, c),
          hint: `${def.name}:: ${c}`,
        };
        if (a.conflictWith) child.conflictWith = a.conflictWith;
        return child;
      });
    } else if (def.value_type === "checkbox") {
      node.children = [
        { key: "t", label: "true",  action: () => writePropertyContinuation(def.name, "true") },
        { key: "f", label: "false", action: () => writePropertyContinuation(def.name, "false") },
      ];
    } else if (def.value_type === "date") {
      node.action = () => openDatePickerForProperty(def.name);
    } else {
      node.input = {
        placeholder: `${def.name} value`,
        initial: def.default ?? "",
        onSubmit: (v) => writePropertyContinuation(def.name, v.trim()),
      };
    }
    return node;
  }

  function getPropertyChildren(): ChordNode[] {
    const defs = propertyDefs ?? [];
    if (defs.length === 0) {
      return [
        { key: "k", label: "Manual key:: value", action: () => applySlash("property"), hint: "key:: value" },
      ];
    }
    const assignments = assignChords(
      defs.map((d) => ({ name: d.name, preferred: d.chord_key })),
      { reserved: SLASH_RESERVED },
    );
    return defs.map((def, i) => {
      const a = assignments[i];
      const node = buildPropertyNode(def, a.key);
      if (a.conflictWith) node.conflictWith = a.conflictWith;
      return node;
    });
  }

  function getSlashTree(): ChordNode[] {
    // Phase 12.2 — slash tree is one flat list:
    //   1. Built-in insertion verbs (Task, Tag, Heading, Link, Date, …) with
    //      hard-coded chord keys.
    //   2. Hoisted tag-properties for the focused block (Status, Priority,
    //      Deadline, …) so the user picks them in one chord rather than
    //      `/p > X`. Their preferred key comes from the Property page's
    //      `chord_key:`; collisions with built-ins fall back to first-letter
    //      and surface a "taken by …" warning in the menu.
    //   3. `/p` "All properties" — discovery surface for every property in
    //      the registry, including ones not on this block's tags.
    //   4. Hardcoded `/s Status` ONLY when the block has no tag-properties,
    //      so untagged blocks still get a one-chord status setter.
    const defs = propertyDefs ?? [];
    const choices = statusChoices ?? ["todo", "doing", "done"];

    const builtins: ChordNode[] = [
      { key: "t", label: "Task",         action: () => applySlash("task"),       hint: "tags:: Task" },
      { key: "T", label: "Tag picker",   action: () => applySlash("tag"),        hint: "#" },
      { key: "h", label: "Heading",      action: () => applySlash("heading") },
      { key: "l", label: "Link",         action: () => applySlash("link"),       hint: "[[ ]]" },
      { key: "d", label: "Date",         action: () => applySlash("date") },
      { key: "q", label: "Query",        action: () => applySlash("query") },
      { key: "w", label: "New widget",   action: () => applySlash("widget") },
      { key: "c", label: "Collection",   action: () => applySlash("collection") },
      { key: "m", label: "Template",     action: () => applySlash("template") },
      { key: "p", label: "All properties", children: getPropertyChildren() },
    ];

    // Single assignChords pass with builtins pre-claimed so a tag-property
    // declaring `chord_key: t` (would shadow Task) loses gracefully and gets
    // a "taken by Task" warning in the rendered menu.
    const items = [
      ...builtins.map((b) => ({ name: b.label, preferred: b.key })),
      ...defs.map((d) => ({ name: d.name, preferred: d.chord_key })),
    ];
    const all = assignChords(items, { reserved: SLASH_RESERVED });

    const builtinNodes: ChordNode[] = builtins.map((b, i) => ({ ...b, key: all[i].key }));
    const propNodes: ChordNode[] = defs.map((def, i) => {
      const a = all[builtins.length + i];
      const node = buildPropertyNode(def, a.key);
      if (a.conflictWith) node.conflictWith = a.conflictWith;
      return node;
    });

    // Untagged-block fallback: keep the legacy `/s Status` so plain blocks
    // (without #Task) can still set a status quickly. When the block IS
    // tagged, Status appears in propNodes and this fallback is skipped.
    const fallbackStatus: ChordNode[] = defs.length === 0
      ? [{
          key: "s",
          label: "Status",
          children: choices.map((s, i) => ({
            key: assignStatusKeys(choices)[i],
            label: s.charAt(0).toUpperCase() + s.slice(1).replace(/-/g, " "),
            action: () => applySlash(s),
            hint: `status:: ${s}`,
          })),
        }]
      : [];

    return [...builtinNodes, ...propNodes, ...fallbackStatus];
  }

  function applySlash(command: string) {
    if (!view || slashStartPos < 0) return;
    const doc = view.state.doc.toString();
    const cursorPos = view.state.selection.main.head;
    const before = doc.slice(0, slashStartPos);
    const after = doc.slice(cursorPos);

    let insert = "";
    const allStatuses = statusChoices ?? ["todo", "doing", "done"];
    if (allStatuses.includes(command)) {
      insert = before.trimEnd() + "\nstatus:: " + command + after;
    } else {
      switch (command) {
        case "task": {
          const cleaned = before + after;
          const hasTask = getBlockTags(cleaned).some((t) => t.toLowerCase() === "task");
          insert = hasTask ? cleaned : toggleBlockTag(cleaned, "Task", autoFillNames?.("Task") ?? []);
          break;
        }
        case "heading":
          insert = "# " + before.trim() + after;
          break;
        case "property":
          insert = before.trimEnd() + "\nkey:: value" + after;
          break;
        case "link":
          insert = before + "[[]]" + after;
          break;
        case "tag": {
          // Remove the slash text, keep cursor position, open tag manager
          // Remove only the slash character; preserve newlines in `after` exactly
          insert = before + after;
          const cursorAfter = before.length;
          const currentDoc = before + after;
          setTimeout(() => {
            if (!view) return;
            const activeTags = new Set(getBlockTags(currentDoc).map((t) => t.toLowerCase()));
            tagManageItems = (notesList ?? [])
              .filter((n) => n.note_type === "Tag")
              .map((n) => ({
                id: n.id,
                label: n.title,
                secondary: activeTags.has(n.title.toLowerCase()) ? "✓" : undefined,
              }));
            autocompleteStartPos = cursorAfter;
            autocompleteType = "tagmanage";
            showAutocomplete = true;
            autocompleteFilter = "";
            const coords = view.coordsAtPos(Math.min(cursorAfter, view.state.doc.length));
            autocompletePosition = coords
              ? { x: coords.left, y: coords.bottom + 4 }
              : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
          }, 0);
          break;
        }
        case "date": {
          // Strip the slash text and open the date picker. The picker
          // dispatches the chosen date as `[[YYYY-MM-DD]]` at the cursor.
          insert = before + after;
          const cursorAfter = before.length;
          setTimeout(() => {
            if (!view) return;
            datePickerCursor = cursorAfter;
            const coords = view.coordsAtPos(Math.min(cursorAfter, view.state.doc.length));
            datePickerPosition = coords
              ? { x: coords.left, y: coords.bottom + 4 }
              : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
            showDatePicker = true;
          }, 0);
          break;
        }
        case "query": {
          // Scaffold an inline query block. Cursor lands at end of `tag:` so
          // the user immediately types a tag name.
          const cleaned = before.trimEnd();
          const queryHead = cleaned + "\nquery:: tag:";
          insert = queryHead + "\nview:: table" + after;
          view.dispatch({
            changes: { from: 0, to: doc.length, insert },
            selection: { anchor: queryHead.length },
          });
          onChange(insert);
          showSlashMenu = false;
          slashStartPos = -1;
          onSlashCommand?.(command);
          return;
        }
        case "widget": {
          // Strip the `/widget` text, prompt for a name, then create a Query
          // note that becomes a saved widget in the rail. Navigates to the
          // new note for editing the DSL.
          insert = before + after;
          view.dispatch({
            changes: { from: 0, to: doc.length, insert },
            selection: { anchor: before.length },
          });
          onChange(insert);
          showSlashMenu = false;
          slashStartPos = -1;
          onSlashCommand?.(command);
          setTimeout(async () => {
            const name = window.prompt("New query widget name:");
            if (!name || !name.trim()) return;
            const trimmed = name.trim();
            const content = `---\ntitle: "${trimmed}"\ntype: "Query"\ntags: []\n---\nquery::\nsection:: saved\n`;
            try {
              const created = await api.createNote(trimmed, content);
              goto(`/p/${encodeURIComponent(created.id)}`);
            } catch (e) {
              console.error("Failed to create widget:", e);
            }
          }, 0);
          return;
        }
        case "collection": {
          // Scaffold an inline collection block. Empty list to start; user
          // adds blocks via the "+ Add block" button.
          const cleaned = before.trimEnd();
          insert = cleaned + "\ncollection:: []\nview:: cards" + after;
          break;
        }
        case "template": {
          // Open a picker showing all #Template-tagged notes. Pick one to
          // insert its body as child blocks (handled by BlockOutliner via
          // onInsertTemplate callback).
          insert = before + after;
          const cursorAfter = before.length;
          setTimeout(() => {
            if (!view) return;
            templatePickItems = (notesList ?? [])
              .filter((n) => n.tags.some((t) => t.toLowerCase() === "template"))
              .map((n) => ({
                id: n.id,
                label: n.title,
              }));
            autocompleteStartPos = cursorAfter;
            autocompleteType = "templatepick";
            showAutocomplete = true;
            autocompleteFilter = "";
            const coords = view.coordsAtPos(Math.min(cursorAfter, view.state.doc.length));
            autocompletePosition = coords
              ? { x: coords.left, y: coords.bottom + 4 }
              : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
          }, 0);
          break;
        }
        default:
          return;
      }
    }

    view.dispatch({
      changes: { from: 0, to: doc.length, insert },
      selection: { anchor: insert.length - after.length },
    });
    onChange(insert);
    showSlashMenu = false;
    slashStartPos = -1;
    onSlashCommand?.(command);
    // Belt-and-braces: ensure cm-editor regains DOM focus after the slash
    // menu closes. The `focused` prop drives focus too, but the explicit
    // call avoids any race with overlay teardown.
    view.focus();
  }

  // When parent changes focused prop, programmatically focus/blur CM6.
  // Phase 9.9 follow-up — also honor `startInInsert` post-mount: the parent
  // may flip `focused` to true AFTER our initial mount (e.g. the auto-focus
  // effect that runs once visibleBlocks settle), and the onMount path's
  // `if (focused)` block won't re-fire. Without this, ?fresh=1 notes land
  // with cm-content focused but stuck in NORMAL.
  let appliedAutoInsert = $state(false);
  $effect(() => {
    if (!focused || !view) return;
    // Only grab DOM focus when the parent's intent is "user is here now"
    // (handleNavigate, click, cross-day arm). The auto-focus effect on
    // mount sets focusedIndex but keeps autoFocused=true to indicate
    // "decorative only — don't steal keyboard focus." Without this gate,
    // every BlockOutliner in a journal stack races to .focus() on mount,
    // chains a focus DOM event through the inline onfocus handler that
    // flips autoFocused=false, which then triggers the auto-INSERT path
    // below for any empty block — landing the user in INSERT before
    // they've pressed a key.
    if (!view.hasFocus && !autoFocused) view.focus();
    if (startInInsert && !appliedAutoInsert) {
      appliedAutoInsert = true;
      const cm = getCM(view);
      if (cm) Vim.handleKey(cm, "i", "mapping");
    }
  });

  // When the parent updates hiddenKeys (e.g. tag added/removed, tag-property
  // hide_by_default changed), reconfigure the facet compartment so the
  // decoration plugin picks up the new value.
  $effect(() => {
    if (!view) return;
    view.dispatch({
      effects: hiddenKeysCompartment.reconfigure(
        hiddenPropertyKeysFacet.of(hiddenKeys ?? { hide: new Set(), hideEmpty: new Set() }),
      ),
    });
  });

  // Phase 9.4 — primary tag for the kind-glyph badge.
  $effect(() => {
    if (!view) return;
    view.dispatch({
      effects: primaryTagCompartment.reconfigure(primaryTagFacet.of(primaryTag ?? null)),
    });
  });

  // Keep visualMode flag in sync so j/k vim actions can check it without props.
  $effect(() => { vimCtx.visualMode = inVisualMode ?? false; });

  // Sync external prop changes (outliner-undo, WS reparse) into cm6's doc.
  // During normal typing the prop already matches cm6's doc by the time
  // this effect runs, so the equality guard prevents redundant transactions.
  $effect(() => {
    const v = view;
    if (!v) return;
    if (initialText !== v.state.doc.toString()) {
      v.dispatch({
        changes: { from: 0, to: v.state.doc.length, insert: initialText },
        // `addToHistory: false` excludes this transaction from cm6's per-block
        // history — so after `u` rewrites the doc, a subsequent local `Cmd+Z`
        // can't walk back through the just-undone state.
        annotations: [
          externalSync.of(true),
          Transaction.addToHistory.of(false),
        ],
      });
    }
  });

  // Keep the global vim context pointing to whichever block is currently
  // focused. This used to live in a $effect on the `focused` prop, but
  // that prop is set by the parent BlockOutliner from `focusedIndex === vi`
  // — and in the journal view, every day's outliner auto-sets
  // `focusedIndex = 0` on mount, so EVERY day's vi=0 BlockEditor thought
  // `focused === true`. The last to flush its effect (the bottom-most day,
  // Friday April 10) won the vimCtx race, and pressing j/k advanced *its*
  // focusedIndex — which then triggered its `view.focus()` effect and
  // snapped DOM focus to that day. Wiring vimCtx in the actual DOM focus
  // event ensures it follows the editor the user is really in.
  function wireVimCtx() {
    if (!view) return;
    vimCtx.view = view;
    vimCtx.navigate = onNavigate ?? null;
    vimCtx.leader = onLeader ?? null;
    vimCtx.deleteBlock = onDeleteBlock ?? null;
    vimCtx.yankBlock = onYankBlock ?? null;
    vimCtx.pasteBlock = onPasteBlock ?? null;
    vimCtx.newBlockBelow = onNewBlockBelow ?? null;
    vimCtx.newBlockAbove = onNewBlockAbove ?? null;
    vimCtx.indent = onIndent ?? null;
    vimCtx.drillIn = onDrillIn ?? null;
    vimCtx.enterVisualMode = onEnterVisualMode ?? null;
    vimCtx.exitVisualMode = onExitVisualMode ?? null;
    vimCtx.visualNav = onVisualNav ?? null;
    vimCtx.visualDelete = onVisualDelete ?? null;
    vimCtx.visualYank = onVisualYank ?? null;
    vimCtx.bulkTagPicker = onBulkTagPicker ?? null;
    vimCtx.bulkIndent = onBulkIndent ?? null;
    vimCtx.toggleFold = onToggleFold ?? null;
    vimCtx.toggleProps = onToggleProps ?? null;
    vimCtx.pageJump = onPageJump ?? null;
    vimCtx.undoOutliner = onUndoOutliner ?? null;
    vimCtx.redoOutliner = onRedoOutliner ?? null;
    vimCtx.beginInsertSession = onBeginInsertSession ?? null;
    vimCtx.endInsertSession = onEndInsertSession ?? null;
    // Only expose drawer-tab cycling when inside a pinned-tab editor so that
    // gt / gT in the main focus-area editor remains a no-op.
    vimCtx.cycleDrawerTab = isPinnedTab ? cycleBottomDrawerTab : null;
  }
  function clearVimCtxIfMine() {
    if (vimCtx.view === view) vimCtx.view = null;
  }

  onMount(() => {
    const theme = EditorView.theme({
      "&": { backgroundColor: "transparent", color: "var(--foreground)", fontSize: "14.5px", fontFamily: "'Source Sans 3', -apple-system, system-ui, sans-serif", lineHeight: "1.7" },
      ".cm-content": { caretColor: "var(--primary)", padding: "0" },
      ".cm-line": { padding: "2px 0" },
      ".cm-cursor, .cm-fat-cursor": { display: "none" },
      "&.cm-focused .cm-cursor": { display: "block", borderLeftColor: "var(--primary)", borderLeftWidth: "2px" },
      "&.cm-focused .cm-fat-cursor": { display: "block", background: "color-mix(in srgb, var(--primary) 25%, transparent) !important" },
      "&.cm-focused .cm-selectionBackground, .cm-selectionBackground": { backgroundColor: "color-mix(in srgb, var(--primary) 15%, transparent) !important" },
      ".cm-gutters": { display: "none" },
      "&.cm-focused": { outline: "none" },
    });

    const focusBlurHandler = EditorView.domEventHandlers({
      focus: () => { wireVimCtx(); onFocus?.(); return false; },
      blur: () => { clearVimCtxIfMine(); if (!showSlashMenu) onBlur(); return false; },
      // Phase 9.5b — wiki-link click navigates via gotoNote when vim is in
      // NORMAL mode. INSERT mode falls through so the click places the cursor.
      // Modifier-click also falls through so cmd/ctrl+click can open a new tab.
      mousedown: (e: MouseEvent, v) => {
        if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return false;
        if (e.button !== 0) return false;
        const tgt = e.target as HTMLElement;
        const linkEl = tgt.closest?.(".cm-tesela-wikilink, .cm-tesela-wikilink-bracket");
        if (!linkEl) return false;
        if (getVimMode() !== "NORMAL") return false;
        const pos = v.posAtCoords({ x: e.clientX, y: e.clientY });
        if (pos === null) return false;
        const doc = v.state.doc.toString();
        for (const m of doc.matchAll(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g)) {
          const start = m.index ?? -1;
          if (start < 0) continue;
          if (pos >= start && pos <= start + m[0].length) {
            const linkTarget = m[1].trim();
            if (linkTarget) {
              e.preventDefault();
              e.stopPropagation();
              gotoNote(linkTarget);
              return true;
            }
            break;
          }
        }
        return false;
      },
    });

    const inputHandler = EditorView.inputHandler.of((v, from, _to, inserted) => {
      // Slash commands: / at start or after whitespace
      if (inserted === "/") {
        const docBefore = v.state.doc.sliceString(0, from);
        const isAtStart = docBefore.trim() === "";
        const isAfterSpace = docBefore.endsWith(" ") || docBefore.endsWith("\n");
        if (isAtStart || isAfterSpace) {
          setTimeout(() => {
            if (!view) return;
            slashStartPos = from;
            showSlashMenu = true;
            const coords = view.coordsAtPos(from + 1);
            if (coords) {
              slashPosition = { x: coords.left, y: coords.bottom + 4 };
            } else {
              const rect = container.getBoundingClientRect();
              slashPosition = { x: rect.left, y: rect.bottom + 4 };
            }
          }, 0);
        }
      }

      // Tag autocomplete: # after space or at start
      if (inserted === "#") {
        const docBefore = v.state.doc.sliceString(0, from);
        const isAtStart = docBefore.trim() === "";
        const isAfterSpace = docBefore.endsWith(" ") || docBefore.endsWith("\n");
        if (isAtStart || isAfterSpace) {
          setTimeout(() => {
            if (!view) return;
            autocompleteStartPos = from;
            autocompleteType = "tag";
            showAutocomplete = true;
            autocompleteFilter = "";
            const coords = view.coordsAtPos(from + 1);
            autocompletePosition = coords
              ? { x: coords.left, y: coords.bottom + 4 }
              : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
          }, 0);
        }
      }

      // Wiki-link autocomplete: [[ (detect second [)
      if (inserted === "[") {
        const docBefore = v.state.doc.sliceString(0, from);
        if (docBefore.endsWith("[")) {
          setTimeout(() => {
            if (!view) return;
            autocompleteStartPos = from - 1; // position of first [
            autocompleteType = "link";
            showAutocomplete = true;
            autocompleteFilter = "";
            const coords = view.coordsAtPos(from + 1);
            autocompletePosition = coords
              ? { x: coords.left, y: coords.bottom + 4 }
              : { x: container.getBoundingClientRect().left, y: container.getBoundingClientRect().bottom + 4 };
          }, 0);
        }
      }

      return false;
    });

    const blockKeymap = keymap.of([
      {
        key: "Escape",
        run: (v) => {
          // Phase 10.3 — slash menu is a ChordMenu now; it handles Esc at
          // capture phase before this runs, so we don't need a `showSlashMenu`
          // branch here. Autocomplete (#, [[) still uses the older filter
          // component and is handled below.
          if (showAutocomplete) { showAutocomplete = false; autocompleteFilter = ""; autocompleteStartPos = -1; return true; }
          if (inVisualMode) { onExitVisualMode?.(); return true; }
          // Let vim handle Escape when in insert/visual mode so it can transition to normal.
          // Only intercept when already in normal mode (to unfocus the block).
          const cm = getCM(v);
          const vimState = cm?.state?.vim as { insertMode?: boolean; visualMode?: boolean } | undefined;
          if (vimState?.insertMode || vimState?.visualMode) return false;
          onEscape?.();
          return true;
        },
      },
      {
        key: "ArrowUp",
        run: (v) => {
          if (showAutocomplete) return autocompleteRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowUp" })) ?? false;
          // Phase 9.9 follow-up — cross-block when no visual line above.
          // Compares y-coord before/after a synthetic cursorLineUp, which
          // respects wrapped lines. Returning false yields to cm6's default
          // ArrowUp (visual-line up); we only intercept at the top edge.
          const before = v.coordsAtPos(v.state.selection.main.head);
          const probe = v.moveVertically(v.state.selection.main, false);
          const after = v.coordsAtPos(probe.head);
          if (!before || !after || Math.abs(after.top - before.top) <= 1) {
            onNavigate?.("up");
            return true;
          }
          return false;
        },
      },
      {
        key: "ArrowDown",
        run: (v) => {
          if (showAutocomplete) return autocompleteRef?.handleKeydown(new KeyboardEvent("keydown", { key: "ArrowDown" })) ?? false;
          const before = v.coordsAtPos(v.state.selection.main.head);
          const probe = v.moveVertically(v.state.selection.main, true);
          const after = v.coordsAtPos(probe.head);
          if (!before || !after || Math.abs(after.top - before.top) <= 1) {
            onNavigate?.("down");
            return true;
          }
          return false;
        },
      },
      {
        key: "Enter",
        run: (v) => {
          // Phase 10.3 — slash menu is a ChordMenu now; chord matching
          // happens at capture phase. No Enter forwarding needed.
          if (showAutocomplete) {
            autocompleteRef?.handleKeydown(new KeyboardEvent("keydown", { key: "Enter" }));
            return true;
          }
          if (onEnter) {
            const doc = v.state.doc.toString();
            const cursor = v.state.selection.main.head;
            const firstNl = doc.indexOf("\n");
            const cursorOnFirstLine = firstNl === -1 || cursor <= firstNl;
            // Phase 10.1 follow-up — when a block has continuation lines
            // (status:: / tags:: / etc. — anything indented after the bullet
            // line) and the user presses Enter on the FIRST line, keep
            // those continuation lines with the CURRENT block. The previous
            // implementation split at cursor unconditionally, so cycling
            // status with Cmd+Enter and then pressing Enter pulled the
            // `status:: doing` line down onto the new (empty) block. Multi-
            // line content edits (cursor past first line) keep the old
            // split-at-cursor behavior.
            if (cursorOnFirstLine && firstNl !== -1) {
              const firstLine = doc.slice(0, firstNl);
              const continuation = doc.slice(firstNl); // includes leading \n
              const beforeCursor = firstLine.slice(0, cursor);
              const afterCursor = firstLine.slice(cursor);
              onChange(beforeCursor + continuation);
              onEnter(afterCursor);
            } else {
              const textBefore = doc.slice(0, cursor);
              const textAfter = doc.slice(cursor);
              if (textAfter) onChange(textBefore);
              onEnter(textAfter);
            }
            return true;
          }
          return false;
        },
      },
      { key: "Mod-Enter", run: () => { if (onCycleStatus) { onCycleStatus(); return true; } return false; } },
      { key: "Tab", run: () => { if (onIndent) { onIndent("indent"); return true; } return false; } },
      { key: "Shift-Tab", run: () => { if (onIndent) { onIndent("outdent"); return true; } return false; } },
      // Phase 9.9 follow-up — Ctrl+U / Ctrl+D as outliner page-jump in vim
      // NORMAL mode. Routed through blockKeymap (cm6 level) instead of vim
      // mapCommand because cm6's standardKeymap on macOS catches Ctrl+D as
      // `deleteCharForward` BEFORE cm-vim's domEventHandlers can intercept.
      // In INSERT mode we yield (return false) so cm-vim's default insert-mode
      // bindings (`<C-d>` = decrease indent, `<C-u>` = delete to line start)
      // still apply.
      {
        key: "Ctrl-d",
        run: (v) => {
          const cm = getCM(v);
          const vs = cm?.state?.vim as { insertMode?: boolean } | undefined;
          if (vs?.insertMode) return false;
          if (!onPageJump) return false;
          onPageJump("down");
          return true;
        },
      },
      {
        key: "Ctrl-u",
        run: (v) => {
          const cm = getCM(v);
          const vs = cm?.state?.vim as { insertMode?: boolean } | undefined;
          if (vs?.insertMode) return false;
          if (!onPageJump) return false;
          onPageJump("up");
          return true;
        },
      },
      // Phase 10.2 follow-up — `g` in NORMAL mode opens the leader chord
      // menu pre-descended into "Go to". Routed via cm6 keymap (Prec.high
      // by extension order) so it pre-empts cm-vim's `g`-prefix state. In
      // INSERT/VISUAL we return false so the user can still type `g` or
      // run vim's visual `g` operators.
      {
        key: "g",
        run: (v) => {
          const cm = getCM(v);
          const vs = cm?.state?.vim as { insertMode?: boolean; visualMode?: boolean } | undefined;
          if (vs?.insertMode || vs?.visualMode) return false;
          document.dispatchEvent(new CustomEvent("tesela:open-leader-at", { detail: { path: ["Go to"] } }));
          return true;
        },
      },
      {
        key: "Backspace",
        run: (v) => {
          if (v.state.doc.length === 0 && onBackspaceEmpty) { onBackspaceEmpty(); return true; }
          // Merge with previous block when Backspace at cursor pos 0 with content
          const cursor = v.state.selection.main.head;
          if (cursor === 0 && v.state.selection.main.anchor === 0 && v.state.doc.length > 0 && onBackspaceMerge) {
            onBackspaceMerge(v.state.doc.toString());
            return true;
          }
          return false;
        },
      },
    ]);

    const updateListener = EditorView.updateListener.of((update) => {
      if (update.docChanged) {
        // Skip echoing back outliner-undo restores — they already wrote the
        // canonical block.body, so re-firing onChange would corrupt history.
        if (update.transactions.some((tr) => tr.annotation(externalSync) === true)) return;
        const doc = update.state.doc.toString();
        onChange(doc);
        const cursorPos = update.state.selection.main.head;
        // Phase 10.3 — chord menu doesn't have a filter to update. We
        // still close the menu if the cursor backs up past the `/` (e.g.
        // user manually deletes it). All single-char keys are otherwise
        // swallowed by ChordMenu's capture-phase handler so typing past
        // the `/` shouldn't happen — but be defensive.
        if (showSlashMenu && slashStartPos >= 0 && cursorPos <= slashStartPos) {
          showSlashMenu = false;
          slashStartPos = -1;
        }
        // Update autocomplete filter
        if (showAutocomplete && autocompleteStartPos >= 0) {
          // Skip the trigger characters that come before the filter text:
          // tag: "#" (1), link: "[[" (2), tagmanage: nothing (0)
          const offset = autocompleteType === "tag" ? 1 : autocompleteType === "link" ? 2 : 0;
          if (cursorPos < autocompleteStartPos + offset) {
            showAutocomplete = false; autocompleteFilter = ""; autocompleteStartPos = -1;
          } else {
            autocompleteFilter = doc.slice(autocompleteStartPos + offset, cursorPos);
          }
        }
      }
    });

    const clampedCursor = initialCursorPos !== undefined
      ? Math.max(0, Math.min(initialText.length, initialCursorPos))
      : undefined;
    const state = EditorState.create({
      doc: initialText,
      selection: clampedCursor !== undefined ? { anchor: clampedCursor, head: clampedCursor } : undefined,
      extensions: [
        blockKeymap,
        vim(),
        keymap.of([...defaultKeymap, ...historyKeymap]),
        history(),
        // Renders `.cm-cursor` in insert mode (the native browser caret is
        // only 1px and easy to miss on empty blocks). cm-vim hides this
        // layer via `.cm-vimMode .cm-cursorLayer:not(.cm-vimCursorLayer)` in
        // normal/visual mode, so its own fat-cursor stays the only cursor.
        drawSelection(),
        theme,
        inputHandler,
        updateListener,
        focusBlurHandler,
        teselaDecorations,
        teselaDecorationTheme,
        hiddenKeysCompartment.of(
          hiddenPropertyKeysFacet.of(hiddenKeys ?? { hide: new Set(), hideEmpty: new Set() }),
        ),
        primaryTagCompartment.of(primaryTagFacet.of(primaryTag ?? null)),
        EditorView.lineWrapping,
      ],
    });

    view = new EditorView({ state, parent: container });

    // If an initial cursor position was requested, focus immediately
    if (clampedCursor !== undefined) {
      view.focus();
    }

    // Register global vim actions once (no-op after first call) and wire
    // mode-change tracking via the CM5-like instance event (not a DOM event).
    let vimModeOff: (() => void) | null = null;
    const cm = getCM(view);
    if (cm) {
      initVimActions();
      const modeListener = (info: { mode: string }) => {
        setVimMode(info.mode);
        // Cache a pre-edit outliner snapshot on Insert entry; clear it on
        // any other mode (the snapshot is promoted on the first keystroke
        // by handleBlockChange — see Phase 3M.1 plan).
        if (info.mode === "insert") vimCtx.beginInsertSession?.();
        else vimCtx.endInsertSession?.();
      };
      cm.on("vim-mode-change", modeListener);
      vimModeOff = () => cm.off("vim-mode-change", modeListener);
    }

    // Focus and optionally enter insert mode for newly created blocks.
    // Snapshot `startInInsert` BEFORE the rAF — `view.focus()` below fires
    // the cm-content DOM focus event, which BlockOutliner's per-row onfocus
    // handler treats as user-initiated and clears `restoredFocus`, causing
    // a re-read of the prop here to flip from false to true and incorrectly
    // drop us into Insert on undo/redo restores. The snapshot makes the
    // decision sticky to mount time.
    //
    // The `!autoFocused` gate matches the gate on the reactive $effect
    // a few hundred lines up. Parent's auto-focus path (focusedIndex=0
    // on mount of a freshly visible day section) sets `autoFocused=true`
    // to mean "decorative only — don't take keyboard focus." Without
    // this gate, view.focus() below fires the focus DOM event → parent's
    // onfocus flips autoFocused=false → the $effect re-evaluates and
    // sees startInInsert=true for an empty block → lands the user in
    // INSERT on a block they navigated into via cross-day j/k.
    if (focused && !autoFocused) {
      const shouldStartInInsert = startInInsert;
      requestAnimationFrame(() => {
        if (!view) return;
        view.focus();
        // Only move cursor to end when no explicit initialCursorPos was given;
        // for split/merge blocks the EditorState already has the right selection.
        if (clampedCursor === undefined) {
          view.dispatch({ selection: { anchor: view.state.doc.length } });
        }
        if (shouldStartInInsert) {
          const cm2 = getCM(view);
          if (cm2) Vim.handleKey(cm2, "i", "mapping");
        }
      });
    }

    return () => {
      vimModeOff?.();
      view?.destroy();
      view = null;
    };
  });

  // Phase 10.2 follow-up — leader-menu `g f` (Follow wiki link) dispatches
  // `tesela:block-action` with kind=`followWiki`. Only the BlockEditor
  // whose cm6 view actually has DOM focus runs the cursor-position scan;
  // others ignore the event. Mirrors the existing `gd` vim action body.
  onMount(() => {
    const handler = (e: Event) => {
      const detail = (e as CustomEvent).detail as { kind?: string };
      if (detail?.kind !== "followWiki") return;
      if (!view || !focused) return;
      const pos = view.state.selection.main.head;
      const doc = view.state.doc.toString();
      for (const m of doc.matchAll(/\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g)) {
        const start = m.index ?? -1;
        if (start < 0) continue;
        if (pos >= start && pos <= start + m[0].length) {
          const target = m[1].trim();
          if (target) gotoNote(target);
          return;
        }
      }
    };
    document.addEventListener("tesela:block-action", handler);
    return () => document.removeEventListener("tesela:block-action", handler);
  });
</script>

<div class="relative">
  <div bind:this={container} class="text-sm leading-relaxed min-h-[24px]"></div>

  {#if showSlashMenu}
    <ChordMenu
      tree={getSlashTree()}
      position={slashPosition}
      headLabel="/"
      onclose={() => {
        showSlashMenu = false;
        slashStartPos = -1;
        // Restore DOM focus to the cm-editor so the user keeps typing
        // wherever they left off — ChordMenu doesn't take focus, but the
        // overlay click can blur as a side-effect on some browsers.
        view?.focus();
      }}
    />
  {/if}

  {#if showAutocomplete}
    <AutocompleteMenu
      bind:this={autocompleteRef}
      items={autocompleteType === "tagmanage" ? tagManageItems : autocompleteType === "templatepick" ? templatePickItems : autocompleteItems}
      filter={autocompleteFilter}
      position={autocompletePosition}
      onselect={(item) => applyAutocomplete(item)}
      onclose={() => { showAutocomplete = false; autocompleteFilter = ""; autocompleteStartPos = -1; }}
    />
  {/if}

  {#if showDatePicker}
    <DatePicker
      position={datePickerPosition}
      onPick={(iso, _time, recurrence) => {
        if (view && datePickerCursor >= 0) {
          if (datePickerPropertyKey) {
            // Phase 10.5 — date-typed property: upsert via the same helper
            // the chord-leaf path uses, so picking a date for an existing
            // `deadline::` line replaces it instead of duplicating.
            // Phase 12.2 — when the picker emits a recurrence, also upsert
            // `recurring::` so a single keystroke commits both properties.
            const doc = view.state.doc.toString();
            let next = upsertBlockProperty(doc, datePickerPropertyKey, `[[${iso}]]`);
            if (recurrence !== null) {
              next = upsertBlockProperty(next, "recurring", recurrence);
            }
            view.dispatch({
              changes: { from: 0, to: doc.length, insert: next },
              selection: { anchor: datePickerCursor },
            });
            onChange(next);
          } else {
            const doc = view.state.doc.toString();
            const before = doc.slice(0, datePickerCursor);
            const after = doc.slice(datePickerCursor);
            const inserted = `[[${iso}]]`;
            const next = before + inserted + after;
            view.dispatch({
              changes: { from: 0, to: doc.length, insert: next },
              selection: { anchor: before.length + inserted.length },
            });
            onChange(next);
          }
          view.focus();
        }
        showDatePicker = false;
        datePickerCursor = -1;
        datePickerPropertyKey = null;
      }}
      onClose={() => { showDatePicker = false; datePickerCursor = -1; datePickerPropertyKey = null; view?.focus(); }}
    />
  {/if}
</div>
