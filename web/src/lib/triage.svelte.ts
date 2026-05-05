/**
 * Triage flow for the Inbox widget (Phase 9.2). Single-key handlers that fire
 * when the middle column has focus AND the active widget is `inbox`.
 *
 * `t` → set status:: todo
 * `d` → set status:: doing
 * `x` → set status:: done (archive — drops out of inbox query)
 *
 * Implementation: fetches the focused row's containing note, edits the
 * referenced block's body to insert/replace `status:: <value>`, PUTs back. The
 * subsequent WS echo invalidates `["widget", "inbox"]` and the row drops out
 * of the list.
 */
import { api } from "$lib/api-client";

export type TriageAction = "todo" | "doing" | "done";

const ACTIONS: Record<string, TriageAction> = {
  t: "todo",
  d: "doing",
  x: "done",
};

export function triageActionForKey(key: string): TriageAction | null {
  return ACTIONS[key.toLowerCase()] ?? null;
}

/**
 * Apply a triage action to the block identified by `blockId` inside the note
 * `pageId`. Returns true if the PUT was issued; false if the block couldn't
 * be located (e.g. stale row).
 */
export async function applyTriage(
  pageId: string,
  blockId: string,
  action: TriageAction,
): Promise<boolean> {
  const note = await api.getNote(pageId);
  const updated = setBlockStatus(note.content, blockId, action);
  if (updated === note.content) return false;
  await api.updateNote(pageId, updated);
  return true;
}

/**
 * Insert (or replace) a `status:: <value>` continuation line on the block.
 * Convenience wrapper around `setBlockProperty`.
 */
export function setBlockStatus(
  content: string,
  blockId: string,
  action: TriageAction,
): string {
  return setBlockProperty(content, blockId, "status", action);
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
  // Block id format: `{pageId}:{lineNumber}`. The line number indexes into the
  // body (post-frontmatter). We need the body's line N to find where to insert.
  const colonIdx = blockId.lastIndexOf(":");
  if (colonIdx < 0) return content;
  const lineNumStr = blockId.slice(colonIdx + 1);
  const lineNum = Number.parseInt(lineNumStr, 10);
  if (!Number.isFinite(lineNum)) return content;

  // Split frontmatter from body — same logic as elsewhere.
  const fmEnd = content.startsWith("---") ? content.indexOf("---", 3) : -1;
  const splitAt = fmEnd >= 0 ? fmEnd + 3 + (content[fmEnd + 3] === "\n" ? 1 : 0) : 0;
  const frontmatter = content.slice(0, splitAt);
  const body = content.slice(splitAt);
  const lines = body.split("\n");
  if (lineNum >= lines.length) return content;

  // Find the block's bullet line and its indent. The block continues until the
  // next line at indent ≤ this one starting with `- `.
  const targetLine = lines[lineNum];
  const indent = targetLine.length - targetLine.trimStart().length;
  const continuationIndent = indent + 2;

  // Look for an existing `key::` continuation line within the block.
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

function escapeRegex(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * Replace a block's first-line text. Preserves the bullet (`- `), indent,
 * and any inline `#tags` or property-style content already on the first
 * line; if `newText` already contains hashtags, those win. Continuation
 * lines (indented properties / sub-paragraphs) are untouched. Pure — no I/O.
 *
 * Used by Phase 10.1's in-place row edit in the query widget. Same line-
 * number addressing as `setBlockProperty`, so callers compose them freely.
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

  const fmEnd = content.startsWith("---") ? content.indexOf("---", 3) : -1;
  const splitAt = fmEnd >= 0 ? fmEnd + 3 + (content[fmEnd + 3] === "\n" ? 1 : 0) : 0;
  const frontmatter = content.slice(0, splitAt);
  const body = content.slice(splitAt);
  const lines = body.split("\n");
  if (lineNum >= lines.length) return content;

  const targetLine = lines[lineNum];
  const indent = targetLine.length - targetLine.trimStart().length;
  // Reconstruct as `<indent>- <newText>`. Newlines in newText would corrupt
  // the body shape; collapse them into spaces.
  const safe = newText.replace(/\r?\n/g, " ").trim();
  lines[lineNum] = `${" ".repeat(indent)}- ${safe}`;
  return frontmatter + lines.join("\n");
}

/**
 * Remove a block (bullet line + all continuation/child lines under it).
 * The block ends at the next line whose indent is `<= indent` and that
 * starts with a bullet (`- `). Pure — no I/O.
 *
 * Used by Phase 10.1's slash-menu "Delete" command on query rows.
 */
export function deleteBlock(content: string, blockId: string): string {
  const colonIdx = blockId.lastIndexOf(":");
  if (colonIdx < 0) return content;
  const lineNum = Number.parseInt(blockId.slice(colonIdx + 1), 10);
  if (!Number.isFinite(lineNum)) return content;

  const fmEnd = content.startsWith("---") ? content.indexOf("---", 3) : -1;
  const splitAt = fmEnd >= 0 ? fmEnd + 3 + (content[fmEnd + 3] === "\n" ? 1 : 0) : 0;
  const frontmatter = content.slice(0, splitAt);
  const body = content.slice(splitAt);
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
    if (ts.startsWith("- ") && cur <= indent) { end = i; break; }
  }
  lines.splice(lineNum, end - lineNum);
  return frontmatter + lines.join("\n");
}

/**
 * Attach a block to a project page by setting `project:: <projectId>`.
 * Wraps `setBlockProperty` + the API PUT.
 */
export async function attachToProject(
  pageId: string,
  blockId: string,
  projectId: string,
): Promise<boolean> {
  const note = await api.getNote(pageId);
  const updated = setBlockProperty(note.content, blockId, "project", projectId);
  if (updated === note.content) return false;
  await api.updateNote(pageId, updated);
  return true;
}
