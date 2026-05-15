/**
 * CodeMirror 6 decorations for Tesela block content:
 * - #tags hidden as atomic empty widgets (legacy inline tags from old notes;
 *   new tags live in the `tags::` block property and never appear inline)
 * - [[wiki-links]] as styled link
 * - key:: value as styled property; specific keys (configured per-block via
 *   the hiddenPropertyKeysFacet) get a hide-class that the parent
 *   `.show-props` ancestor can override
 */
import { EditorView, Decoration, WidgetType, type DecorationSet, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { EditorSelection, EditorState, Facet, RangeSet, RangeSetBuilder, Transaction } from "@codemirror/state";

class EmptyWidget extends WidgetType {
  toDOM() { return document.createElement("span"); }
  eq() { return true; }
}

// Phase 9.4's inline KindBadgeWidget (the all-caps red TASK / URGENT chip
// prepended to block-line 0) was removed in 9.7 — the right-side tag pill
// is the canonical kind indicator now, freeing the left edge of the editor
// for typing. The `primaryTagFacet` below is kept defined in case another
// surface (e.g. read-only block reference card) wants the kind inline.

const tagHide = Decoration.replace({ widget: new EmptyWidget() });
const bidHide = Decoration.replace({ widget: new EmptyWidget() });
const wikiLinkMark = Decoration.mark({ class: "cm-tesela-wikilink" });
const wikiLinkBracketMark = Decoration.mark({ class: "cm-tesela-wikilink-bracket" });
const propertyKeyMark = Decoration.mark({ class: "cm-tesela-prop-key" });
const propertyValueMark = Decoration.mark({ class: "cm-tesela-prop-value" });

const tagsLineHide = Decoration.line({ attributes: { class: "cm-tesela-tags-line" } });
const hiddenPropLineDeco = Decoration.line({ attributes: { class: "cm-tesela-hidden-prop-line" } });

const TAG_RE = /#([A-Za-z0-9_/-]+)/g;
// Persistent block-id comments emitted by the server's note_tree serializer.
// One leading whitespace char (canonical form: a single space) is included so
// the visible line ends exactly where the user expects.
const BID_RE = /[ \t]?<!-- bid:[0-9a-fA-F-]+ -->/g;
const WIKI_LINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;
// Allow empty values so we can decorate auto-filled `key:: ` (no value yet)
// lines too. Use `[ \t]?` (not `\s?`) for the separator — `\s` would match a
// newline and let `(.*)` greedily eat the next line into the value group.
const PROPERTY_RE = /^([A-Za-z_][A-Za-z0-9_]*)::[ \t]?(.*)$/gm;
// `tags:: ...` lines are managed via pills and the /tag command — hidden from
// the editor via display:none on the line.
const TAGS_LINE_RE = /^tags:: .+$/gm;

/**
 * Per-block configuration for which property keys to hide in the editor.
 * Population: the parent (BlockOutliner) computes this set from the block's
 * inherited tag-property defs and writes it via the compartment-wrapped facet.
 */
export type HiddenKeysConfig = {
  /** Property keys (lowercase) that are unconditionally hidden by default. */
  hide: ReadonlySet<string>;
  /** Property keys (lowercase) hidden only when their value is empty. */
  hideEmpty: ReadonlySet<string>;
};

const EMPTY_HIDDEN_KEYS: HiddenKeysConfig = { hide: new Set(), hideEmpty: new Set() };

export const hiddenPropertyKeysFacet = Facet.define<HiddenKeysConfig, HiddenKeysConfig>({
  combine: (values) => values[0] ?? EMPTY_HIDDEN_KEYS,
});

/**
 * Phase 9.4 — primary tag (kind) of the surrounding block, surfaced via the
 * outliner's `block.tags[0]`. Drives the kind-glyph badge decoration. `null`
 * (or absent) means no badge.
 */
export const primaryTagFacet = Facet.define<string | null, string | null>({
  combine: (values) => values[0] ?? null,
});

type Built = { decorations: DecorationSet; atomicTags: RangeSet<Decoration> };

function buildDecorations(view: EditorView): Built {
  const builder = new RangeSetBuilder<Decoration>();
  const atomicBuilder = new RangeSetBuilder<Decoration>();
  const doc = view.state.doc.toString();
  const config = view.state.facet(hiddenPropertyKeysFacet);

  const decos: Array<{ from: number; to: number; decoration: Decoration; atomic?: boolean }> = [];

  // Tags: always hidden + atomic so cursor jumps over the token as one unit
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + m[0].length, decoration: tagHide, atomic: true });
  }

  // tags:: property lines: hide the whole line (canonical display is the pill UI)
  TAGS_LINE_RE.lastIndex = 0;
  while ((m = TAGS_LINE_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index, decoration: tagsLineHide });
  }

  // Block-id comments: hide entirely + atomic so cursor jumps past them.
  BID_RE.lastIndex = 0;
  while ((m = BID_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + m[0].length, decoration: bidHide, atomic: true });
  }
  // Phase 9.7 — the inline kind-glyph badge that used to prepend "TASK" /
  // "URGENT" to block-line 0 was removed in favor of the standalone tag pill
  // on the right side of the block. Keeps the editor's left edge clean for
  // typing. The `primaryTagFacet` and `KindBadgeWidget` are kept around in
  // case a future surface (e.g. read-only block reference) wants to surface
  // the kind inline; they're just not wired into the editor decorations now.

  // Wiki-links
  WIKI_LINK_RE.lastIndex = 0;
  while ((m = WIKI_LINK_RE.exec(doc)) !== null) {
    decos.push({ from: m.index, to: m.index + 2, decoration: wikiLinkBracketMark });
    decos.push({ from: m.index + 2, to: m.index + m[0].length - 2, decoration: wikiLinkMark });
    decos.push({ from: m.index + m[0].length - 2, to: m.index + m[0].length, decoration: wikiLinkBracketMark });
  }

  // Properties: emit key/value styling, plus a hide-class line decoration if
  // configured to hide via the facet (either always-hide or empty-hide+empty).
  PROPERTY_RE.lastIndex = 0;
  while ((m = PROPERTY_RE.exec(doc)) !== null) {
    const key = m[1].toLowerCase();
    if (key === "tags") continue; // tags:: handled above
    const value = m[2] ?? "";
    const isEmpty = value.trim() === "";

    const shouldHide = config.hide.has(key) || (isEmpty && config.hideEmpty.has(key));
    if (shouldHide) {
      decos.push({ from: m.index, to: m.index, decoration: hiddenPropLineDeco });
    }

    const keyEnd = m.index + m[1].length + 2; // `key::`
    decos.push({ from: m.index, to: keyEnd, decoration: propertyKeyMark });
    // Skip value mark when value range is empty (zero-width marks are invalid).
    const valueStart = keyEnd + (m[0].length > m[1].length + 2 ? 1 : 0); // after `:: `
    const valueEnd = m.index + m[0].length;
    if (valueEnd > valueStart) {
      decos.push({ from: valueStart, to: valueEnd, decoration: propertyValueMark });
    }
  }

  decos.sort((a, b) => a.from - b.from || a.to - b.to);
  for (const d of decos) {
    builder.add(d.from, d.to, d.decoration);
    if (d.atomic) atomicBuilder.add(d.from, d.to, d.decoration);
  }
  return { decorations: builder.finish(), atomicTags: atomicBuilder.finish() };
}

