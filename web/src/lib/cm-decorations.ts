/**
 * CodeMirror 6 decorations for Tesela block content:
 * - #tags inside the block render as styled marks (`cm-tesela-tag`).
 * - #tags at the trailing end of a block (the "trailing cluster" per the
 *   tag-system spec) render as chip widgets (atomic, clickable, opens the
 *   tag page on click).
 * - [[wiki-links]] as styled link.
 * - key:: value as styled property; specific keys (configured per-block via
 *   the hiddenPropertyKeysFacet) get a hide-class that the parent
 *   `.show-props` ancestor can override.
 */
import { EditorView, Decoration, WidgetType, type DecorationSet, ViewPlugin, type ViewUpdate } from "@codemirror/view";
import { EditorSelection, EditorState, Facet, RangeSet, RangeSetBuilder, Transaction } from "@codemirror/state";

class EmptyWidget extends WidgetType {
  toDOM() { return document.createElement("span"); }
  eq() { return true; }
}

/** Trailing-cluster chip widget. Renders a clickable `#name` pill at the
 *  end of the block. Clicking dispatches a `tesela:open-tag` custom event
 *  on document so the host (BlockOutliner / BufferShell) can navigate to
 *  the tag's page. The widget owns the DOM but defers navigation to the
 *  host — same Reference-driven flow as wiki-link clicks. */
class TagChipWidget extends WidgetType {
  readonly name: string;
  constructor(name: string) {
    super();
    this.name = name;
  }
  toDOM() {
    const el = document.createElement("button");
    el.type = "button";
    el.className = "cm-tesela-tag-chip";
    el.textContent = `#${this.name}`;
    el.title = `open ${this.name}`;
    el.addEventListener("mousedown", (e) => {
      // Prevent the editor from stealing focus before our click fires.
      e.preventDefault();
    });
    el.addEventListener("click", (e) => {
      e.preventDefault();
      e.stopPropagation();
      document.dispatchEvent(
        new CustomEvent("tesela:open-tag", { detail: { value: this.name } }),
      );
    });
    return el;
  }
  eq(other: TagChipWidget) {
    return other.name === this.name;
  }
  ignoreEvent() { return false; }
}

// Phase 9.4's inline KindBadgeWidget (the all-caps red TASK / URGENT chip
// prepended to block-line 0) was removed in 9.7 — the right-side tag pill
// is the canonical kind indicator now, freeing the left edge of the editor
// for typing. The `primaryTagFacet` below is kept defined in case another
// surface (e.g. read-only block reference card) wants the kind inline.

const tagInlineMark = Decoration.mark({ class: "cm-tesela-tag" });
const bidHide = Decoration.replace({ widget: new EmptyWidget() });
const wikiLinkMark = Decoration.mark({ class: "cm-tesela-wikilink" });
const wikiLinkBracketMark = Decoration.mark({ class: "cm-tesela-wikilink-bracket" });
const propertyKeyMark = Decoration.mark({ class: "cm-tesela-prop-key" });
const propertyValueMark = Decoration.mark({ class: "cm-tesela-prop-value" });

const tagsLineHide = Decoration.line({ attributes: { class: "cm-tesela-tags-line" } });
const hiddenPropLineDeco = Decoration.line({ attributes: { class: "cm-tesela-hidden-prop-line" } });

// Code-fence line decorations. Each code line carries `cm-tesela-code-line`
// plus first/last/fence variant classes; cached by class string so a
// decoration rebuild reuses instances instead of allocating per line.
const codeLineDecoCache = new Map<string, Decoration>();
function codeLineDeco(cls: string): Decoration {
  let deco = codeLineDecoCache.get(cls);
  if (!deco) {
    deco = Decoration.line({ attributes: { class: cls } });
    codeLineDecoCache.set(cls, deco);
  }
  return deco;
}

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

/**
 * Pure helper: toggle a `#tag` between inline-and-trailing positions.
 *
 * - If `cursor` is inside an inline `#tag` (one that's NOT in the trailing
 *   cluster), the tag is cut out of its inline position and appended as a
 *   trailing chip. The cursor lands at the cut location.
 * - Otherwise, if a trailing chip exists, the rightmost trailing token is
 *   popped and inserted at `cursor`. The cursor lands after the insertion.
 * - If neither applies, returns null (caller should treat as no-op).
 *
 * The whole operation is a single edit, so the editor groups it as one
 * undo step.
 */
