import { test } from "node:test";
import assert from "node:assert/strict";
import { clampTableCursor, moveTableCursor } from "../../src/lib/table/table-nav.ts";

// tesela-ya4.3 — the 2D keyboard-cursor model IS the acceptance contract
// ("keyboard navigates rows AND columns"): row moves must never bleed into
// the column axis and vice versa, and every move clamps at the table's
// bounds instead of wrapping or going out of range.

test("clampTableCursor: clamps both axes independently into range", () => {
  assert.deepEqual(clampTableCursor({ row: 5, col: 5 }, 3, 4), { row: 2, col: 3 });
  assert.deepEqual(clampTableCursor({ row: -1, col: -1 }, 3, 4), { row: 0, col: 0 });
  assert.deepEqual(clampTableCursor({ row: 1, col: 2 }, 3, 4), { row: 1, col: 2 }, "in-range cursor is unchanged");
});

test("clampTableCursor: a zero-length axis clamps to 0, not negative", () => {
  assert.deepEqual(clampTableCursor({ row: 4, col: 2 }, 0, 4), { row: 0, col: 2 });
  assert.deepEqual(clampTableCursor({ row: 4, col: 2 }, 3, 0), { row: 2, col: 0 });
  assert.deepEqual(clampTableCursor({ row: 4, col: 2 }, 0, 0), { row: 0, col: 0 });
});

test("moveTableCursor: down/up move the row axis only, column untouched", () => {
  const start = { row: 1, col: 2 };
  assert.deepEqual(moveTableCursor(start, "down", 5, 5), { row: 2, col: 2 });
  assert.deepEqual(moveTableCursor(start, "up", 5, 5), { row: 0, col: 2 });
});

test("moveTableCursor: left/right move the column axis only, row untouched", () => {
  const start = { row: 1, col: 2 };
  assert.deepEqual(moveTableCursor(start, "left", 5, 5), { row: 1, col: 1 });
  assert.deepEqual(moveTableCursor(start, "right", 5, 5), { row: 1, col: 3 });
});

test("moveTableCursor: down/up/left/right clamp at the table edge instead of going out of range", () => {
  assert.deepEqual(moveTableCursor({ row: 2, col: 0 }, "down", 3, 5), { row: 2, col: 0 }, "last row — down is a no-op");
  assert.deepEqual(moveTableCursor({ row: 0, col: 0 }, "up", 3, 5), { row: 0, col: 0 }, "first row — up is a no-op");
  assert.deepEqual(moveTableCursor({ row: 0, col: 0 }, "left", 3, 5), { row: 0, col: 0 }, "first col — left is a no-op");
  assert.deepEqual(moveTableCursor({ row: 0, col: 4 }, "right", 3, 5), { row: 0, col: 4 }, "last col — right is a no-op");
});

test("moveTableCursor: first-row/last-row jump the row axis, preserving the column", () => {
  const start = { row: 1, col: 3 };
  assert.deepEqual(moveTableCursor(start, "first-row", 5, 5), { row: 0, col: 3 });
  assert.deepEqual(moveTableCursor(start, "last-row", 5, 5), { row: 4, col: 3 });
});

test("moveTableCursor: first-col/last-col jump the column axis, preserving the row", () => {
  const start = { row: 3, col: 1 };
  assert.deepEqual(moveTableCursor(start, "first-col", 5, 5), { row: 3, col: 0 });
  assert.deepEqual(moveTableCursor(start, "last-col", 5, 5), { row: 3, col: 4 });
});

test("moveTableCursor: every step clamps against an empty table (0 rows or 0 cols) without going negative", () => {
  for (const step of ["down", "up", "left", "right", "first-row", "last-row", "first-col", "last-col"]) {
    assert.deepEqual(moveTableCursor({ row: 0, col: 0 }, step, 0, 0), { row: 0, col: 0 }, `step=${step}`);
  }
});
