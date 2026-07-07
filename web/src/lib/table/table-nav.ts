/**
 * tesela-ya4.3 — pure 2D keyboard-cursor model for the generalized query
 * table: row nav (j/k, g/G) AND column nav (h/l, 0/$). Extracted so the
 * clamp-at-bounds acceptance contract ("keyboard navigates rows AND
 * columns") is unit-testable without mounting `QueryTable.svelte` (mirrors
 * why `kanban-group-by.ts` was extracted for the kanban board).
 *
 * Column 0 is always the fixed "Block" label column; columns 1..N are the
 * resolved typed property columns (`table-columns.ts`), so `colCount` is
 * `1 + propertyColumns.length` at every call site.
 */

export interface TableCursor {
  row: number;
  col: number;
}

export type TableNavStep =
  | "down"
  | "up"
  | "left"
  | "right"
  | "first-row"
  | "last-row"
  | "first-col"
  | "last-col";

/** Clamp a cursor into `[0, rowCount)` × `[0, colCount)`. A zero-length
 *  axis clamps to 0 (the empty-table case — nothing to focus, but the
 *  cursor stays a valid, non-negative coordinate rather than going
 *  negative or out of range). */
export function clampTableCursor(cursor: TableCursor, rowCount: number, colCount: number): TableCursor {
  return {
    row: rowCount <= 0 ? 0 : Math.min(Math.max(cursor.row, 0), rowCount - 1),
    col: colCount <= 0 ? 0 : Math.min(Math.max(cursor.col, 0), colCount - 1),
  };
}

export function moveTableCursor(
  cursor: TableCursor,
  step: TableNavStep,
  rowCount: number,
  colCount: number,
): TableCursor {
  switch (step) {
    case "down":
      return clampTableCursor({ row: cursor.row + 1, col: cursor.col }, rowCount, colCount);
    case "up":
      return clampTableCursor({ row: cursor.row - 1, col: cursor.col }, rowCount, colCount);
    case "left":
      return clampTableCursor({ row: cursor.row, col: cursor.col - 1 }, rowCount, colCount);
    case "right":
      return clampTableCursor({ row: cursor.row, col: cursor.col + 1 }, rowCount, colCount);
    case "first-row":
      return clampTableCursor({ row: 0, col: cursor.col }, rowCount, colCount);
    case "last-row":
      return clampTableCursor({ row: rowCount - 1, col: cursor.col }, rowCount, colCount);
    case "first-col":
      return clampTableCursor({ row: cursor.row, col: 0 }, rowCount, colCount);
    case "last-col":
      return clampTableCursor({ row: cursor.row, col: colCount - 1 }, rowCount, colCount);
  }
}
