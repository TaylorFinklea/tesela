/**
 * Block-level parser for Tesela note bodies.
 * Port of crates/tesela-core/src/block.rs.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";

const TAG_RE = /#([A-Za-z0-9_/-]+)/g;
const WIKI_LINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;
const BID_COMMENT_RE = /<!--\s*bid:([0-9a-fA-F-]{32,36})\s*-->/g;
const PROPERTY_LINE_RE = /^([A-Za-z_][A-Za-z0-9_]*)::(?:[ \t]+(.*)|[ \t]*)$/;

type FenceMarker = {
  char: "`" | "~";
  width: number;
};

type CodeFenceSpan = {
  from: number;
  to: number;
  lang: string;
  value: string;
};

function fenceMarker(line: string): FenceMarker | null {
  let leading = 0;
  while (leading < line.length && line[leading] === " ") leading += 1;
  if (leading > 3 || leading >= line.length) return null;
  const char = line[leading];
  if (char !== "`" && char !== "~") return null;
  let width = 0;
  while (line[leading + width] === char) width += 1;
  return width >= 3 ? { char, width } : null;
}

function closesFence(line: string, marker: FenceMarker): boolean {
  let leading = 0;
  while (leading < line.length && line[leading] === " ") leading += 1;
  if (leading > 3 || leading >= line.length || line[leading] !== marker.char) return false;
  let width = 0;
  while (line[leading + width] === marker.char) width += 1;
  return width >= marker.width && line.slice(leading + width).trim() === "";
}

function continuationText(line: string, blockIndent: number, trimTrailing: boolean): string {
  const expected = (blockIndent + 1) * 2;
  const prefix = line.slice(0, expected);
  const content = prefix.length === expected && /^ *$/.test(prefix)
    ? line.slice(expected)
    : line.trimStart();
  return trimTrailing ? content.trimEnd() : content;
}

export function parseBlocks(noteId: string, body: string): ParsedBlock[] {
  const lines = body.split("\n");
  // Match Rust's `str::lines()`: a file-ending newline is a terminator, not
  // an extra blank payload line. A second newline remains significant.
  if (body.endsWith("\n")) lines.pop();

  type RawBlock = {
    lineNum: number;
    indent: number;
    text: string;
    fence: FenceMarker | null;
  };
  const raw: RawBlock[] = [];
  let current: RawBlock | null = null;
  let globalFence: FenceMarker | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Once a block owns a fence, its payload outranks blank/bullet/property
    // detection. Remove exactly the list-continuation prefix and preserve
    // every remaining byte, including blank lines and trailing spaces.
    if (current?.fence) {
      const content = continuationText(line, current.indent, false);
      current.text += "\n" + content;
      if (closesFence(content, current.fence)) current.fence = null;
      continue;
    }

    // Raw top-level fences before the first canonical bullet are not editor
    // blocks, but bullet-shaped payload inside them must stay inert.
    if (current === null) {
      if (globalFence !== null) {
        if (closesFence(line, globalFence)) globalFence = null;
        continue;
      }
      const opener = fenceMarker(line);
      if (opener !== null) {
        globalFence = opener;
        continue;
      }
    }

    const trimStart = line.trimStart();
    if (trimStart === "") continue;
    const spaces = line.length - trimStart.length;
    const indent = Math.floor(spaces / 2);
    // Bullet starts a block if the line begins with "- " (with content) OR
    // equals "-" / "- " exactly (empty-content block, used when tags/properties
    // live on continuation lines).
    const trimmedEnd = trimStart.trimEnd();
    const isBullet = trimStart.startsWith("- ") || trimmedEnd === "-";
    if (isBullet) {
      if (current) raw.push(current);
      const text = trimStart.startsWith("- ")
        ? trimStart.slice(2).trimEnd()
        : "";
      const visible = stripStructuralBid(text).text;
      current = { lineNum: i, indent, text, fence: fenceMarker(visible) };
    } else if (current) {
      const continuation = continuationText(line, current.indent, true);
      current.text += "\n" + continuation;
      current.fence = fenceMarker(continuation);
    }
  }
  if (current) raw.push(current);

  // Build blocks with inherited_tags via ancestor stack
  const ancestorStack: { indent: number; tags: string[] }[] = [];
  const blocks: ParsedBlock[] = [];

  for (const { lineNum, indent, text } of raw) {
    while (ancestorStack.length > 0 && ancestorStack[ancestorStack.length - 1].indent >= indent) {
      ancestorStack.pop();
    }
    const seen = new Set<string>();
    const inherited_tags = ancestorStack
      .flatMap((a) => a.tags)
      .filter((t) => (seen.has(t) ? false : (seen.add(t), true)));

    const block = makeBlock(noteId, lineNum, indent, text, inherited_tags);
    ancestorStack.push({ indent, tags: block.tags });
    blocks.push(block);
  }

  return blocks;
}

/** Serialize editor blocks back to canonical Tesela Markdown body text. */
export function renderBlockBody(blocks: ParsedBlock[]): string {
  return blocks
    .map((block) => {
      const indent = "  ".repeat(block.indent_level);
      const rawText = block.bid
        ? stripStructuralBid(block.raw_text.split("\n")[0] ?? "", block.bid).text
          + (block.raw_text.includes("\n") ? block.raw_text.slice(block.raw_text.indexOf("\n")) : "")
        : block.raw_text;
      const lines = rawText.split("\n");
      const bidComment = block.bid ? `<!-- bid:${block.bid} -->` : "";

      if (fenceMarker(lines[0] ?? "") !== null) {
        const first = bidComment ? `${indent}- ${bidComment}` : `${indent}-`;
        const rest = lines.map((line) => `${indent}  ${line}`);
        return [first, ...rest].join("\n");
      }

      const firstText = lines[0] ?? "";
      const firstContent = [firstText, bidComment].filter((part) => part.length > 0).join(" ");
      const first = firstContent ? `${indent}- ${firstContent}` : `${indent}-`;
      const rest = lines.slice(1).map((line) => `${indent}  ${line}`);
      return [first, ...rest].join("\n");
    })
    .join("\n");
}

