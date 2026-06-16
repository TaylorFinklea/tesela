import { test } from "node:test";
import assert from "node:assert/strict";
import { planEnterSplit } from "../../src/lib/editor/enter-split.ts";

/**
 * Helper: locate the cursor at the END of a given line within a multi-line
 * doc and return { doc, cursor, lineText } the way the editor would compute
 * them (absolute offset + the cursor's own line text).
 */
function atEndOfLine(doc, lineText) {
  const idx = doc.indexOf(lineText);
  assert.notEqual(idx, -1, `line ${JSON.stringify(lineText)} not in doc`);
  return { doc, cursor: idx + lineText.length, lineText };
}

test("THE BUG: Enter on a task's property line keeps the whole block + empty sibling", () => {
  // A task block: prose, then a user-typed custom property, then `tags:: Task`
  // BELOW it (the make-task line). Cursor at the end of `testpoints:: 10`.
  // Pre-fix this shipped `\ntags:: Task` to the new block, orphaning the
  // original. Now the block stays intact and the new block is empty.
  const doc = "ship the big feature [[dude]]\ntestpoints:: 10\ntags:: Task";
  const { cursor, lineText } = atEndOfLine(doc, "testpoints:: 10");
  const plan = planEnterSplit(doc, cursor, lineText);
  assert.equal(plan.current, null, "current block must be left untouched");
  assert.equal(plan.next, "", "new block must be empty");
});

test("bug, other ordering: `tags:: Task` above the custom property", () => {
  const doc = "ship the big feature\ntags:: Task\ntestpoints:: 10";
  // Cursor at end of the `tags:: Task` line (which has `testpoints:: 10` below).
  const { cursor, lineText } = atEndOfLine(doc, "tags:: Task");
  const plan = planEnterSplit(doc, cursor, lineText);
  assert.equal(plan.current, null);
  assert.equal(plan.next, "");
});

test("(a) Enter at end of a single-line plain block → empty sibling, no change", () => {
  const doc = "just some text";
  const plan = planEnterSplit(doc, doc.length, doc);
  assert.equal(plan.current, null);
  assert.equal(plan.next, "");
});

test("(a') Enter mid-text of a single-line block → splits the prose", () => {
  const doc = "hello world";
  const cursor = "hello".length; // between "hello" and " world"
  const plan = planEnterSplit(doc, cursor, doc);
  assert.equal(plan.current, "hello");
  assert.equal(plan.next, " world");
});

test("(b) first-line guard: Enter on the prose line keeps continuation lines", () => {
  // Cursor at end of the prose line; the `tags:: Task` continuation must stay.
  const doc = "ship it\ntags:: Task\ntestpoints:: 10";
  const cursor = "ship it".length;
  const plan = planEnterSplit(doc, cursor, "ship it");
  assert.equal(plan.current, "ship it\ntags:: Task\ntestpoints:: 10");
  assert.equal(plan.next, "");
});

test("(b') first-line guard mid-prose: prose splits, continuation stays with current", () => {
  const doc = "ship the feature\ntags:: Task";
  const cursor = "ship the".length; // split "ship the| feature"
  const plan = planEnterSplit(doc, cursor, "ship the feature");
  assert.equal(plan.current, "ship the\ntags:: Task");
  assert.equal(plan.next, " feature");
});

test("(c) Enter in genuine multi-line PROSE still splits at the cursor", () => {
  // No property line involved — multi-line prose body splits normally.
  const doc = "first prose line\nsecond prose line";
  const { cursor } = atEndOfLine(doc, "second prose line");
  const plan = planEnterSplit(doc, cursor, "second prose line");
  // cursor at very end → nothing after → empty sibling, no change.
  assert.equal(plan.current, null);
  assert.equal(plan.next, "");
});

test("(c') Enter mid second prose line splits that line, keeps the first", () => {
  const doc = "first prose line\nsecond prose line";
  const cursor = doc.indexOf("second") + "second".length; // "second| prose line"
  const plan = planEnterSplit(doc, cursor, "second prose line");
  assert.equal(plan.current, "first prose line\nsecond");
  assert.equal(plan.next, " prose line");
});