export function promoteOrDemoteTag(
  doc: string,
  cursor: number,
): { changes: { from: number; to: number; insert: string }[]; cursor: number } | null {
  const trailingStart = findTrailingClusterStart(doc);

  // Find an inline `#tag` whose range covers the cursor.
  const inlineRe = /#([A-Za-z0-9_/-]+)/g;
  let m: RegExpExecArray | null;
  let inlineHit: { from: number; to: number; name: string } | null = null;
  while ((m = inlineRe.exec(doc)) !== null) {
    const from = m.index;
    const to = m.index + m[0].length;
    if (from >= trailingStart) break; // entered the cluster
    if (cursor >= from && cursor <= to) {
      inlineHit = { from, to, name: m[1] };
      break;
    }
  }

  if (inlineHit) {
    // Demote: cut the inline tag, append to the trailing cluster.
    const beforeAppend = doc.slice(0, doc.replace(/\s+$/, "").length); // pre-trailing-whitespace
    const trailingHasContent = trailingStart < beforeAppend.length;
    const sep = trailingHasContent ? " " : doc.trim().length > 0 ? " " : "";
    const trimmedRight = doc.replace(/\s+$/, "").length;
    // Build two edits:
    //   1. Delete [inlineHit.from, inlineHit.to + 1) where the +1 strips a
    //      separator space if the next char is whitespace, so the seam
    //      doesn't leave a double space. Bounded by doc length.
    let deleteTo = inlineHit.to;
    if (deleteTo < doc.length && /\s/.test(doc[deleteTo] ?? "")) deleteTo += 1;
    //   2. Append `<sep>#name` at the position just before trailing
    //      whitespace (so the chip is the last visible thing).
    const insertAt = trimmedRight;
    const insertPiece = `${sep}#${inlineHit.name}`;
    // When the deletion is before the insertion point, the insert position
    // shifts by the deletion's length. CodeMirror handles this correctly
    // when both changes are passed as a single array; positions are in the
    // ORIGINAL doc.
    const changes = [
      { from: inlineHit.from, to: deleteTo, insert: "" },
      { from: insertAt, to: insertAt, insert: insertPiece },
    ];
    // New cursor: at the original `inlineHit.from`, after the delete.
    return { changes, cursor: inlineHit.from };
  }

  // Promote: find the rightmost trailing tag, pop it, insert at cursor.
  if (trailingStart >= doc.length) return null;
  const trailingText = doc.slice(trailingStart);
  // Tokenize: match `#name` strings inside the cluster.
  const trailingTokens: { from: number; to: number; name: string }[] = [];
  const reAll = /#([A-Za-z0-9_/-]+)/g;
  let tm: RegExpExecArray | null;
  while ((tm = reAll.exec(trailingText)) !== null) {
    const from = trailingStart + tm.index;
    trailingTokens.push({ from, to: from + tm[0].length, name: tm[1] });
  }
  if (trailingTokens.length === 0) return null;
  const popped = trailingTokens[trailingTokens.length - 1];

  // Determine the popped range with any leading whitespace (so the cluster
  // doesn't leave a stray space after pop).
  let popFrom = popped.from;
  while (popFrom > 0 && /[ \t]/.test(doc[popFrom - 1] ?? "") && popFrom - 1 >= trailingStart) {
    popFrom -= 1;
  }

  // Inline insertion piece. Add a leading space before `#tag` when the
  // character left of cursor is non-whitespace, and a trailing space when
  // the character at cursor is non-whitespace (or end-of-doc).
  const leftChar = cursor > 0 ? doc[cursor - 1] : "";
  const rightChar = cursor < doc.length ? doc[cursor] : "";
  const leftPad = leftChar && !/\s/.test(leftChar) ? " " : "";
  const rightPad = rightChar && !/\s/.test(rightChar) ? " " : "";
  const insertPiece = `${leftPad}#${popped.name}${rightPad}`;

  const changes = [
    { from: popFrom, to: popped.to, insert: "" },
    { from: cursor, to: cursor, insert: insertPiece },
  ];
  // Cursor: after the inserted piece (using the ORIGINAL cursor + leftPad +
  // `#name`).
  return {
    changes,
    cursor: cursor + leftPad.length + 1 + popped.name.length,
  };
}

/**
 * Find the start offset of the trailing-tag cluster in `doc`.
 *
 * The cluster is one or more `#tag` tokens at the end of the doc, separated
 * only by whitespace (including newlines). The returned offset is the
 * position of the first `#` in the cluster, or `doc.length` if there is no
 * trailing cluster.
 *
 * Mirrors `split_inline_and_trailing_tags` in `crates/tesela-core/src/block.rs`.
 * Both implementations must stay in sync; the regex alphabet is the same.
 */