function makeBlock(noteId: string, lineNum: number, indentLevel: number, rawText: string, inherited_tags: string[]): ParsedBlock {
  const sourceLines = rawText.split("\n");
  const structural = stripStructuralBid(sourceLines[0] ?? "");
  const cleanLines = [structural.text, ...sourceLines.slice(1)];
  // Canonical lifted fences use a bid-only bullet followed by the opener as
  // a continuation. The empty bullet is scaffolding, not editor content.
  if (cleanLines[0]?.trim() === "" && cleanLines.length > 1 && fenceMarker(cleanLines[1] ?? "")) {
    cleanLines.shift();
  }
  const cleanRawText = cleanLines.join("\n");
  const markupText = textOutsideCodeFences(cleanRawText);
  const properties = extractProperties(markupText);

  // Merge tags from `tags::` property (new format) and inline `#tag` (legacy).
  // tags:: owns the slot — remove from properties so the right-sidebar
  // property pane doesn't double-display it next to the pill UI.
  const seen = new Set<string>();
  const tags: string[] = [];
  if (properties.tags !== undefined) {
    for (const t of properties.tags.split(",").map((s) => s.trim()).filter((s) => s.length > 0)) {
      const k = t.toLowerCase();
      if (!seen.has(k)) { seen.add(k); tags.push(t); }
    }
    delete properties.tags;
  }
  for (const t of extractTags(markupText)) {
    const k = t.toLowerCase();
    if (!seen.has(k)) { seen.add(k); tags.push(t); }
  }

  // Pull the bid out of the owning line's `<!-- bid:UUID -->` marker, so
  // we can re-emit it on save. Without this the save would strip the
  // bid (it only round-trips raw_text-with-stripped-bid), the server
  // would re-stamp a fresh UUID for the bid-less line, and
  // apply_block_upsert would append a duplicate file row.
  const bid = structural.bid;
  const firstLine = cleanRawText.split("\n")[0] ?? cleanRawText;
  const text = firstLine
    .replace(TAG_RE, "")
    .trim();

  const { inline: inline_tags, trailing: trailing_tags } = splitInlineAndTrailingTags(markupText);

  return {
    id: `${noteId}:${lineNum}`,
    bid,
    text,
    raw_text: cleanRawText,
    tags,
    inline_tags,
    trailing_tags,
    inherited_tags,
    properties,
    indent_level: indentLevel,
    note_id: noteId,
    // The client-side parser has no way to know the parent note's
    // `note_type` from inside a note body; that metadata lives on the
    // Note record itself. Server-side queries populate this field
    // when running `on:*` predicates. Leaving null on the web mirror
    // is safe — local parsers don't run those predicates.
    parent_note_type: null,
  };
}

/** Remove the rightmost structural bid from one logical owning line.
 * Earlier valid-looking comments may be literal lifted text. */
export function stripStructuralBid(
  line: string,
  expectedBid?: string,
): { text: string; bid: string | null } {
  const input = line.trimEnd();
  let owned: { start: number; end: number; bid: string } | null = null;
  BID_COMMENT_RE.lastIndex = 0;
  let match: RegExpExecArray | null;
  while ((match = BID_COMMENT_RE.exec(input)) !== null) {
    const bid = match[1];
    if (expectedBid === undefined || bid.toLowerCase() === expectedBid.toLowerCase()) {
      owned = { start: match.index, end: match.index + match[0].length, bid };
    }
  }
  if (owned === null) return { text: input, bid: null };

  const preceding = owned.start > 0 && /[ \t]/.test(input[owned.start - 1] ?? "")
    ? owned.start - 1
    : owned.start;
  return {
    text: (input.slice(0, preceding) + input.slice(owned.end)).trimEnd(),
    bid: owned.bid,
  };
}

