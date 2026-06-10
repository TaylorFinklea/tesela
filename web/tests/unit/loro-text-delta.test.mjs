import { test } from "node:test";
import assert from "node:assert/strict";
import { deltaToChanges } from "../../src/lib/loro/text-delta.ts";

// The coordinate contract under test: CM6 interprets every from/to in a
// multi-change dispatch relative to the ORIGINAL (pre-transaction) document,
// so the mapper must advance its position counter on retain AND delete (both
// consume original-doc length) and NOT on insert (which consumes none). The
// pre-fix BlockEditor mapping inverted insert/delete and misapplied every
// multi-run delta (remote Alt-Enter tag demotes, coalesced same-frame
// splices, offline catch-up merges).

/** Reference applier: CM6 multi-change semantics — all ranges address the
 *  original doc; equivalent to sorting by `from` and applying back-to-front. */
function applyChanges(doc, changes) {
  const sorted = [...changes].sort((a, b) => b.from - a.from || b.to - a.to);
  let out = doc;
  for (const c of sorted) {
    out = out.slice(0, c.from) + c.insert + out.slice(c.to);
  }
  return out;
}

/** Reference quill-delta applier (sequential, in mutated coordinates). */
function applyDelta(doc, delta) {
  let pos = 0;
  let out = doc;
  for (const op of delta) {
    if (typeof op.retain === "number") pos += op.retain;
    else if (typeof op.insert === "string") {
      out = out.slice(0, pos) + op.insert + out.slice(pos);
      pos += op.insert.length;
    } else if (typeof op.delete === "number") {
      out = out.slice(0, pos) + out.slice(pos + op.delete);
    }
  }
  return out;
}

test("single insert run", () => {
  assert.deepEqual(deltaToChanges([{ retain: 3 }, { insert: "xy" }]), [
    { from: 3, to: 3, insert: "xy" },
  ]);
});

test("single delete run", () => {
  assert.deepEqual(deltaToChanges([{ retain: 2 }, { delete: 4 }]), [
    { from: 2, to: 6, insert: "" },
  ]);
});

test("delete advances the original-doc position (audit worked example)", () => {
  // [retain 2, insert "ab", retain 3, delete 1]: the delete's original-doc
  // span is 5..6 — the pre-fix code emitted 7..8 (advanced on the insert,
  // not on the delete).
  const changes = deltaToChanges([
    { retain: 2 },
    { insert: "ab" },
    { retain: 3 },
    { delete: 1 },
  ]);
  assert.deepEqual(changes, [
    { from: 2, to: 2, insert: "ab" },
    { from: 5, to: 6, insert: "" },
  ]);
});

test("Alt-Enter tag-demote delta (delete then later insert)", () => {
  // "see #foo bar" → "see bar #foo": delete 4..9, insert " #foo" at 12.
  // The pre-fix mapping emitted the insert at 7 — INSIDE the delete range.
  const doc = "see #foo bar";
  const delta = [{ retain: 4 }, { delete: 5 }, { retain: 3 }, { insert: " #foo" }];
  const changes = deltaToChanges(delta);
  assert.deepEqual(changes, [
    { from: 4, to: 9, insert: "" },
    { from: 12, to: 12, insert: " #foo" },
  ]);
  assert.equal(applyChanges(doc, changes), "see bar #foo");
  assert.equal(applyChanges(doc, changes), applyDelta(doc, delta));
});

test("multi-run deltas converge with the sequential quill application", () => {
  const doc = "the quick brown fox jumps";
  const deltas = [
    [{ insert: "A" }, { retain: 3 }, { delete: 1 }, { retain: 5 }, { insert: "zz" }],
    [{ delete: 4 }, { insert: "THE " }, { retain: 6 }, { delete: 6 }],
    [{ retain: 10 }, { insert: "🦊" }, { retain: 4 }, { delete: 3 }, { insert: "x" }],
    [{ delete: 25 }, { insert: "replaced" }],
  ];
  for (const delta of deltas) {
    const changes = deltaToChanges(delta);
    assert.equal(
      applyChanges(doc, changes),
      applyDelta(doc, delta),
      `diverged for ${JSON.stringify(delta)}`,
    );
  }
});

test("retain-only / empty deltas produce no changes", () => {
  assert.deepEqual(deltaToChanges([]), []);
  assert.deepEqual(deltaToChanges([{ retain: 7 }]), []);
});