export function findTrailingClusterStart(doc: string): number {
  let cursor = doc.length;
  // Right-trim whitespace.
  while (cursor > 0 && /\s/.test(doc[cursor - 1] ?? "")) cursor -= 1;
  let clusterStart = cursor;

  while (true) {
    // Walk left over a tag name.
    const nameEnd = cursor;
    while (cursor > 0 && /[A-Za-z0-9_/\-]/.test(doc[cursor - 1] ?? "")) cursor -= 1;
    const nameStart = cursor;
    if (nameEnd === nameStart || cursor === 0 || doc[cursor - 1] !== "#") break;
    cursor -= 1; // consume `#`
    clusterStart = cursor;
    // Walk left over whitespace before the next token in the cluster.
    while (cursor > 0 && /[ \t\n\r]/.test(doc[cursor - 1] ?? "")) cursor -= 1;
  }

  return clusterStart;
}

/** A fenced ``` code region within a block's doc. `from`/`to` span from
 *  the first character of the opening ``` line through the last character
 *  of the closing ``` line (the trailing newline is excluded). `closed`
 *  is false for a fence with no closing ``` (it runs to end-of-doc). */
export type CodeFenceRange = { from: number; to: number; closed: boolean };

/**
 * Pure: find fenced ``` code regions in `doc`. An opening fence is any
 * line whose trimmed text starts with ```` ``` ````; the closing fence is
 * a line whose trimmed text is exactly ```` ``` ````. An unclosed fence
 * runs to the end of the doc, matching CommonMark.
 *
 * Used both to paint the code surface and to suppress tag / wiki-link /
 * property parsing inside code — fenced content is never block markup.
 */
export function findCodeFenceRanges(doc: string): CodeFenceRange[] {
  const ranges: CodeFenceRange[] = [];
  const lines = doc.split("\n");
  let offset = 0;
  let openAt: number | null = null;
  for (const line of lines) {
    if (openAt === null) {
      if (line.trimStart().startsWith("```")) openAt = offset;
    } else if (line.trim() === "```") {
      ranges.push({ from: openAt, to: offset + line.length, closed: true });
      openAt = null;
    }
    offset += line.length + 1; // +1 for the consumed "\n"
  }
  if (openAt !== null) {
    ranges.push({ from: openAt, to: doc.length, closed: false });
  }
  return ranges;
}