/** Split `#tag` tokens into (inline, trailing). Mirrors the Rust impl in
 *  crates/tesela-core/src/block.rs:split_inline_and_trailing_tags. */
export function splitInlineAndTrailingTags(rawText: string): { inline: string[]; trailing: string[] } {
  let cursor = rawText.replace(/\s+$/, "").length;
  let clusterStart = cursor;
  const trailingStarts: number[] = [];

  for (;;) {
    while (cursor > 0 && /[ \t\n\r]/.test(rawText[cursor - 1] ?? "")) cursor -= 1;
    const nameEnd = cursor;
    while (cursor > 0 && /[A-Za-z0-9_/\-]/.test(rawText[cursor - 1] ?? "")) cursor -= 1;
    const nameStart = cursor;
    if (nameEnd === nameStart || cursor === 0 || rawText[cursor - 1] !== "#") break;
    cursor -= 1;
    clusterStart = cursor;
    trailingStarts.push(cursor);
  }

  const inlineText = rawText.slice(0, clusterStart);
  const inline: string[] = [];
  const inlineRe = /#([A-Za-z0-9_/-]+)/g;
  let im: RegExpExecArray | null;
  while ((im = inlineRe.exec(inlineText)) !== null) inline.push(im[1]);

  const trailing: string[] = trailingStarts
    .slice()
    .reverse()
    .map((pos) => {
      const after = rawText.slice(pos + 1);
      const mm = /^[A-Za-z0-9_/-]+/.exec(after);
      return mm ? mm[0] : "";
    })
    .filter((s) => s.length > 0);

  return { inline, trailing };
}

function extractTags(text: string): string[] {
  const tags: string[] = [];
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(text)) !== null) tags.push(m[1]);
  return tags;
}

function extractProperties(text: string): Record<string, string> {
  const props: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const m = PROPERTY_LINE_RE.exec(line);
    if (m !== null) props[m[1]] = m[2] ?? "";
  }
  return props;
}

export function extractWikiLinks(text: string): Array<{ target: string; display: string; start: number; end: number }> {
  const links: Array<{ target: string; display: string; start: number; end: number }> = [];
  WIKI_LINK_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = WIKI_LINK_RE.exec(text)) !== null) {
    links.push({
      target: m[1].trim(),
      display: m[2]?.trim() ?? m[1].trim(),
      start: m.index,
      end: m.index + m[0].length,
    });
  }
  return links;
}

export type TextSegment =
  | { type: "text"; value: string }
  | { type: "link"; value: string; href: string }
  | { type: "code"; value: string; lang: string };

function findCodeFenceSpans(text: string): CodeFenceSpan[] {
  const spans: CodeFenceSpan[] = [];
  const lines = text.split("\n");
  let offset = 0;
  let open: { from: number; contentStart: number; lang: string; marker: FenceMarker } | null = null;

  for (const line of lines) {
    const lineStart = offset;
    const lineEnd = lineStart + line.length;
    if (open === null) {
      const marker = fenceMarker(line);
      if (marker !== null) {
        const leading = line.length - line.trimStart().length;
        open = {
          from: lineStart,
          contentStart: lineEnd < text.length ? lineEnd + 1 : lineEnd,
          lang: line.slice(leading + marker.width).trim(),
          marker,
        };
      }
    } else if (closesFence(line, open.marker)) {
      const contentEnd = Math.max(open.contentStart, lineStart - 1);
      spans.push({
        from: open.from,
        to: lineEnd,
        lang: open.lang,
        value: text.slice(open.contentStart, contentEnd),
      });
      open = null;
    }
    offset = lineEnd + 1;
  }

  if (open !== null) {
    spans.push({
      from: open.from,
      to: text.length,
      lang: open.lang,
      value: text.slice(open.contentStart),
    });
  }

  return spans;
}

function textOutsideCodeFences(text: string): string {
  const spans = findCodeFenceSpans(text);
  if (spans.length === 0) return text;
  let out = "";
  let cursor = 0;
  for (const span of spans) {
    out += text.slice(cursor, span.from);
    out += text.slice(span.from, span.to).replace(/[^\r\n]/g, "|");
    cursor = span.to;
  }
  out += text.slice(cursor);
  return out;
}