export const teselaDecorations = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet;
    atomicTags: RangeSet<Decoration>;
    constructor(view: EditorView) {
      const built = buildDecorations(view);
      this.decorations = built.decorations;
      this.atomicTags = built.atomicTags;
    }
    update(update: ViewUpdate) {
      const hiddenChanged =
        update.startState.facet(hiddenPropertyKeysFacet) !==
        update.state.facet(hiddenPropertyKeysFacet);
      const primaryChanged =
        update.startState.facet(primaryTagFacet) !==
        update.state.facet(primaryTagFacet);
      if (update.docChanged || update.viewportChanged || hiddenChanged || primaryChanged) {
        const built = buildDecorations(update.view);
        this.decorations = built.decorations;
        this.atomicTags = built.atomicTags;
      }
    }
  },
  {
    decorations: (v) => v.decorations,
    provide: (plugin) => EditorView.atomicRanges.of((view) => view.plugin(plugin)?.atomicTags ?? RangeSet.empty),
  },
);

// ── atomic-cursor transaction filter ────────────────────────────────────────
//
// `EditorView.atomicRanges` is only consulted by the BUILT-IN cursor motion
// commands. `@replit/codemirror-vim`'s `h`/`l` go through `moveByCharacters`
// → `setCursor(line, cur.ch ± 1)` which dispatches a plain selection
// transaction without checking atomicRanges (see dist/index.js:2389 + 7258).
// The cursor visibly gets stuck at the empty `widgetBuffer` span and goes
// invisible until enough `l` presses exhaust the underlying hidden chars.
//
// The fix runs as a `transactionFilter`: any selection that lands inside an
// atomic range gets snapped to the appropriate edge (forward motion → `to`,
// backward → `from`). Catches cm-vim, mouse clicks, and any other consumer
// that dispatches a selection without checking atomicRanges.
//
// Atomic ranges covered:
//   - inline `#tags` (TAG_RE) — fully hidden via tagHide widget
//   - block-id comments (BID_RE) — fully hidden via bidHide widget
//   - whole `tags:: ...` lines — hidden via `display:none` line decoration
//   - hidden `key:: value` property lines per the hiddenPropertyKeysFacet

