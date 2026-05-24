/**
 * Pure body-mutation helpers for the block-id-addressed write paths
 * (triage actions, in-place row edits, deletes). Kept in a plain `.ts`
 * file with zero runtime dependencies so they're independently
 * unit-testable; `triage.svelte.ts` re-exports them for the existing
 * call surface.
 *
 * **Line-numbering contract.** Block ids are `{noteId}:{lineNumber}`
 * where `lineNumber` indexes into the body the server's parser sees.
 * That body is `gray_matter`'s trimmed content — i.e. it starts at the
 * first non-blank line *after* the closing frontmatter fence. The
 * helpers here must match that contract; the `splitFrontmatter` below
 * therefore consumes any blank lines that follow the fence so
 * `lines[0]` is the same line the server's `parse_blocks` would call
 * `line_num = 0`. The bug this fixed: an off-by-one where `:0` aimed
 * at the blank line and mutations landed *above* the bullet.
 */

function splitFrontmatter(content: string): { frontmatter: string; body: string } {
  const fmEnd = content.startsWith("---") ? content.indexOf("---", 3) : -1;
  let splitAt = fmEnd >= 0 ? fmEnd + 3 + (content[fmEnd + 3] === "\n" ? 1 : 0) : 0;
  // Match the server's body contract: skip leading blank lines so
  // `lines[0]` aligns with `parse_blocks` line numbering.
  while (splitAt < content.length && content[splitAt] === "\n") splitAt++;
  return { frontmatter: content.slice(0, splitAt), body: content.slice(splitAt) };
}

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Insert (or replace) a `key:: value` continuation line on the block whose
 * deterministic id is `blockId` (`{pageId}:{lineNumber}`). Pure function — no
 * I/O. Mirrors the line-number addressing the indexer uses.
 */
export function setBlockProperty(
  content: string,
  blockId: string,
  key: string,
  value: string,
): string {
  const colonIdx = blockId.lastIndexOf(":");
  if (colonIdx < 0) return content;
  const lineNumStr = blockId.slice(colonIdx + 1);
  const lineNum = Number.parseInt(lineNumStr, 10);
  if (!Number.isFinite(lineNum)) return content;

  const { frontmatter, body } = splitFrontmatter(content);
  const lines = body.split("\n");
  if (lineNum >= lines.length) return content;

  const targetLine = lines[lineNum];
  const indent = targetLine.length - targetLine.trimStart().length;
  const continuationIndent = indent + 2;

  const re = new RegExp(`^${escapeRegex(key)}::`);
  let cursor = lineNum + 1;
  let propLineIdx = -1;
  while (cursor < lines.length) {
    const l = lines[cursor];
    const ts = l.trimStart();
    if (ts.length > 0) {
      const cur = l.length - ts.length;
      // A new bullet at <= indent ends the block.
      if (ts.startsWith("- ") && cur <= indent) break;
      if (cur >= continuationIndent && re.test(ts)) {
        propLineIdx = cursor;
        break;
      }
    }
    cursor++;
  }

  const newLine = `${" ".repeat(continuationIndent)}${key}:: ${value}`;
  if (propLineIdx >= 0) {
    lines[propLineIdx] = newLine;
  } else {
    lines.splice(lineNum + 1, 0, newLine);
  }

  return frontmatter + lines.join("\n");
}

/**
 * Replace a block's first-line text. Preserves the bullet (`- `), indent,
 * and any inline `#tags` or property-style content already on the first
 * line; if `newText` already contains hashtags, those win. Continuation
 * lines (indented properties / sub-paragraphs) are untouched.
 */
export function setBlockText(
  content: string,
  blockId: string,
  newText: string,
): string {
  const colonIdx = blockId.lastIndexOf(":");
  if (colonIdx < 0) return content;
  const lineNum = Number.parseInt(blockId.slice(colonIdx + 1), 10);
  if (!Number.isFinite(lineNum)) return content;

  const { frontmatter, body } = splitFrontmatter(content);
  const lines = body.split("\n");
  if (lineNum >= lines.length) return content;

  const targetLine = lines[lineNum];
  const indent = targetLine.length - targetLine.trimStart().length;
  // Newlines in newText would corrupt the body shape; collapse to spaces.
  const safe = newText.replace(/\r?\n/g, " ").trim();
  lines[lineNum] = `${" ".repeat(indent)}- ${safe}`;
  return frontmatter + lines.join("\n");
}

/**
 * Remove a block (bullet line + all continuation/child lines under it).
 * The block ends at the next line whose indent is `<= indent` and that
 * starts with a bullet (`- `).
 */
export function deleteBlock(content: string, blockId: string): string {
  const colonIdx = blockId.lastIndexOf(":");
  if (colonIdx < 0) return content;
  const lineNum = Number.parseInt(blockId.slice(colonIdx + 1), 10);
  if (!Number.isFinite(lineNum)) return content;

  const { frontmatter, body } = splitFrontmatter(content);
  const lines = body.split("\n");
  if (lineNum >= lines.length) return content;

  const targetLine = lines[lineNum];
  const indent = targetLine.length - targetLine.trimStart().length;
  let end = lines.length;
  for (let i = lineNum + 1; i < lines.length; i++) {
    const l = lines[i];
    const ts = l.trimStart();
    if (ts.length === 0) continue; // blank line — keep walking
    const cur = l.length - ts.length;
    if (ts.startsWith("- ") && cur <= indent) {
      end = i;
      break;
    }
  }
  lines.splice(lineNum, end - lineNum);
  return frontmatter + lines.join("\n");
}