function segmentInlineText(text: string): TextSegment[] {
  const links = extractWikiLinks(text);
  if (links.length === 0) return text ? [{ type: "text", value: text }] : [];
  const out: TextSegment[] = [];
  let cursor = 0;
  for (const link of links) {
    if (link.start > cursor) {
      out.push({ type: "text", value: text.slice(cursor, link.start) });
    }
    out.push({
      type: "link",
      value: link.display,
      href: "/p/" + encodeURIComponent(link.target.toLowerCase()),
    });
    cursor = link.end;
  }
  if (cursor < text.length) {
    out.push({ type: "text", value: text.slice(cursor) });
  }
  return out;
}

/** Split text into plain, wikilink, and fenced-code segments for rendering. */
export function segmentText(text: string): TextSegment[] {
  const codeSpans = findCodeFenceSpans(text);
  if (codeSpans.length === 0) return segmentInlineText(text);
  const out: TextSegment[] = [];
  let cursor = 0;
  for (const span of codeSpans) {
    if (span.from > cursor) {
      out.push(...segmentInlineText(text.slice(cursor, span.from)));
    }
    out.push({ type: "code", lang: span.lang, value: span.value });
    cursor = span.to;
  }
  if (cursor < text.length) {
    out.push(...segmentInlineText(text.slice(cursor)));
  }
  return out.length > 0 ? out : [{ type: "text", value: text }];
}

export function blockDisplayText(block: Pick<ParsedBlock, "text" | "raw_text">): string {
  if (findCodeFenceSpans(block.raw_text).length === 0) return block.text;
  const spans = findCodeFenceSpans(block.raw_text);
  const isInsideCode = (index: number) => spans.some((span) => index >= span.from && index < span.to);
  const lines = block.raw_text.split("\n");
  const kept: string[] = [];
  let offset = 0;
  for (const line of lines) {
    const lineStart = offset;
    if (isInsideCode(lineStart)) {
      kept.push(line);
    } else if (!PROPERTY_LINE_RE.test(line)) {
      kept.push(line.replace(TAG_RE, "").trimEnd());
    }
    offset += line.length + 1;
  }
  return kept.join("\n").trimEnd();
}

// ── Inline-span rendering contract (tesela-pfix.6) ──────────────────────────
//
// The shared fixture is crates/tesela-core/tests/fixtures/inline-span-conformance.json
// (consumed here and by app/Tesela-iOS/Sources/Components/BlockText.swift's
// `BlockText.parseInlineSpans`). See the fixture's `_contract` header for the
// full scope/precedence rules. This is a DELIBERATELY narrower, portable mirror
// of the unfocused-block markdown pass in cm-decorations.ts's buildDecorations
// — flat, non-nesting, single-line only — not a replacement for it.

export type InlineSpanKind = "plain" | "bold" | "italic" | "code" | "strike" | "link" | "wikilink";
export type InlineSpan = { kind: InlineSpanKind; text: string };

// Alternation order = precedence at a shared starting position: code > bold >
// italic > strike > wikilink > link. Groups: 1 code, 2/3 bold (**/__), 4
// italic, 5 strike, 6 wikilink, 7 link-text (8 link-url, unused — display-only
// contract).
const INLINE_SPAN_RE =
  /`([^`\n]+?)`|\*\*([^*\n]+?)\*\*|__([^_\n]+?)__|\*([^*\n]+?)\*|~~([^~\n]+?)~~|\[\[([^\]\n]+?)\]\]|\[([^\]\n]+?)\]\(([^)\n]+?)\)/g;

/** Parse single-line prose into the flat, ordered inline-span list both web
 *  and iOS render — the REAL production parser BlockText and (indirectly)
 *  cm-decorations.ts style prose with. See the fixture contract for scope. */
export function parseInlineSpans(text: string): InlineSpan[] {
  const spans: InlineSpan[] = [];
  let cursor = 0;
  INLINE_SPAN_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = INLINE_SPAN_RE.exec(text)) !== null) {
    if (m.index > cursor) {
      spans.push({ kind: "plain", text: text.slice(cursor, m.index) });
    }
    if (m[1] !== undefined) {
      spans.push({ kind: "code", text: m[1] });
    } else if (m[2] !== undefined || m[3] !== undefined) {
      spans.push({ kind: "bold", text: (m[2] ?? m[3])! });
    } else if (m[4] !== undefined) {
      spans.push({ kind: "italic", text: m[4] });
    } else if (m[5] !== undefined) {
      spans.push({ kind: "strike", text: m[5] });
    } else if (m[6] !== undefined) {
      spans.push({ kind: "wikilink", text: m[6] });
    } else if (m[7] !== undefined) {
      spans.push({ kind: "link", text: m[7] });
    }
    cursor = m.index + m[0].length;
  }
  if (cursor < text.length) {
    spans.push({ kind: "plain", text: text.slice(cursor) });
  }
  return spans;
}