/**
 * Pure: returns sorted, non-overlapping `[from, to)` byte ranges in `doc`
 * that the cursor should skip over. Exported so the unit tests can exercise
 * the snap logic without a live editor.
 */
export function findAtomicCursorRanges(doc: string, config: HiddenKeysConfig): Array<[number, number]> {
  const ranges: Array<[number, number]> = [];

  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    ranges.push([m.index, m.index + m[0].length]);
  }

  BID_RE.lastIndex = 0;
  while ((m = BID_RE.exec(doc)) !== null) {
    ranges.push([m.index, m.index + m[0].length]);
  }

  // For full-line hides, include the trailing newline so `j` from above
  // moves all the way past the hidden line to the line below.
  function lineRangeWithNewline(lineStart: number, lineLen: number): [number, number] {
    const end = lineStart + lineLen;
    const after = doc.charCodeAt(end) === 10 ? end + 1 : end; // \n
    return [lineStart, after];
  }

  TAGS_LINE_RE.lastIndex = 0;
  while ((m = TAGS_LINE_RE.exec(doc)) !== null) {
    ranges.push(lineRangeWithNewline(m.index, m[0].length));
  }

  if (config.hide.size > 0 || config.hideEmpty.size > 0) {
    PROPERTY_RE.lastIndex = 0;
    while ((m = PROPERTY_RE.exec(doc)) !== null) {
      const key = m[1].toLowerCase();
      if (key === "tags") continue;
      const value = m[2] ?? "";
      const isEmpty = value.trim() === "";
      if (config.hide.has(key) || (isEmpty && config.hideEmpty.has(key))) {
        ranges.push(lineRangeWithNewline(m.index, m[0].length));
      }
    }
  }

  ranges.sort((a, b) => a[0] - b[0] || a[1] - b[1]);
  return ranges;
}

