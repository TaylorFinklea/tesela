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
  const blocks: ParsedBlock[] = [];
  let current: { lineNum: number; indent: number; text: string } | null = null;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();
    if (trimmed === "") continue;

    const spaces = line.length - line.trimStart().length;
    const indent = Math.floor(spaces / 2);

    if (trimmed.startsWith("- ")) {
      if (current) blocks.push(makeBlock(noteId, current.lineNum, current.indent, current.text));
      current = { lineNum: i, indent, text: trimmed.slice(2) };
    } else if (current) {
      current.text += "\n" + trimmed;
    }
  }

  if (current) blocks.push(makeBlock(noteId, current.lineNum, current.indent, current.text));
  return blocks;
}

function makeBlock(noteId: string, lineNum: number, indentLevel: number, rawText: string): ParsedBlock {
  const tags = extractTags(rawText);
  const properties = extractProperties(rawText);
  const firstLine = rawText.split("\n")[0] ?? rawText;
  const text = firstLine.replace(TAG_RE, "").trim();

  return {
    id: `${noteId}:${lineNum}`,
    text,
    raw_text: rawText,
    tags,
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
