// Unit tests for the GFM pipe-table pure helpers:
//   splitPipeCells  — split one raw table row into trimmed cells
//   findPipeTables  — detect all pipe-table regions in a doc string
//
// These run without a live CodeMirror editor; the table-widget rendering
// (TableWidget, teselaTableDecorations StateField) requires a real DOM and
// is covered by manual QA.
//
// Import note: the test runner uses a strip-only TS loader, so we import
// the TS source file directly (as in the other unit test files).

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  findPipeTables,
  splitPipeCells,
} from "../../src/lib/cm-decorations.ts";

// ── splitPipeCells ────────────────────────────────────────────────────────

test("splitPipeCells: leading + trailing pipe both stripped", () => {
  assert.deepEqual(splitPipeCells("| a | b |"), ["a", "b"]);
});

test("splitPipeCells: leading pipe only", () => {
  assert.deepEqual(splitPipeCells("| a | b"), ["a", "b"]);
});

test("splitPipeCells: trailing pipe only", () => {
  assert.deepEqual(splitPipeCells("a | b |"), ["a", "b"]);
});

test("splitPipeCells: no surrounding pipes", () => {
  assert.deepEqual(splitPipeCells("a | b"), ["a", "b"]);
});

test("splitPipeCells: trims whitespace around each cell", () => {
  assert.deepEqual(splitPipeCells("|   a   |   b   |"), ["a", "b"]);
});

test("splitPipeCells: single cell with surrounding pipes", () => {
  // `| a |` — leading AND trailing pipe both present, both dropped, one cell.
  assert.deepEqual(splitPipeCells("| a |"), ["a"]);
});

test("splitPipeCells: three cells", () => {
  assert.deepEqual(splitPipeCells("| x | y | z |"), ["x", "y", "z"]);
});

// ── findPipeTables ────────────────────────────────────────────────────────

test("findPipeTables: returns empty for text with no table", () => {
  assert.deepEqual(findPipeTables("hello world"), []);
  assert.deepEqual(findPipeTables("just a #tag in a #note"), []);
  assert.deepEqual(findPipeTables(""), []);
});

test("findPipeTables: detects a 2-col table with one body row", () => {
  const doc = "| a | b |\n|---|---|\n| 1 | 2 |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].header, ["a", "b"]);
  assert.deepEqual(r[0].body, [["1", "2"]]);
  assert.deepEqual(r[0].align, [null, null]);
});

test("findPipeTables: table with no body rows (header + separator only)", () => {
  const doc = "| a | b |\n|---|---|";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].header, ["a", "b"]);
  assert.deepEqual(r[0].body, []);
});

test("findPipeTables: alignment markers (left / right / center)", () => {
  const doc = "| a | b | c |\n|:---|---:|:---:|";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].align, ["left", "right", "center"]);
  assert.deepEqual(r[0].body, []);
});

test("findPipeTables: a header without a separator is NOT a table", () => {
  const doc = "| a | b |\nthis is not a separator";
  assert.deepEqual(findPipeTables(doc), []);
});

test("findPipeTables: a body row with wrong column count ends the table", () => {
  const doc = "| a | b |\n|---|---|\n| 1 | 2 | 3 |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  // The 3-cell row has the wrong count — table body is empty.
  assert.deepEqual(r[0].body, []);
});

test("findPipeTables: blank line ends a table; two separate tables in one doc", () => {
  const doc = "| a | b |\n|---|---|\n| 1 | 2 |\n\n| c | d |\n|---|---|\n| 3 | 4 |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 2);
  assert.deepEqual(r[0].header, ["a", "b"]);
  assert.deepEqual(r[0].body, [["1", "2"]]);
  assert.deepEqual(r[1].header, ["c", "d"]);
  assert.deepEqual(r[1].body, [["3", "4"]]);
});

test("findPipeTables: range covers the whole table region", () => {
  const doc = "before\n| a | b |\n|---|---|\n| 1 | 2 |\nafter";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  // Region: from start of header line to start of the line after the table.
  // The trailing newline is part of the range (so `to` lands at "after").
  assert.equal(doc.slice(r[0].from, r[0].to), "| a | b |\n|---|---|\n| 1 | 2 |\n");
  assert.equal(doc.slice(r[0].to, r[0].to + 5), "after");
});

test("findPipeTables: last-line table (no trailing newline)", () => {
  const doc = "| a | b |\n|---|---|\n| 1 | 2 |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  // to == doc.length when the table ends the doc.
  assert.equal(r[0].to, doc.length);
  assert.equal(doc.slice(r[0].from, r[0].to), doc);
});

test("findPipeTables: rows without leading/trailing pipes (loose GFM style)", () => {
  const doc = "a | b\n---|---\n1 | 2";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].header, ["a", "b"]);
  assert.deepEqual(r[0].body, [["1", "2"]]);
});

test("findPipeTables: a #tag inside a cell is plain cell text (not a tag mark)", () => {
  // The detector returns raw cell text. The ViewPlugin's tag pass is what
  // skips table regions at decoration time. This test pins the contract:
  // cell content is preserved with only cell-level trimming.
  const doc = "| a #tag | b |\n|---|---|\n| c | d |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].header, ["a #tag", "b"]);
});

test("findPipeTables: a [[wikilink]] inside a cell is plain cell text", () => {
  const doc = "| a | [[Page]] |\n|---|---|\n| b | c |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].header, ["a", "[[Page]]"]);
});

test("findPipeTables: separator requires at least one dash + a pipe (not just ---)", () => {
  // `---` alone is a setext underline / horizontal rule, NOT a table separator.
  const doc = "header\n---\nrow";
  assert.deepEqual(findPipeTables(doc), []);
});

test("findPipeTables: multiple body rows", () => {
  const doc = "| Name | Value |\n|---|---|\n| foo | 1 |\n| bar | 2 |\n| baz | 3 |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].body, [["foo", "1"], ["bar", "2"], ["baz", "3"]]);
});

test("findPipeTables: three-column table", () => {
  const doc = "| A | B | C |\n|---|---|---|\n| 1 | 2 | 3 |";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].header, ["A", "B", "C"]);
  assert.deepEqual(r[0].body, [["1", "2", "3"]]);
  assert.deepEqual(r[0].align, [null, null, null]);
});

test("findPipeTables: table embedded in larger doc", () => {
  const doc = "Some prose before.\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\nSome prose after.";
  const r = findPipeTables(doc);
  assert.equal(r.length, 1);
  assert.deepEqual(r[0].header, ["a", "b"]);
  assert.deepEqual(r[0].body, [["1", "2"]]);
  // Offsets do NOT include the leading "Some prose before.\n\n".
  assert.equal(doc.slice(r[0].from, r[0].from + 9), "| a | b |");
});