function buildDecorations(view: EditorView): Built {
  const builder = new RangeSetBuilder<Decoration>();
  const atomicBuilder = new RangeSetBuilder<Decoration>();
  const doc = view.state.doc.toString();
  const config = view.state.facet(hiddenPropertyKeysFacet);

  const decos: Array<{ from: number; to: number; decoration: Decoration; atomic?: boolean }> = [];

  // Fenced ``` code regions. Each line of a region is painted as a code
  // surface, and the region's character range excludes its content from
  // the inline tag/wiki/property parsing below — fenced text is literal.
  const codeRanges = findCodeFenceRanges(doc);
  const insideCode = (i: number): boolean =>
    codeRanges.some((r) => i >= r.from && i < r.to);
  for (const region of codeRanges) {
    const firstLine = view.state.doc.lineAt(region.from).number;
    const lastLine = view.state.doc.lineAt(region.to).number;
    for (let ln = firstLine; ln <= lastLine; ln++) {
      const line = view.state.doc.line(ln);
      const isFirst = ln === firstLine;
      const isLast = ln === lastLine;
      let cls = "cm-tesela-code-line";
      if (isFirst) cls += " cm-tesela-code-line-first";
      if (isLast) cls += " cm-tesela-code-line-last";
      // The opening ``` line is always a fence; the closing one only when
      // the fence is actually closed (an unclosed fence has no end line).
      if (isFirst || (isLast && region.closed)) cls += " cm-tesela-code-fence-line";
      decos.push({ from: line.from, to: line.from, decoration: codeLineDeco(cls) });
    }
  }

  // Tags: position-aware classification per the tag-system spec.
  //   - Tokens inside the trailing cluster (one or more `#tag` tokens at
  //     the end of the doc, separated only by whitespace) render as chip
  //     widgets (atomic, clickable).
  //   - All other `#tag` tokens render inline as styled marks (not atomic;
  //     the cursor can edit them normally).
  const trailingStart = findTrailingClusterStart(doc);
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    const from = m.index;
    const to = m.index + m[0].length;
    if (insideCode(from)) continue;
    if (from >= trailingStart) {
      const name = m[1];
      decos.push({
        from,
        to,
        decoration: Decoration.replace({ widget: new TagChipWidget(name) }),
        atomic: true,
      });
    } else {
      decos.push({ from, to, decoration: tagInlineMark });
    }
  }

  // tags:: property lines: hide the whole line (canonical display is the pill UI)
  TAGS_LINE_RE.lastIndex = 0;
  while ((m = TAGS_LINE_RE.exec(doc)) !== null) {
    if (insideCode(m.index)) continue;
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
    if (insideCode(m.index)) continue;
    decos.push({ from: m.index, to: m.index + 2, decoration: wikiLinkBracketMark });
    decos.push({ from: m.index + 2, to: m.index + m[0].length - 2, decoration: wikiLinkMark });
    decos.push({ from: m.index + m[0].length - 2, to: m.index + m[0].length, decoration: wikiLinkBracketMark });
  }

  // Properties: emit key/value styling, plus a hide-class line decoration if
  // configured to hide via the facet (either always-hide or empty-hide+empty).
  PROPERTY_RE.lastIndex = 0;
  while ((m = PROPERTY_RE.exec(doc)) !== null) {
    if (insideCode(m.index)) continue;
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
//   - trailing-cluster `#tag` tokens — rendered as chip widgets
//   - block-id comments (BID_RE) — fully hidden via bidHide widget
//   - whole `tags:: ...` lines — hidden via `display:none` line decoration
//   - hidden `key:: value` property lines per the hiddenPropertyKeysFacet
//
// NOTE: inline `#tag` tokens (those NOT in the trailing cluster) are
// intentionally NOT atomic — they render as styled marks that the user
// can edit normally. Only the trailing-cluster chip widgets are atomic.

/**
 * Pure: returns sorted, non-overlapping `[from, to)` byte ranges in `doc`
 * that the cursor should skip over. Exported so the unit tests can exercise
 * the snap logic without a live editor.
 */
export function findAtomicCursorRanges(doc: string, config: HiddenKeysConfig): Array<[number, number]> {
  const ranges: Array<[number, number]> = [];

  // Fenced code regions: `#tag` / `tags::` tokens inside one are literal
  // code, not block markup — they get no chip widget, so they must not
  // be treated as atomic cursor ranges either.
  const codeRanges = findCodeFenceRanges(doc);
  const insideCode = (i: number): boolean =>
    codeRanges.some((r) => i >= r.from && i < r.to);

  // Only `#tag` tokens in the trailing cluster are atomic (they're chip
  // widgets that can't be entered with the cursor). Inline tokens stay
  // edit-friendly.
  const trailingStart = findTrailingClusterStart(doc);
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    if (m.index >= trailingStart && !insideCode(m.index)) {
      ranges.push([m.index, m.index + m[0].length]);
    }
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
    if (!insideCode(m.index)) {
      ranges.push(lineRangeWithNewline(m.index, m[0].length));
    }
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
  ".cm-tesela-tag": {
    // Inline `#tag` token — styled but editable. Read like a wiki-link, but
    // dimmer + monospace-feeling so it stays in the flow of prose.
    color: "var(--primary)",
    fontSize: "0.95em",
    opacity: "0.85",
  },
  ".cm-tesela-tag-chip": {
    // Trailing-cluster chip — atomic widget, click opens the tag page.
    display: "inline-block",
    background: "color-mix(in srgb, var(--primary) 12%, transparent)",
    color: "var(--primary)",
    border: "1px solid color-mix(in srgb, var(--primary) 35%, transparent)",
    borderRadius: "10px",
    padding: "0 7px",
    margin: "0 4px 0 0",
    fontFamily: "var(--theme-font-mono, var(--v4-mono))",
    fontSize: "0.78em",
    lineHeight: "1.5",
    cursor: "pointer",
    textTransform: "lowercase",
    verticalAlign: "0.05em",
  },
  ".cm-tesela-tag-chip:hover": {
    background: "color-mix(in srgb, var(--primary) 20%, transparent)",
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
  ".cm-tesela-code-line": {
    // Fenced ``` code lines — a raised monospace surface. Consecutive
    // lines' backgrounds abut, so a multi-line fence reads as one panel.
    background: "var(--surface-2)",
    fontFamily: "var(--theme-font-mono, var(--v4-mono))",
    fontSize: "0.86em",
    padding: "0 10px",
  },
  ".cm-tesela-code-line-first": {
    paddingTop: "6px",
    borderTopLeftRadius: "6px",
    borderTopRightRadius: "6px",
  },
  ".cm-tesela-code-line-last": {
    paddingBottom: "6px",
    borderBottomLeftRadius: "6px",
    borderBottomRightRadius: "6px",
  },
  ".cm-tesela-code-fence-line": {
    // The ``` delimiter lines stay visible (the block is editable) but
    // recede so the code body reads as the content.
    color: "var(--muted-foreground)",
  },
});
