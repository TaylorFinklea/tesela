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
import { EditorSelection, EditorState, Facet, RangeSet, RangeSetBuilder, StateEffect, StateField, Transaction } from "@codemirror/state";
// `.ts` extension so the node test runner resolves it (the repo convention
// for relative imports; `rewriteRelativeImportExtensions` handles the build).
import { tokenizeCode } from "./code-highlight.ts";
import { detectTokens, resolveDetectSpec, type DetectConfig } from "./task-tokens.ts";
import { getBlockTags } from "./block-tags.ts";
import { apiBase } from "./runtime-base.ts";

/** Resolve a markdown image source when the unfocused editor renders it.
 *  Relative sources are kept relative in the document and served from the
 *  mosaic's attachments route; absolute URLs are left untouched. */
export function resolveImageUrl(src: string, base = apiBase()): string {
  const trimmed = src.trim();
  if (/^[a-z][a-z0-9+.-]*:/i.test(trimmed) || trimmed.startsWith("//")) {
    return trimmed;
  }

  const parts = trimmed.replaceAll("\\", "/").split("/");
  const attachmentIndex = parts.findIndex((part) => part.toLowerCase() === "attachments");
  const relativeParts = attachmentIndex >= 0 ? parts.slice(attachmentIndex + 1) : parts;
  const normalizedParts: string[] = [];
  for (const part of relativeParts) {
    if (!part || part === ".") continue;
    if (part === "..") {
      normalizedParts.pop();
    } else {
      normalizedParts.push(part);
    }
  }

  return `${base.replace(/\/+$/, "")}/attachments/${normalizedParts.join("/")}`;
}

/** True for portable relative references to a PDF in the attachments route. */
export function isPdfAttachmentRef(src: string): boolean {
  const trimmed = src.trim();
  if (!trimmed || /^[a-z][a-z0-9+.-]*:/i.test(trimmed) || trimmed.startsWith("//")) {
    return false;
  }

  const path = trimmed.split(/[?#]/, 1)[0] ?? "";
  const parts = path.replaceAll("\\", "/").split("/");
  const attachmentIndex = parts.findIndex((part) => part.toLowerCase() === "attachments");
  const filename = parts[parts.length - 1] ?? "";
  return attachmentIndex >= 0 && parts.length > attachmentIndex + 1 && filename.toLowerCase().endsWith(".pdf");
}

class EmptyWidget extends WidgetType {
  toDOM() { return document.createElement("span"); }
  eq() { return true; }
}

// (Removed the trailing-cluster `TagChipWidget` — Model A, 2026-06-07: every
//  prose `#tag` renders inline; committed tags are the right-edge colored pills.
//  The position-based chip was dormant anyway — the trailing `<!-- bid -->`
//  marker keeps a tag off the true trailing edge.)

/** Inline image rendered from `![alt](url)` (markdown render, unfocused). */
class ImageWidget extends WidgetType {
  readonly src: string;
  readonly alt: string;
  constructor(src: string, alt: string) {
    super();
    this.src = src;
    this.alt = alt;
  }
  toDOM() {
    const img = document.createElement("img");
    img.src = resolveImageUrl(this.src);
    img.alt = this.alt;
    img.className = "cm-tesela-md-image";
    img.loading = "lazy";
    return img;
  }
  eq(other: ImageWidget) {
    return other.src === this.src && other.alt === this.alt;
  }
  ignoreEvent() {
    return true;
  }
}

/** Inline PDF link rendered as a route-backed chip (markdown render, unfocused). */
class PdfWidget extends WidgetType {
  readonly src: string;
  readonly label: string;
  constructor(src: string, label: string) {
    super();
    this.src = src;
    this.label = label;
  }
  toDOM() {
    const link = document.createElement("a");
    const url = resolveImageUrl(this.src);
    const label = this.label.trim() || "PDF";
    link.className = "cm-tesela-md-pdf";
    link.href = url;
    link.target = "_blank";
    link.rel = "noopener noreferrer";
    link.textContent = `📄 ${label}`;
    link.setAttribute("aria-label", `Open PDF ${label}`);
    link.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      window.open(url, "_blank", "noopener,noreferrer");
    });
    return link;
  }
  eq(other: PdfWidget) {
    return other.src === this.src && other.label === this.label;
  }
  ignoreEvent() {
    return true;
  }
}

/** Thematic break rendered from a `---` / `***` / `___` line. */
class HrWidget extends WidgetType {
  toDOM() {
    const el = document.createElement("hr");
    el.className = "cm-tesela-md-hr";
    return el;
  }
  eq() {
    return true;
  }
}

// ── GFM pipe-table widget ──────────────────────────────────────────────────
//
// IMPORTANT: CodeMirror 6 FORBIDS multi-line (line-break-spanning)
// `Decoration.replace(...)` from a ViewPlugin's `decorations` facet —
// they MUST come from a `StateField`. This widget is only ever emitted
// from `teselaTableDecorations` (a StateField below), NEVER from the
// `teselaDecorations` ViewPlugin. Violating this rule causes a runtime
// throw whenever the editor renders an unfocused block with a table.
// See: https://codemirror.net/docs/ref/#view.Decoration^replace

/** GFM pipe table rendered as an HTML <table> widget (unfocused blocks).
 *  The underlying raw markdown is left in the doc; the widget disappears
 *  when the block gains focus so the user can edit the source. */
