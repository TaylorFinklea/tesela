/**
 * How pressing Enter splits a (possibly multi-line) block in the outliner
 * editor.
 *
 * A block's editor doc is its prose line followed by continuation lines —
 * `tags:: Task`, `key:: value`, … — which are CSS-hidden when not being
 * edited. Splitting must NEVER carve those continuation/property lines onto the
 * NEW block: they belong to the block whose prose they annotate. Two guards
 * enforce that, beyond a plain raw-offset split:
 *
 *  - Phase 10.1: Enter on the prose (first) line keeps ALL continuation lines
 *    with the current block (the new block gets only the post-cursor prose).
 *  - 2026-06-15: Enter on a property/`tags::` continuation line keeps the whole
 *    block intact and drops a clean empty sibling. Without this, Enter on a
 *    task's `testpoints:: 10` line shipped the trailing `tags:: Task` (+ the
 *    display scaffold it drives) onto the new block, stripping task-ness off
 *    the original.
 */
export type EnterSplitPlan = {
  /** New full text for the CURRENT block, or `null` to leave it untouched. */
  current: string | null;
  /** `raw_text` for the NEW block created below. */
  next: string;
};

/**
 * Editor-layer property / `tags::` line matcher. Mirrors `PROPERTY_RE` in
 * cm-decorations.ts and `PROPERTY_LINE_RE` in block-parser.ts — the project
 * keeps independent copies of this pattern rather than a shared export.
 */
const PROPERTY_LINE_RE = /^([A-Za-z_][A-Za-z0-9_]*)::[ \t]?/;

/**
 * Decide what the current and new block receive when Enter is pressed.
 *
 * @param doc            full block source (prose line + continuation lines)
 * @param cursor         absolute character offset of the cursor into `doc`
 * @param cursorLineText text of the single line the cursor sits on
 */
export function planEnterSplit(
  doc: string,
  cursor: number,
  cursorLineText: string,
): EnterSplitPlan {
  const firstNl = doc.indexOf("\n");
  const cursorOnFirstLine = firstNl === -1 || cursor <= firstNl;

  // Cursor on the prose (first) line of a block that has continuation lines:
  // split the prose, keep ALL continuation lines with the current block.
  if (cursorOnFirstLine && firstNl !== -1) {
    const firstLine = doc.slice(0, firstNl);
    const continuation = doc.slice(firstNl); // includes the leading \n
    return {
      current: firstLine.slice(0, cursor) + continuation,
      next: firstLine.slice(cursor),
    };
  }

  // Cursor on a property / `tags::` continuation line: never carve property
  // lines across blocks — keep the whole block intact, create an empty sibling.
  if (PROPERTY_LINE_RE.test(cursorLineText)) {
    return { current: null, next: "" };
  }

  // Cursor in genuine multi-line prose: split at the raw offset (the original
  // behavior; the current block is only rewritten when there is trailing text).
  const textAfter = doc.slice(cursor);
  return { current: textAfter ? doc.slice(0, cursor) : null, next: textAfter };
}
