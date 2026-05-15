// Unit tests for the cm-decorations pure helpers — the atomic cursor
// snap logic that backs `teselaAtomicCursorFilter`. The decoration
// pipeline itself (ViewPlugin, theme) needs a live EditorView, so
// these focus on the regex+snap math that's reachable without one.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  findAtomicCursorRanges,
  snapHeadOutOfAtomicRanges,
} from "../../src/lib/cm-decorations.ts";

const EMPTY = { hide: new Set(), hideEmpty: new Set() };

test("findAtomicCursorRanges finds inline #tags", () => {
  const doc = "hello #bug there #urgent end";
  const r = findAtomicCursorRanges(doc, EMPTY);
  assert.deepEqual(r, [
    [6, 10],
    [17, 24],
  ]);
});

test("findAtomicCursorRanges finds bid comments with leading space", () => {
  const doc = "first line <!-- bid:abc12345-DEAD-BEEF-0000-1234567890ab --> tail";
  const r = findAtomicCursorRanges(doc, EMPTY);
  // Range covers the leading space + comment so the cursor doesn't land
  // on the trailing whitespace before the hidden token.
  assert.equal(r.length, 1);
  assert.equal(r[0][0], 10);
  assert.equal(doc.slice(r[0][0], r[0][1]), " <!-- bid:abc12345-DEAD-BEEF-0000-1234567890ab -->");
});

test("findAtomicCursorRanges includes tags:: line + trailing newline", () => {
  const doc = "before\ntags:: a, b\nafter";
  const r = findAtomicCursorRanges(doc, EMPTY);
  assert.equal(r.length, 1);
  // 7 = start of "tags::" line. End includes the \n.
  assert.equal(r[0][0], 7);
  // "tags:: a, b" is 11 chars starting at 7; range includes the \n at 18.
  assert.equal(r[0][1], 19);
});

test("findAtomicCursorRanges respects hide config for property lines", () => {
  const doc = "title\nuuid:: abc-123\nbody";
  const empty = findAtomicCursorRanges(doc, EMPTY);
  assert.equal(empty.length, 0, "no atomic range when uuid not hidden");
  const hidden = findAtomicCursorRanges(doc, {
    hide: new Set(["uuid"]),
    hideEmpty: new Set(),
  });
  assert.equal(hidden.length, 1);
  assert.equal(hidden[0][0], 6);
});

test("findAtomicCursorRanges hideEmpty only fires when value is empty", () => {
  const filled = "x\nfoo:: bar\ny";
  const empty = "x\nfoo:: \ny";
  const cfg = { hide: new Set(), hideEmpty: new Set(["foo"]) };
  assert.equal(findAtomicCursorRanges(filled, cfg).length, 0);
  assert.equal(findAtomicCursorRanges(empty, cfg).length, 1);
});

test("findAtomicCursorRanges always returns sorted ranges", () => {
  // tags:: line + a #tag — pushed in different order by the scanner,
  // sorted by from at the end.
  const doc = "tags:: x, y\nhello #bug";
  const r = findAtomicCursorRanges(doc, EMPTY);
  for (let i = 1; i < r.length; i++) {
    assert.ok(r[i][0] >= r[i - 1][0], "ranges sorted");
  }
});

// ── snapHeadOutOfAtomicRanges ─────────────────────────────────────────────

test("snap: head outside any range is unchanged", () => {
  const ranges = [
    [10, 20],
    [30, 40],
  ];
  assert.equal(snapHeadOutOfAtomicRanges(5, 0, ranges), 5);
  assert.equal(snapHeadOutOfAtomicRanges(25, 0, ranges), 25);
  assert.equal(snapHeadOutOfAtomicRanges(50, 0, ranges), 50);
});

test("snap: head with no motion (oldHead == newHead) is unchanged — mouse click into the middle of a tag stays put", () => {
  const ranges = [[10, 20]];
  // Click into a tag without prior motion → keep cursor where it is so
  // the docChanged-guarded path can land cleanly (typing won't fire this
  // filter anyway).
  assert.equal(snapHeadOutOfAtomicRanges(15, 15, ranges), 15);
});

test("snap: vim `l` landing AT `from` snaps past to `to` (boundary inclusive forward)", () => {
  const ranges = [[10, 20]];
  // Forward motion from char 9 → newHead = 10 = from. We snap to `to`
  // because `from` and `to` collapse to the same x for 0-width widgets.
  assert.equal(snapHeadOutOfAtomicRanges(10, 9, ranges), 20);
});

test("snap: vim `h` landing AT `to` snaps back to `from` (boundary inclusive backward)", () => {
  const ranges = [[10, 20]];
  // Backward motion from char 21 → newHead = 20 = to. Snap to from.
  assert.equal(snapHeadOutOfAtomicRanges(20, 21, ranges), 10);
});

test("snap: forward motion through an atomic range lands at `to`", () => {
  const ranges = [[10, 20]];
  assert.equal(snapHeadOutOfAtomicRanges(15, 9, ranges), 20);
  assert.equal(snapHeadOutOfAtomicRanges(15, 10, ranges), 20);
});

test("snap: backward motion through an atomic range lands at `from`", () => {
  const ranges = [[10, 20]];
  assert.equal(snapHeadOutOfAtomicRanges(15, 21, ranges), 10);
  assert.equal(snapHeadOutOfAtomicRanges(15, 20, ranges), 10);
});

test("snap: cursor exiting the range (newHead = to going forward) stays at to", () => {
  // Cursor was at `from` from a prior boundary-inclusive snap, then `l`
  // produces newHead = from+1. We just verified that gets snapped to `to`.
  // Now from `to`, `l` → newHead = to+1 (outside range entirely). No snap.
  const ranges = [[10, 20]];
  assert.equal(snapHeadOutOfAtomicRanges(21, 20, ranges), 21);
});

test("snap: only the first matching range fires (ranges are non-overlapping)", () => {
  const ranges = [
    [10, 20],
    [30, 40],
  ];
  assert.equal(snapHeadOutOfAtomicRanges(35, 0, ranges), 40);
});

test("snap: cm-vim `l` step from before-tag lands past the tag", () => {
  // Doc: "abc #bug rest". Tag at [4, 8]. Cursor at ch=4 ('#').
  // Pressing `l` in cm-vim: newHead = 5, oldHead = 4. After our filter:
  // head should snap forward to 8.
  const ranges = findAtomicCursorRanges("abc #bug rest", EMPTY);
  assert.deepEqual(ranges, [[4, 8]]);
  assert.equal(snapHeadOutOfAtomicRanges(5, 4, ranges), 8);
});
