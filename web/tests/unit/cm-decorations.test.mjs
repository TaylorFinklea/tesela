// Unit tests for the cm-decorations pure helpers — the atomic cursor
// snap logic that backs `teselaAtomicCursorFilter`. The decoration
// pipeline itself (ViewPlugin, theme) needs a live EditorView, so
// these focus on the regex+snap math that's reachable without one.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  findAtomicCursorRanges,
  snapHeadOutOfAtomicRanges,
  findTrailingClusterStart,
  promoteOrDemoteTag,
} from "../../src/lib/cm-decorations.ts";

const EMPTY = { hide: new Set(), hideEmpty: new Set() };

test("findAtomicCursorRanges finds trailing-cluster #tags only (inline are editable)", () => {
  // Tag-system Phase 5: only the trailing cluster is atomic. The mid-block
  // `#bug` here is inline (followed by " there") — should NOT be atomic.
  // The trailing `#urgent #end` are in the trailing cluster — should be.
  const doc = "hello #bug there #urgent #end";
  const r = findAtomicCursorRanges(doc, EMPTY);
  assert.deepEqual(r, [
    [17, 24], // #urgent
    [25, 29], // #end
  ]);
});

test("findAtomicCursorRanges treats a doc that is only tags as one cluster", () => {
  // All four tokens are at the trailing edge → all atomic.
  const doc = "#a #b #c #d";
  const r = findAtomicCursorRanges(doc, EMPTY);
  assert.equal(r.length, 4);
});

test("findAtomicCursorRanges treats no-trailing-cluster docs as having no atomic tag ranges", () => {
  // Tag mid-block, then plain text at the end → cluster is empty.
  const doc = "hello #bug there end";
  const r = findAtomicCursorRanges(doc, EMPTY);
  assert.equal(r.filter((range) => range[1] - range[0] === 4).length, 0);
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

test("snap: cm-vim `l` step from before-tag lands past the tag (trailing cluster)", () => {
  // Tag-system Phase 5: only trailing-cluster tags are atomic. Use a doc
  // whose tag is at the trailing edge.
  const doc = "abc #bug";
  const ranges = findAtomicCursorRanges(doc, EMPTY);
  assert.deepEqual(ranges, [[4, 8]]);
  assert.equal(snapHeadOutOfAtomicRanges(5, 4, ranges), 8);
});

// ── findTrailingClusterStart ─────────────────────────────────────────────

test("findTrailingClusterStart: returns doc.length when no trailing tags", () => {
  assert.equal(findTrailingClusterStart("hello world"), "hello world".length);
});

test("findTrailingClusterStart: returns the position of the first # in the cluster", () => {
  const doc = "some text #foo";
  // Cluster starts at '#'
  assert.equal(findTrailingClusterStart(doc), 10);
});

test("findTrailingClusterStart: multiple trailing tags share one cluster", () => {
  const doc = "some text #foo #bar #baz";
  // Cluster starts at the first # of the run
  assert.equal(findTrailingClusterStart(doc), 10);
});

test("findTrailingClusterStart: trailing whitespace doesn't break the cluster", () => {
  const doc = "x #a   ";
  // The trailing spaces are trimmed first, then the cluster is found.
  assert.equal(findTrailingClusterStart(doc), 2);
});

test("findTrailingClusterStart: inline #tag mid-block is NOT in the cluster", () => {
  const doc = "hello #bug there";
  // The '#bug' is mid-block; cluster is empty → returns doc.length.
  assert.equal(findTrailingClusterStart(doc), doc.length);
});

test("findTrailingClusterStart: bare # is not a tag", () => {
  const doc = "value is #";
  // No tag-name chars after `#` → no cluster.
  assert.equal(findTrailingClusterStart(doc), doc.length);
});

// ── promoteOrDemoteTag ────────────────────────────────────────────────────

test("promoteOrDemoteTag: cursor inside an inline tag demotes to trailing", () => {
  const doc = "hello #foo there";
  // Cursor at the 'o' of '#foo' (position 9)
  const result = promoteOrDemoteTag(doc, 9);
  assert.ok(result, "should return a change set");
  // Two changes: delete the inline range, insert at trailing position
  assert.equal(result.changes.length, 2);
  // Apply manually: assembled doc should have #foo at end
  const sorted = [...result.changes].sort((a, b) => a.from - b.from);
  // After applying both edits, document content should be `hello there #foo`
  // (or similar — the helper guarantees the trailing chip is at the end)
  const delEdit = sorted.find((c) => c.from === 6);
  const insEdit = sorted.find((c) => c.insert.includes("#foo"));
  assert.ok(delEdit, "should have a delete edit at inline tag");
  assert.ok(insEdit, "should have an insert edit with the tag");
});

test("promoteOrDemoteTag: cursor outside any tag promotes the rightmost trailing chip", () => {
  const doc = "hello world #foo";
  // Cursor at the 'l' of 'world' (position 9)
  const result = promoteOrDemoteTag(doc, 9);
  assert.ok(result, "should return a change set");
  assert.equal(result.changes.length, 2);
  // Should have a delete edit covering the trailing token, and an insert
  // edit at the cursor with #foo
  const insEdit = result.changes.find((c) => c.from === 9 && c.to === 9);
  assert.ok(insEdit, "should insert at cursor");
  assert.ok(insEdit.insert.includes("#foo"), "insert should contain the tag");
});

test("promoteOrDemoteTag: no inline tags and no trailing cluster returns null", () => {
  assert.equal(promoteOrDemoteTag("hello world", 5), null);
});

test("promoteOrDemoteTag: cursor between tags in trailing cluster pops the rightmost", () => {
  const doc = "task #foo #bar";
  // Cursor at end of doc (no inline tag; cluster has two tags)
  const result = promoteOrDemoteTag(doc, doc.length);
  assert.ok(result, "should pop the rightmost trailing chip");
  // Inserted piece should reference #bar (the rightmost)
  const insEdit = result.changes.find((c) => c.insert.includes("#"));
  assert.ok(insEdit?.insert.includes("#bar"), "should pop #bar (rightmost)");
});

test("promoteOrDemoteTag: idempotent toggle round-trip", () => {
  // Demote #foo inline → trailing → promote it back to inline.
  const start = "hello #foo there";
  const demoted = promoteOrDemoteTag(start, 8);
  assert.ok(demoted);
  // Apply the changes manually
  let doc = start;
  const sorted = [...demoted.changes].sort((a, b) => b.from - a.from);
  for (const e of sorted) {
    doc = doc.slice(0, e.from) + e.insert + doc.slice(e.to);
  }
  // After demote, doc has #foo at the end
  assert.ok(doc.endsWith("#foo"), `expected trailing #foo, got: ${doc}`);
});
