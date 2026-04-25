/**
 * Block-level parser for Tesela note bodies.
 * Port of crates/tesela-core/src/block.rs.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";

const TAG_RE = /#([A-Za-z0-9_/-]+)/g;
const PROPERTY_RE = /([A-Za-z_][A-Za-z0-9_]*):: (.+)/g;
const WIKI_LINK_RE = /\[\[([^\]|]+)(?:\|([^\]]+))?\]\]/g;

export function parseBlocks(noteId: string, body: string): ParsedBlock[] {
  const lines = body.split("\n");
  const raw: { lineNum: number; indent: number; text: string }[] = [];
  let current: { lineNum: number; indent: number; text: string } | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
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
      current = { lineNum: i, indent, text };
    } else if (current) {
      current.text += "\n" + trimStart.trimEnd();
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

function makeBlock(noteId: string, lineNum: number, indentLevel: number, rawText: string, inherited_tags: string[]): ParsedBlock {
  const properties = extractProperties(rawText);

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
  for (const t of extractTags(rawText)) {
    const k = t.toLowerCase();
    if (!seen.has(k)) { seen.add(k); tags.push(t); }
  }

  const firstLine = rawText.split("\n")[0] ?? rawText;
  const text = firstLine.replace(TAG_RE, "").trim();

  return {
    id: `${noteId}:${lineNum}`,
    text,
    raw_text: rawText,
    tags,
    inherited_tags,
    properties,
    indent_level: indentLevel,
    note_id: noteId,
  };
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
  PROPERTY_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = PROPERTY_RE.exec(text)) !== null) props[m[1]] = m[2];
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
