/**
 * Block tag manipulation: tags live in a `tags::` child property line on each
 * block. Legacy inline `#tag` tokens are still read (so old notes keep working)
 * but new writes always go through the `tags::` line.
 */

const TAGS_LINE_RE = /^tags:: (.+)$/;
const TAGS_LINE_RE_M = /^tags:: (.+)$/m;
const INLINE_TAG_RE = /#([A-Za-z0-9_/-]+)/g;

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/** Read the merged tag list (tags:: property first, then any inline #tag tokens). */
export function getBlockTags(rawText: string): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  const m = rawText.match(TAGS_LINE_RE_M);
  if (m) {
    for (const t of m[1].split(",").map((s) => s.trim()).filter((s) => s.length > 0)) {
      const k = t.toLowerCase();
      if (!seen.has(k)) {
        seen.add(k);
        out.push(t);
      }
    }
  }
  for (const inline of rawText.matchAll(INLINE_TAG_RE)) {
    const t = inline[1];
    const k = t.toLowerCase();
    if (!seen.has(k)) {
      seen.add(k);
      out.push(t);
    }
  }
  return out;
}

/**
 * Toggle a tag on/off the block's `tags::` line. If the tag is currently
 * present (in either tags:: or as a legacy inline #tag), it's removed from
 * both. If absent, it's appended to the tags:: line (creating the line if
 * needed).
 *
 * When ADDING a tag, optional `addPropertyNames` are appended as empty
 * `name:: ` continuation lines (skipping any names already present on the
 * block). Caller is expected to filter out hide_by_default-flagged names —
 * the function doesn't make that decision itself. When REMOVING a tag, the
 * function does NOT strip property lines (preserves user-entered values).
 */
export function toggleBlockTag(
  rawText: string,
  tagName: string,
  addPropertyNames: string[] = [],
): string {
  const lower = tagName.toLowerCase();
  const inlineRe = new RegExp(`\\s*#${escapeRe(tagName)}(?![A-Za-z0-9_/-])`, "gi");

  const lines = rawText.split("\n");
  const tagsIdx = lines.findIndex((l) => TAGS_LINE_RE.test(l));
  const currentList = tagsIdx >= 0
    ? lines[tagsIdx].match(TAGS_LINE_RE)![1]
        .split(",")
        .map((s) => s.trim())
        .filter((s) => s.length > 0)
    : [];
  const inListIdx = currentList.findIndex((t) => t.toLowerCase() === lower);
  const hasInline = inlineRe.test(rawText);
  inlineRe.lastIndex = 0;

  if (inListIdx >= 0 || hasInline) {
    // Remove — DON'T touch property lines
    const stripped = rawText.replace(inlineRe, "");
    const resultLines = stripped.split("\n");
    const newTagsIdx = resultLines.findIndex((l) => TAGS_LINE_RE.test(l));
    if (newTagsIdx >= 0) {
      const newList = currentList.filter((t) => t.toLowerCase() !== lower);
      if (newList.length === 0) {
        resultLines.splice(newTagsIdx, 1);
      } else {
        resultLines[newTagsIdx] = `tags:: ${newList.join(", ")}`;
      }
    }
    return resultLines.join("\n");
  }

  // Add — write tags:: line then auto-append any missing property lines
  let added: string;
  if (tagsIdx >= 0) {
    const updated = [...lines];
    updated[tagsIdx] = `tags:: ${[...currentList, tagName].join(", ")}`;
    added = updated.join("\n");
  } else {
    added = `${rawText}\ntags:: ${tagName}`;
  }

  if (addPropertyNames.length === 0) return added;

  // Find existing keys (case-insensitive) to skip duplicates.
  const existingKeyRe = /^([A-Za-z_][A-Za-z0-9_]*)::/gm;
  const existingKeys = new Set<string>();
  let m: RegExpExecArray | null;
  while ((m = existingKeyRe.exec(added)) !== null) {
    existingKeys.add(m[1].toLowerCase());
  }

  const toAppend: string[] = [];
  for (const name of addPropertyNames) {
    if (!name) continue;
    if (existingKeys.has(name.toLowerCase())) continue;
    existingKeys.add(name.toLowerCase());
    toAppend.push(`${name}:: `);
  }
  if (toAppend.length === 0) return added;
  return `${added}\n${toAppend.join("\n")}`;
}