class TableWidget extends WidgetType {
  readonly header: string[];
  readonly body: string[][];
  readonly align: Array<"left" | "center" | "right" | null>;
  constructor(
    header: string[],
    body: string[][],
    align: Array<"left" | "center" | "right" | null>,
  ) {
    super();
    this.header = header;
    this.body = body;
    this.align = align;
  }
  eq(other: TableWidget) {
    if (other.header.length !== this.header.length) return false;
    if (other.body.length !== this.body.length) return false;
    for (let i = 0; i < this.header.length; i++) {
      if (other.header[i] !== this.header[i]) return false;
    }
    for (let i = 0; i < this.body.length; i++) {
      const r1 = this.body[i];
      const r2 = other.body[i];
      if (!r1 || !r2 || r1.length !== r2.length) return false;
      for (let j = 0; j < r1.length; j++) {
        if (r1[j] !== r2[j]) return false;
      }
    }
    if (other.align.length !== this.align.length) return false;
    for (let i = 0; i < this.align.length; i++) {
      if (other.align[i] !== this.align[i]) return false;
    }
    return true;
  }
  toDOM() {
    const table = document.createElement("table");
    table.className = "cm-tesela-md-table";
    const thead = document.createElement("thead");
    const headerRow = document.createElement("tr");
    for (let i = 0; i < this.header.length; i++) {
      const th = document.createElement("th");
      // textContent (never innerHTML) — cell text is plain markdown, not HTML.
      th.textContent = this.header[i] ?? "";
      const a = this.align[i];
      if (a) th.style.textAlign = a;
      headerRow.appendChild(th);
    }
    thead.appendChild(headerRow);
    table.appendChild(thead);
    if (this.body.length > 0) {
      const tbody = document.createElement("tbody");
      for (const row of this.body) {
        const tr = document.createElement("tr");
        for (let i = 0; i < row.length; i++) {
          const td = document.createElement("td");
          td.textContent = row[i] ?? "";
          const a = this.align[i];
          if (a) td.style.textAlign = a;
          tr.appendChild(td);
        }
        tbody.appendChild(tr);
      }
      table.appendChild(tbody);
    }
    return table;
  }
  ignoreEvent() {
    return true;
  }
}

// Phase 9.4's inline KindBadgeWidget (the all-caps red TASK / URGENT chip
// prepended to block-line 0) was removed in 9.7 — the right-side tag pill
// is the canonical kind indicator now, freeing the left edge of the editor
// for typing. The `primaryTagFacet` below is kept defined in case another
// surface (e.g. read-only block reference card) wants the kind inline.

const tagInlineMark = Decoration.mark({ class: "cm-tesela-tag" });
// Model B: inline priority tokens (p1..p4), colored per level to match the
// BlockDateRow flag. They stay non-atomic / freely editable — they lift out on
// commit (Enter/blur), not while typing.
const priorityInlineMarks: Record<number, Decoration> = {
  1: Decoration.mark({ class: "cm-tesela-priority cm-tesela-priority-1" }),
  2: Decoration.mark({ class: "cm-tesela-priority cm-tesela-priority-2" }),
  3: Decoration.mark({ class: "cm-tesela-priority cm-tesela-priority-3" }),
  4: Decoration.mark({ class: "cm-tesela-priority cm-tesela-priority-4" }),
};
const dateInlineMark = Decoration.mark({ class: "cm-tesela-date" });
const numberInlineMark = Decoration.mark({ class: "cm-tesela-number" });
const selectInlineMark = Decoration.mark({ class: "cm-tesela-select" });
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

// Syntax-highlight token marks (`hljs-*`, themed in CSS), cached by kind.
const hljsMarkCache = new Map<string, Decoration>();
function hljsMark(kind: string): Decoration {
  let deco = hljsMarkCache.get(kind);
  if (!deco) {
    deco = Decoration.mark({ class: `hljs-${kind}` });
    hljsMarkCache.set(kind, deco);
  }
  return deco;
}