/**
 * Pure: snap `newHead` out of any atomic range. Direction is inferred from
 * `oldHead` — forward motion lands at `to`, backward at `from`. The far
 * edge of the range is "inclusive" when motion is INTO it (forward into
 * `from` snaps past to `to`, backward into `to` snaps back to `from`)
 * because a 0-width hidden widget collapses `from` and `to` onto the same
 * physical column — without this, vim `l` from the char just before a
 * `#tag` feels stuck for an extra press. The "from" edge stays selectable
 * when motion is away from the range (cursor just left from to-1 backward
 * etc.), and any boundary visit with `oldHead === newHead` (mouse click
 * etc.) keeps the original head so external selections aren't moved.
 *
 * The transactionFilter that owns this skips doc-changing transactions
 * entirely, so this snap never fires during typing.
 */
export function snapHeadOutOfAtomicRanges(
  newHead: number,
  oldHead: number,
  ranges: ReadonlyArray<readonly [number, number]>,
): number {
  for (const [from, to] of ranges) {
    if (newHead < from) break; // ranges sorted
    if (newHead > to) continue;
    const forward = newHead > oldHead;
    const backward = newHead < oldHead;
    if (forward && newHead >= from && newHead < to) return to;
    if (backward && newHead > from && newHead <= to) return from;
  }
  return newHead;
}

export const teselaAtomicCursorFilter = EditorState.transactionFilter.of((tr) => {
  // Skip doc-changing transactions (typing, paste, undo): the post-edit
  // cursor placement is the editor's own concern and snapping there would
  // teleport the cursor across a tag the user just typed in front of.
  if (!tr.selection || tr.docChanged) return tr;
  const config = tr.startState.facet(hiddenPropertyKeysFacet);
  const ranges = findAtomicCursorRanges(tr.newDoc.toString(), config);
  if (ranges.length === 0) return tr;

  const oldSel = tr.startState.selection;
  let changed = false;
  const adjusted = tr.newSelection.ranges.map((r, i) => {
    const oldHead = oldSel.ranges[i]?.head ?? -1;
    const head = snapHeadOutOfAtomicRanges(r.head, oldHead, ranges);
    if (head === r.head) return r;
    changed = true;
    // Collapse anchor to head when the prior selection was a cursor (empty)
    // so we don't accidentally turn a vim `l` into a visual selection.
    const anchor = r.anchor === r.head ? head : r.anchor;
    return EditorSelection.range(anchor, head);
  });
  if (!changed) return tr;

  // Rebuild as a spec. Forward the annotations we care about — userEvent
  // (history grouping) and remote (collaboration). Other annotations are
  // rare on selection-only transactions and would just survive as-is on
  // the original tr anyway (this filter only fires when we actually shift
  // the selection).
  const userEvent = tr.annotation(Transaction.userEvent);
  const remote = tr.annotation(Transaction.remote);
  const annotations = [
    ...(userEvent !== undefined ? [Transaction.userEvent.of(userEvent)] : []),
    ...(remote !== undefined ? [Transaction.remote.of(remote)] : []),
  ];
  return [
    {
      changes: tr.changes,
      selection: EditorSelection.create(adjusted, tr.newSelection.mainIndex),
      effects: tr.effects,
      scrollIntoView: tr.scrollIntoView,
      annotations: annotations.length > 0 ? annotations : undefined,
    },
  ];
});

export const teselaDecorationTheme = EditorView.theme({
  ".cm-tesela-tags-line": {
    display: "none",
  },
  ".cm-tesela-wikilink": {
    color: "var(--primary)",
    textDecoration: "underline",
    textDecorationColor: "color-mix(in srgb, var(--primary) 30%, transparent)",
    textUnderlineOffset: "3px",
    textDecorationThickness: "1px",
  },
  ".cm-tesela-wikilink-bracket": {
    color: "var(--muted-foreground)",
    opacity: "0.4",
    fontSize: "0.85em",
  },
  ".cm-tesela-prop-key": {
    color: "var(--muted-foreground)",
    fontSize: "0.9em",
  },
  ".cm-tesela-prop-value": {
    color: "color-mix(in srgb, var(--primary) 50%, var(--foreground))",
    fontSize: "0.9em",
  },
});