// Floating "copy" button for a fenced code block. Copies the code between the
// fences. Positioned (CSS) at the top-right of the code surface.
class CodeCopyWidget extends WidgetType {
  // Explicit field (not a TS constructor parameter property) so the node
  // unit-test runner's strip-only TS loader can parse this module.
  readonly code: string;
  constructor(code: string) {
    super();
    this.code = code;
  }
  eq(other: CodeCopyWidget) {
    return other.code === this.code;
  }
  toDOM() {
    const btn = document.createElement("button");
    btn.className = "cm-tesela-code-copy";
    btn.type = "button";
    btn.textContent = "copy";
    btn.title = "Copy code";
    btn.addEventListener("mousedown", (e) => {
      e.preventDefault();
      e.stopPropagation();
      try {
        void navigator.clipboard?.writeText(this.code).catch(() => {});
      } catch {
        /* clipboard unavailable */
      }
      btn.textContent = "copied";
      window.setTimeout(() => {
        btn.textContent = "copy";
      }, 1200);
    });
    return btn;
  }
  ignoreEvent() {
    return true;
  }
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

// ── Markdown inline/line formatting (rendered when the block is NOT focused;
// raw when focused, for editing). Hand-rolled regexes, consistent with the
// tag/wiki/property matchers above. Bold is matched before italic so a `**`
// pair isn't mis-read as two `*` italics. `\n` is excluded so a span can't
// straddle lines within a multi-line block.
const MD_BOLD_RE = /\*\*([^*\n]+?)\*\*|__([^_\n]+?)__/g;
const MD_ITALIC_RE = /\*([^*\n]+?)\*/g;
const MD_CODE_RE = /`([^`\n]+?)`/g;
const MD_STRIKE_RE = /~~([^~\n]+?)~~/g;
const MD_HEADING_RE = /^(#{1,6})([ \t]+)(.*)$/gm;
// `![alt](url)` images — matched BEFORE links (the `[alt](url)` tail would
// otherwise be read as a plain link).
const MD_IMAGE_RE = /!\[([^\]\n]*)\]\(([^)\n]+)\)/g;
// `[text](url)` — the `[[wiki]]` form never matches (it has no `](`), so the
// two link syntaxes don't collide.
const MD_LINK_RE = /\[([^\]\n]+)\]\(([^)\n]+)\)/g;
// `> quote` (one optional space after `>`).
const MD_QUOTE_RE = /^(>[ \t]?)(.*)$/gm;
// `==highlight==`.
const MD_HIGHLIGHT_RE = /==([^=\n]+?)==/g;
// A line of 3+ `-`/`*`/`_` (optionally space-separated) → a thematic break.
const MD_HR_RE = /^[ \t]*([-*_])(?:[ \t]*\1){2,}[ \t]*$/gm;

const mdBoldMark = Decoration.mark({ class: "cm-tesela-md-bold" });
const mdItalicMark = Decoration.mark({ class: "cm-tesela-md-italic" });
const mdCodeMark = Decoration.mark({ class: "cm-tesela-md-code" });
const mdStrikeMark = Decoration.mark({ class: "cm-tesela-md-strike" });
const mdHighlightMark = Decoration.mark({ class: "cm-tesela-md-highlight" });
const mdHrReplace = Decoration.replace({ widget: new HrWidget() });
const mdLinkMark = Decoration.mark({ class: "cm-tesela-md-link" });
const mdQuoteLineDeco = Decoration.line({ attributes: { class: "cm-tesela-md-quote" } });

// ── Callouts (`[!type] title`, optionally `> `-prefixed) ────────────────────
// Obsidian-style admonitions. The whole block renders as a typed box. Type
// aliases collapse onto a small canonical set with an icon + color (CSS).
const CALLOUT_ICON: Record<string, string> = {
  info: "ℹ", warning: "⚠", error: "⛔", note: "✎", tip: "💡", success: "✓", question: "?",
};
const CALLOUT_ALIAS: Record<string, string> = {
  warn: "warning", caution: "warning", attention: "warning",
  danger: "error", fail: "error", failure: "error", bug: "error", missing: "error",
  hint: "tip", important: "tip",
  check: "success", done: "success", todo: "success",
  abstract: "info", summary: "info", tldr: "info",
  faq: "question", help: "question",
  quote: "note", cite: "note", example: "note", abstract2: "note",
};
const CALLOUT_RE = /^([ \t]*(?:>[ \t]?)?)\[!([A-Za-z]+)\]([ \t]?)(.*)$/;
function calloutType(raw: string): string {
  const t = raw.toLowerCase();
  const norm = CALLOUT_ALIAS[t] ?? t;
  return CALLOUT_ICON[norm] ? norm : "note";
}
const calloutLineCache = new Map<string, Decoration>();
function calloutLineDeco(type: string, first: boolean, last: boolean): Decoration {
  const cls =
    `cm-tesela-callout cm-tesela-callout-${type}` +
    (first ? " cm-tesela-callout-first" : "") +
    (last ? " cm-tesela-callout-last" : "");
  let deco = calloutLineCache.get(cls);
  if (!deco) {
    deco = Decoration.line({ attributes: { class: cls } });
    calloutLineCache.set(cls, deco);
  }
  return deco;
}
class CalloutIconWidget extends WidgetType {
  // Explicit field (not a TS constructor parameter property) so the node
  // unit-test runner's strip-only TS loader can parse this module.
  readonly type: string;
  constructor(type: string) {
    super();
    this.type = type;
  }
  eq(other: CalloutIconWidget) {
    return other.type === this.type;
  }
  toDOM() {
    const s = document.createElement("span");
    s.className = `cm-tesela-callout-icon cm-tesela-callout-icon-${this.type}`;
    s.textContent = CALLOUT_ICON[this.type] ?? "•";
    s.setAttribute("aria-hidden", "true");
    return s;
  }
  ignoreEvent() {
    return true;
  }
}
// Hides a ``` fence delimiter line entirely (display:none) so a fenced block
// reads as a clean code surface when the block isn't being edited.
const mdCodeFenceHideLine = Decoration.line({ attributes: { class: "cm-tesela-md-code-fence-hidden" } });
// Zero-width replace that removes a marker (`**`, `` ` ``, `### `) from the
// rendered (unfocused) view entirely.
const mdMarkerHide = Decoration.replace({});
// Heading line decorations (size/weight by level), cached by class.
const headingLineCache = new Map<number, Decoration>();
function headingLineDeco(level: number): Decoration {
  let deco = headingLineCache.get(level);
  if (!deco) {
    deco = Decoration.line({ attributes: { class: `cm-tesela-md-heading cm-tesela-md-h${level}` } });
    headingLineCache.set(level, deco);
  }
  return deco;
}

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

/**
 * Model B — lowercased tag names whose blocks get inline NLP detection
 * (priority/date highlight + lift). The block is detection-enabled when its
 * DIRECT tags (its own `tags::` + inline `#tags`, never inherited) intersect
 * this set. Default seeded on for `task`; the parent recomputes it from the
 * tag pages' `detect_tokens` flag.
 */
const EMPTY_DETECT_CONFIG: DetectConfig = new Map();
export const detectConfigFacet = Facet.define<DetectConfig, DetectConfig>({
  combine: (values) => values[0] ?? EMPTY_DETECT_CONFIG,
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

// ── GFM pipe-table detection (pure helpers, exported for unit tests) ──────────

export type PipeTable = {
  from: number;
  to: number;
  header: string[];
  body: string[][];
  align: Array<"left" | "center" | "right" | null>;
};

/** Pure: split a single pipe-table row into trimmed cells. A leading or
 *  trailing `|` is a separator (dropped), not part of any cell. Used both
 *  to detect table regions and to render the widget's cell text. */
export function splitPipeCells(line: string): string[] {
  const trimmed = line.trim();
  const parts = trimmed.split("|");
  if (trimmed.startsWith("|") && parts.length > 0) parts.shift();
  if (trimmed.endsWith("|") && parts.length > 0) parts.pop();
  return parts.map((c) => c.trim());
}

function parsePipeAlign(cell: string): "left" | "center" | "right" | null {
  const c = cell.trim();
  const startsCol = c.startsWith(":");
  const endsCol = c.endsWith(":");
  if (startsCol && endsCol) return "center";
  if (endsCol) return "right";
  if (startsCol) return "left";
  return null;
}

/** Pure: find GFM pipe-table regions in `doc`. A region is a header line, a
 *  `---|---` (or `:--`/`--:`/`:-:`) separator line, and zero or more body
 *  rows. The separator determines the column count; header and body rows must
 *  match it. Blank lines, lines without `|`, or rows with the wrong column
 *  count end the table. The `to` offset is the start of the line AFTER the
 *  table (or `doc.length` if the table is the doc's last line) — same
 *  convention as `findCodeFenceRanges`.
 *
 *  Used both to render the table as an HTML widget (via StateField, unfocused)
 *  and to suppress tag / wiki-link / property parsing inside cells. */
export function findPipeTables(doc: string): PipeTable[] {
  const tables: PipeTable[] = [];
  const lines = doc.split("\n");
  const lineStarts: number[] = [];
  let off = 0;
  for (const line of lines) {
    lineStarts.push(off);
    off += line.length + 1; // +1 for the consumed "\n"
  }
  const isRow = (line: string): boolean => line.trim().length > 0 && line.includes("|");
  const isSep = (line: string): boolean =>
    /^\s*\|?\s*:?-+:?\s*(\|\s*:?-+:?\s*)+\|?\s*$/.test(line);

  let i = 0;
  while (i < lines.length - 1) {
    const headerLine = lines[i];
    const sepLine = lines[i + 1];
    if (headerLine !== undefined && sepLine !== undefined && isRow(headerLine) && isSep(sepLine)) {
      const headerCells = splitPipeCells(headerLine);
      const sepCells = splitPipeCells(sepLine);
      const cols = sepCells.length;
      if (headerCells.length === cols) {
        let j = i + 2;
        while (
          j < lines.length &&
          isRow(lines[j] ?? "") &&
          splitPipeCells(lines[j] ?? "").length === cols
        ) {
          j++;
        }
        const from = lineStarts[i] ?? 0;
        const to = j < lines.length ? (lineStarts[j] ?? doc.length) : doc.length;
        const body: string[][] = [];
        for (let k = i + 2; k < j; k++) body.push(splitPipeCells(lines[k] ?? ""));
        const align = sepCells.map(parsePipeAlign);
        tables.push({ from, to, header: headerCells, body, align });
        i = j;
        continue;
      }
    }
    i++;
  }
  return tables;
}

// ── StateField for table block decorations ──────────────────────────────────────
//
// WHY A StateField (not the ViewPlugin):
//
// CodeMirror 6 forbids multi-line (line-break-spanning) Decoration.replace
// decorations from a ViewPlugin's `decorations` facet. They MUST be provided
// via a StateField. The `teselaDecorations` ViewPlugin handles all single-line
// and non-replace-across-newlines decorations fine; GFM pipe tables span
// multiple lines so they MUST live here. Violating the rule causes a runtime
// throw even though the code type-checks. See CM6 docs:
// https://codemirror.net/docs/ref/#view.Decoration^replace

// ── StateEffect + focused StateField for table decoration gating ────────────
//
// StateField.update only receives EditorState, not EditorView, so it cannot
// call view.hasFocus directly. Instead a companion ViewPlugin fires a
// StateEffect when focus changes; the focusedStateField records it; and
// teselaTableDecorations reads it to decide whether to emit widgets.

/** Dispatched by tableFocusTracker when focus changes. */
const setFocusedEffect = StateEffect.define<boolean>();

/** Tracks editor focus as pure state so StateFields can read it. */
export const focusedStateField = StateField.define<boolean>({
  create: () => false,
  update(value, tr) {
    for (const e of tr.effects) {
      if (e.is(setFocusedEffect)) return e.value;
    }
    return value;
  },
});

function buildTableDecorations(state: EditorState): DecorationSet {
  // Only render widgets when the block is NOT focused (same gate as the
  // ViewPlugin's !view.hasFocus block for all other markdown decorations).
  const focused = state.field(focusedStateField);
  if (focused) return Decoration.none;

  const doc = state.doc.toString();
  const tables = findPipeTables(doc);
  if (tables.length === 0) return Decoration.none;

  const builder = new RangeSetBuilder<Decoration>();
  for (const t of tables) {
    const from = Math.max(0, t.from);
    const to = Math.min(doc.length, t.to);
    if (to <= from) continue;
    builder.add(
      from,
      to,
      Decoration.replace({ widget: new TableWidget(t.header, t.body, t.align) }),
    );
  }
  return builder.finish();
}

/**
 * StateField<DecorationSet> that emits block-spanning Decoration.replace for
 * GFM pipe tables in unfocused blocks.
 *
 * MUST be a StateField (not the ViewPlugin's `decorations` facet) because
 * CodeMirror 6 forbids multi-line (line-break-spanning) replace decorations
 * from ViewPlugin decorations. Violating this rule type-checks fine but
 * throws at runtime. See CM6 docs on Decoration.replace.
 */
export const teselaTableDecorations = StateField.define<DecorationSet>({
  create(state) {
    return buildTableDecorations(state);
  },
  update(deco, tr) {
    if (tr.docChanged) return buildTableDecorations(tr.state);
    // Check for a focus-change effect.
    for (const e of tr.effects) {
      if (e.is(setFocusedEffect)) return buildTableDecorations(tr.state);
    }
    return deco.map(tr.changes);
  },
  provide(field) {
    return EditorView.decorations.from(field);
  },
});

/**
 * ViewPlugin that dispatches setFocusedEffect on focusChanged so that
 * teselaTableDecorations (a StateField) can react and show/hide widgets.
 * Must be included alongside teselaTableDecorations in the extensions array.
 */
export const tableFocusTracker = ViewPlugin.fromClass(
  class {
    update(update: ViewUpdate) {
      if (update.focusChanged) {
        // Schedule the dispatch after the current update cycle to avoid
        // "dispatch from within an update" errors.
        const view = update.view;
        Promise.resolve().then(() => {
          view.dispatch({ effects: setFocusedEffect.of(view.hasFocus) });
        });
      }
    }
  },
);

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
  // GFM pipe tables: detect early so the tag/wikilink/property passes can
  // skip cell content. The actual block-replacing widget lives in the
  // teselaTableDecorations StateField (see below), NOT here, because
  // CodeMirror 6 forbids multi-line replace decorations from a ViewPlugin.
  const tables = findPipeTables(doc);
  const insideTable = (i: number): boolean =>
    tables.some((t) => i >= t.from && i < t.to);
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
    // Syntax-highlight the content (always-on, so it's highlighted while
    // editing too) + a floating copy button. Lang = the opening fence's
    // trailing word (```bash → "bash"). Content = the lines between fences.
    const lang = view.state.doc.line(firstLine).text.replace(/^\s*`+/, "").trim();
    const contentLast = region.closed ? lastLine - 1 : lastLine;
    if (firstLine + 1 <= contentLast) {
      const contentStart = view.state.doc.line(firstLine + 1).from;
      const contentEnd = view.state.doc.line(contentLast).to;
      if (contentEnd > contentStart) {
        const codeText = view.state.doc.sliceString(contentStart, contentEnd);
        for (const tk of tokenizeCode(codeText, lang)) {
          decos.push({ from: contentStart + tk.start, to: contentStart + tk.end, decoration: hljsMark(tk.kind) });
        }
        decos.push({ from: contentStart, to: contentStart, decoration: Decoration.widget({ widget: new CodeCopyWidget(codeText), side: -1 }) });
      }
    }
  }

  // Tags (Model A, 2026-06-07): every `#tag` in the prose renders inline as a
  // styled, editable mark. Committed tags live on the `tags::` line and surface
  // as a right-edge colored pill (BlockOutliner) — there is no in-editor chip
  // widget. (`tagsLineHide` below keeps the `tags::` line out of the prose.)
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(doc)) !== null) {
    if (insideCode(m.index) || insideTable(m.index)) continue;
    decos.push({ from: m.index, to: m.index + m[0].length, decoration: tagInlineMark });
  }

  // Model B detect-inline: priority / date / number / select tokens on the
  // prose line — highlighted live, but ONLY when this block has a
  // detection-enabled DIRECT tag (config-driven, default #Task). getBlockTags
  // reads the block's own tags:: + inline #tags (never inherited), so an
  // inheriting child never highlights. Tokens lift out on blur, not here.
  const detectConfig = view.state.facet(detectConfigFacet);
  const detectSpec = detectConfig.size > 0 ? resolveDetectSpec(getBlockTags(doc), detectConfig) : null;
  if (detectSpec) {
    for (const tok of detectTokens(doc, detectSpec)) {
      if (insideCode(tok.from)) continue;
      const deco =
        tok.kind === "priority" ? priorityInlineMarks[tok.level!]
        : tok.kind === "number" ? numberInlineMark
        : tok.kind === "date" ? dateInlineMark
        : selectInlineMark;
      decos.push({ from: tok.from, to: tok.to, decoration: deco });
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
    if (insideCode(m.index) || insideTable(m.index)) continue;
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

  // ── Markdown formatting — RENDERED only when this block is NOT focused.
  // When the editor has focus the user is editing it, so we show pure raw
  // markdown (skip all of this — no atomic ranges to fight with, because an
  // unfocused editor has no cursor). Markers are hidden in render mode; the
  // content carries the style class. Suppressed inside fenced ``` code.
  if (!view.hasFocus) {
    const hideMarker = (from: number, to: number) => {
      if (to > from) decos.push({ from, to, decoration: mdMarkerHide });
    };

    // Images `![alt](url)` — replace with an inline <img>. The widget keeps
    // the raw markdown source and resolves it only while rendering, so a
    // relative attachment path remains portable in saved content. Matched
    // FIRST + recorded so the `[alt](url)` tail isn't also read as a plain
    // link, and so inline markup inside doesn't fire.
    const imageRanges: Array<[number, number]> = [];
    MD_IMAGE_RE.lastIndex = 0;
    while ((m = MD_IMAGE_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      if (insideCode(from)) {
        MD_IMAGE_RE.lastIndex = from + 1;
        continue;
      }
      const url = m[2].trim();
      imageRanges.push([from, to]);
      decos.push({ from, to, decoration: Decoration.replace({ widget: new ImageWidget(url, m[1]) }) });
    }

    // Relative PDF links become route-backed chips. The original markdown
    // remains in the document and reappears when the block is focused.
    const pdfRanges: Array<[number, number]> = [];
    MD_LINK_RE.lastIndex = 0;
    while ((m = MD_LINK_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      if (insideCode(from) || !isPdfAttachmentRef(m[2])) {
        MD_LINK_RE.lastIndex = from + 1;
        continue;
      }
      pdfRanges.push([from, to]);
      decos.push({ from, to, decoration: Decoration.replace({ widget: new PdfWidget(m[2].trim(), m[1]) }) });
    }

    // Inline `code` spans next — their content is literal, so other inline
    // markup inside them is left untouched.
    const codeSpanRanges: Array<[number, number]> = [];
    MD_CODE_RE.lastIndex = 0;
    while ((m = MD_CODE_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      if (insideCode(from) || imageRanges.some(([a, b]) => from >= a && from < b)) {
        MD_CODE_RE.lastIndex = from + 1;
        continue;
      }
      codeSpanRanges.push([from, to]);
      hideMarker(from, from + 1);
      hideMarker(to - 1, to);
      if (to - 1 > from + 1) decos.push({ from: from + 1, to: to - 1, decoration: mdCodeMark });
    }
    const literal = (i: number): boolean =>
      insideCode(i) ||
      insideTable(i) ||
      codeSpanRanges.some(([a, b]) => i >= a && i < b) ||
      imageRanges.some(([a, b]) => i >= a && i < b) ||
      pdfRanges.some(([a, b]) => i >= a && i < b);

    // Bold (`**` / `__`). Record ranges so italic can't re-match the inner `*`.
    const boldRanges: Array<[number, number]> = [];
    MD_BOLD_RE.lastIndex = 0;
    while ((m = MD_BOLD_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      if (literal(from)) {
        MD_BOLD_RE.lastIndex = from + 1;
        continue;
      }
      boldRanges.push([from, to]);
      hideMarker(from, from + 2);
      hideMarker(to - 2, to);
      if (to - 2 > from + 2) decos.push({ from: from + 2, to: to - 2, decoration: mdBoldMark });
    }
    const insideBold = (i: number): boolean => boldRanges.some(([a, b]) => i >= a && i < b);

    // Italic (`*…*`) — skip code spans + the inner text of bold runs.
    MD_ITALIC_RE.lastIndex = 0;
    while ((m = MD_ITALIC_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      // Rewind to just past the start on a skip: a spurious match across a
      // `**bold**` boundary must not consume the real italic's opening `*`.
      if (literal(from) || insideBold(from)) {
        MD_ITALIC_RE.lastIndex = from + 1;
        continue;
      }
      hideMarker(from, from + 1);
      hideMarker(to - 1, to);
      if (to - 1 > from + 1) decos.push({ from: from + 1, to: to - 1, decoration: mdItalicMark });
    }

    // Strikethrough (`~~…~~`).
    MD_STRIKE_RE.lastIndex = 0;
    while ((m = MD_STRIKE_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      if (literal(from)) {
        MD_STRIKE_RE.lastIndex = from + 1;
        continue;
      }
      hideMarker(from, from + 2);
      hideMarker(to - 2, to);
      if (to - 2 > from + 2) decos.push({ from: from + 2, to: to - 2, decoration: mdStrikeMark });
    }

    // Highlight (`==…==`).
    MD_HIGHLIGHT_RE.lastIndex = 0;
    while ((m = MD_HIGHLIGHT_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      if (literal(from)) {
        MD_HIGHLIGHT_RE.lastIndex = from + 1;
        continue;
      }
      hideMarker(from, from + 2);
      hideMarker(to - 2, to);
      if (to - 2 > from + 2) decos.push({ from: from + 2, to: to - 2, decoration: mdHighlightMark });
    }

    // ATX headings (`# …` through `###### …`) — line-level size/weight, the
    // `### ` prefix hidden.
    MD_HEADING_RE.lastIndex = 0;
    while ((m = MD_HEADING_RE.exec(doc)) !== null) {
      if (literal(m.index)) continue;
      const level = m[1].length;
      const markerEnd = m.index + m[1].length + m[2].length; // through `### `
      const line = view.state.doc.lineAt(m.index);
      decos.push({ from: line.from, to: line.from, decoration: headingLineDeco(level) });
      hideMarker(m.index, markerEnd);
    }

    // `[text](url)` links — style the text, hide the `[` and `](url)`. (Click
    // currently just focuses the block → raw; open-in-browser is a follow-up.)
    MD_LINK_RE.lastIndex = 0;
    while ((m = MD_LINK_RE.exec(doc)) !== null) {
      const from = m.index;
      const to = m.index + m[0].length;
      if (literal(from)) {
        MD_LINK_RE.lastIndex = from + 1;
        continue;
      }
      const textEnd = from + 1 + m[1].length;
      hideMarker(from, from + 1); // `[`
      hideMarker(textEnd, to); // `](url)`
      if (textEnd > from + 1) decos.push({ from: from + 1, to: textEnd, decoration: mdLinkMark });
    }

    // `> ` blockquotes — line styling + hide the marker. A `> [!type]` line is
    // a callout (handled below), not a plain quote.
    MD_QUOTE_RE.lastIndex = 0;
    while ((m = MD_QUOTE_RE.exec(doc)) !== null) {
      if (literal(m.index)) continue;
      if (/^\[!/.test(m[2] ?? "")) continue;
      const line = view.state.doc.lineAt(m.index);
      decos.push({ from: line.from, to: line.from, decoration: mdQuoteLineDeco });
      hideMarker(m.index, m.index + m[1].length);
    }

    // Callouts — a block whose FIRST line is `[!type] title` (optionally
    // `> `-prefixed) renders the WHOLE block as a typed box (info / warning /
    // error / note / tip / success / question). The `[!type]` marker is hidden
    // and replaced with the type icon; the title + body keep their text.
    {
      const l1 = view.state.doc.line(1);
      const cm = l1.text.match(CALLOUT_RE);
      if (cm && !literal(l1.from)) {
        const type = calloutType(cm[2]);
        const total = view.state.doc.lines;
        for (let ln = 1; ln <= total; ln++) {
          const line = view.state.doc.line(ln);
          decos.push({ from: line.from, to: line.from, decoration: calloutLineDeco(type, ln === 1, ln === total) });
        }
        const markerEnd = l1.from + cm[1].length + 2 + cm[2].length + 1; // `[!type]`
        decos.push({ from: l1.from, to: markerEnd, decoration: Decoration.replace({ widget: new CalloutIconWidget(type) }) });
      }
    }

    // Horizontal rules (`---` / `***` / `___` alone on a line) → an <hr>.
    MD_HR_RE.lastIndex = 0;
    while ((m = MD_HR_RE.exec(doc)) !== null) {
      if (literal(m.index)) continue;
      const line = view.state.doc.lineAt(m.index);
      if (line.to > line.from) {
        decos.push({ from: line.from, to: line.to, decoration: mdHrReplace });
      }
    }

    // Fenced ``` blocks: hide the delimiter lines so the block reads as a
    // clean code surface (the content lines keep their painted background from
    // the always-on pass above). Closed fences hide both ends; an unclosed one
    // hides only the opener.
    for (const region of codeRanges) {
      const openLine = view.state.doc.lineAt(region.from);
      decos.push({ from: openLine.from, to: openLine.from, decoration: mdCodeFenceHideLine });
      if (region.closed) {
        const closeLine = view.state.doc.lineAt(region.to);
        decos.push({ from: closeLine.from, to: closeLine.from, decoration: mdCodeFenceHideLine });
      }
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
      const detectTagsChanged =
        update.startState.facet(detectConfigFacet) !==
        update.state.facet(detectConfigFacet);
      if (
        update.docChanged ||
        update.viewportChanged ||
        update.focusChanged ||
        hiddenChanged ||
        primaryChanged ||
        detectTagsChanged
      ) {
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

  // Model A: no `#tag` is atomic — every prose tag is an editable inline mark.
  // (Bid comments below stay atomic so the cursor jumps past them.)
  let m: RegExpExecArray | null;

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
  ".cm-tesela-priority": {
    // Inline priority token (p1..p4) — colored per level to match the flag.
    fontWeight: "600",
    fontSize: "0.95em",
  },
  // Priority levels read theme role tokens (`--priority-p1`…`p3` in
  // app.css, mirroring `--type-*`) rather than hardcoded hex, so a theme
  // can deepen them for contrast (see `[data-theme="prism-light"]` in
  // themes.css) — the fallback keeps the original literal if the var is
  // ever missing. p4 has no token of its own: it's the "no signal" level,
  // so it reads the neutral `--muted-foreground` scale directly.
  ".cm-tesela-priority-1": { color: "var(--priority-p1, #EB5C58)" },
  ".cm-tesela-priority-2": { color: "var(--priority-p2, #E8A33D)" },
  ".cm-tesela-priority-3": { color: "var(--priority-p3, #6B9AE0)" },
  ".cm-tesela-priority-4": { color: "var(--muted-foreground, #8A909C)" },
  ".cm-tesela-date": {
    // Inline natural-language date token (scheduled/deadline) — cyan, lifts
    // to the below-strip on commit. Theme role token, same as priority.
    color: "var(--date-token, #62B8CE)",
    fontSize: "0.95em",
  },
  ".cm-tesela-number": {
    // Inline number-property token (e.g. "5 points") — violet.
    color: "#A98BE0",
    fontSize: "0.95em",
  },
  ".cm-tesela-select": {
    // Inline non-priority select token — muted accent.
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
    fontFamily: "var(--theme-font-mono)",
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
    fontFamily: "var(--theme-font-mono)",
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

  // ── Markdown formatting (rendered when the block is not focused) ──────────
  ".cm-tesela-md-bold": { fontWeight: "700" },
  ".cm-tesela-md-italic": { fontStyle: "italic" },
  ".cm-tesela-md-strike": {
    textDecoration: "line-through",
    textDecorationColor: "var(--muted-foreground)",
  },
  ".cm-tesela-md-code": {
    fontFamily: "var(--theme-font-mono)",
    fontSize: "0.88em",
    background: "var(--surface-2)",
    border: "1px solid color-mix(in srgb, var(--foreground) 8%, transparent)",
    borderRadius: "4px",
    padding: "0.05em 0.32em",
  },
  ".cm-tesela-md-heading": { fontWeight: "600", lineHeight: "1.3" },
  ".cm-tesela-md-h1": { fontSize: "1.6em" },
  ".cm-tesela-md-h2": { fontSize: "1.4em" },
  ".cm-tesela-md-h3": { fontSize: "1.2em" },
  ".cm-tesela-md-h4": { fontSize: "1.08em" },
  ".cm-tesela-md-h5": { fontSize: "1em", opacity: "0.92" },
  ".cm-tesela-md-h6": { fontSize: "0.92em", opacity: "0.82" },
  ".cm-tesela-md-link": {
    color: "var(--primary)",
    textDecoration: "underline",
    textDecorationColor: "color-mix(in srgb, var(--primary) 30%, transparent)",
    textUnderlineOffset: "3px",
    textDecorationThickness: "1px",
    cursor: "pointer",
  },
  ".cm-tesela-md-pdf": {
    display: "inline-flex",
    alignItems: "center",
    gap: "4px",
    color: "var(--primary)",
    background: "var(--surface-2)",
    border: "1px solid color-mix(in srgb, var(--primary) 24%, transparent)",
    borderRadius: "5px",
    padding: "1px 6px",
    textDecoration: "none",
    cursor: "pointer",
    verticalAlign: "middle",
  },
  ".cm-tesela-md-quote": {
    borderLeft: "3px solid color-mix(in srgb, var(--foreground) 18%, transparent)",
    paddingLeft: "10px",
    color: "var(--muted-foreground)",
    fontStyle: "italic",
  },
  ".cm-tesela-md-code-fence-hidden": {
    display: "none",
  },
  ".cm-tesela-md-highlight": {
    background: "color-mix(in srgb, #ffd54f 38%, transparent)",
    borderRadius: "3px",
    padding: "0.02em 0.18em",
  },
  ".cm-tesela-md-image": {
    maxWidth: "100%",
    maxHeight: "340px",
    borderRadius: "6px",
    margin: "4px 0",
    display: "block",
  },
  "hr.cm-tesela-md-hr": {
    border: "none",
    borderTop: "1px solid color-mix(in srgb, var(--foreground) 20%, transparent)",
    margin: "8px 0",
    width: "100%",
  },
  // GFM pipe table — rendered (as an HTML <table>) when the block is unfocused.
  // The Decoration.replace that emits this widget comes from the
  // teselaTableDecorations StateField, NOT the teselaDecorations ViewPlugin.
  ".cm-tesela-md-table": {
    borderCollapse: "collapse",
    margin: "6px 0",
    fontSize: "0.92em",
    lineHeight: "1.4",
    border: "1px solid color-mix(in srgb, var(--foreground) 15%, transparent)",
    borderRadius: "4px",
    overflow: "hidden",
  },
  ".cm-tesela-md-table th": {
    background: "var(--surface-2)",
    fontWeight: "600",
    textAlign: "left",
    padding: "4px 10px",
    borderBottom: "1px solid color-mix(in srgb, var(--foreground) 20%, transparent)",
    borderRight: "1px solid color-mix(in srgb, var(--foreground) 8%, transparent)",
  },
  ".cm-tesela-md-table td": {
    padding: "4px 10px",
    borderRight: "1px solid color-mix(in srgb, var(--foreground) 8%, transparent)",
    borderBottom: "1px solid color-mix(in srgb, var(--foreground) 8%, transparent)",
  },
  ".cm-tesela-md-table th:last-child, .cm-tesela-md-table td:last-child": {
    borderRight: "none",
  },
  ".cm-tesela-md-table tr:last-child td": {
    borderBottom: "none",
  },
});
