// Unit tests for the JournalView placeholder preview helper.
//
// The placeholder is rendered for OFF-SCREEN journal sections (no
// CodeMirror mounted) so a large imported graph stays cheap to render.
// Today it shows raw body lines — bid comments, property continuations
// like `status:: done` and `tags:: Task`, and malformed metadata — which
// clutters the preview with structural noise. This helper extracts the
// actual block text the user typed, ignoring structural markers.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { previewLines } from "../../src/lib/journal-preview.ts";

// --- bid stamp stripping ----------------------------------------------------

test("strips bid comment from the same line as bullet text", () => {
  const body = "- hello <!-- bid:019ebda4-d6ad-76b0-a968-1759681734b7 -->";
  assert.deepEqual(previewLines(body), [{ text: "hello", indent: 0 }]);
});

test("strips bid comment with extra whitespace and various UUID lengths", () => {
  // Mirrors the block-parser's BID_COMMENT_RE (lowercase `bid:` only).
  // 32-char and 36-char hex UUIDs are both accepted by the parser.
  const uuid36 = "019ebda4-d6ad-76b0-a968-1759681734b7";
  const uuid32 = "019ebda4d6ad76b0a9681759681734b7";
  const cases = [
    `<!--bid:${uuid36}-->`,
    `<!--  bid:${uuid36}  -->`,
    `<!-- bid:${uuid32} -->`,
  ];
  for (const stamp of cases) {
    const body = `- text ${stamp}`;
    assert.deepEqual(previewLines(body), [{ text: "text", indent: 0 }], stamp);
  }
});

test("strips bid stamp from a standalone empty bullet", () => {
  const body = "- <!-- bid:019ebda4-d6ad-76b0-a968-1759681734b7 -->";
  // Empty bullet: no user text. Should yield nothing (not an empty entry).
  assert.deepEqual(previewLines(body), []);
});

// --- property-only continuation lines ---------------------------------------

test("skips property-only continuation lines (tags::, status::, etc.)", () => {
  const body = [
    "- //this is a task /",
    "  status:: done",
    "  tags:: Task",
    "- nice",
  ].join("\n");
  assert.deepEqual(previewLines(body), [
    { text: "//this is a task /", indent: 0 },
    { text: "nice", indent: 0 },
  ]);
});

test("skips ALL of the typical apple-reminders / system property lines", () => {
  const body = [
    "- dude",
    "  apple_reminder_id:: 8F01C85C-2E7B-4CF2-9319-398B04D146BF",
    "  apple_reminder_synced_at:: 2026-05-23T11:46:50.622565+00:00",
    "  priority:: high",
    "  deadline:: [[2026-05-08]] 10:00",
    "- next",
  ].join("\n");
  assert.deepEqual(previewLines(body), [
    { text: "dude", indent: 0 },
    { text: "next", indent: 0 },
  ]);
});

test("does NOT treat a property-shaped string as a property when it's the block text", () => {
  // If the user TYPED `key:: value` as their block text (not as a continuation
  // line), the literal text must still appear in the preview — we are
  // stripping structural metadata, not silencing `::` content.
  const body = "- key:: value";
  assert.deepEqual(previewLines(body), [{ text: "key:: value", indent: 0 }]);
});

test("does NOT touch #hashtag-shaped block text", () => {
  const body = "- fix the #bug tomorrow";
  assert.deepEqual(previewLines(body), [{ text: "fix the #bug tomorrow", indent: 0 }]);
});

test("does NOT touch [[wiki-link]]-shaped block text", () => {
  const body = "- look at [[Phase3GQA]] next";
  assert.deepEqual(previewLines(body), [{ text: "look at [[Phase3GQA]] next", indent: 0 }]);
});

// --- malformed metadata-looking continuation lines --------------------------

test("skips malformed metadata-looking continuation lines (broken :: cases)", () => {
  // These look like property continuations but are obviously corrupted —
  // missing space, missing key, missing value. None are valid `key:: value`
  // continuations and none should ever surface in the preview.
  const body = [
    "- real block",
    "  Deadline::cheduled::",
    "  ::no-key",
    "  priority: high",
    "- next real",
  ].join("\n");
  assert.deepEqual(previewLines(body), [
    { text: "real block", indent: 0 },
    { text: "next real", indent: 0 },
  ]);
});

// --- empty / blank body -----------------------------------------------------

test("empty body yields no preview lines", () => {
  assert.deepEqual(previewLines(""), []);
});

test("body with only bid stamps yields no preview lines", () => {
  const body = "- <!-- bid:019ebda4-d6ad-76b0-a968-1759681734b7 -->";
  assert.deepEqual(previewLines(body), []);
});

test("body of only blank lines and bid stamps yields no preview lines", () => {
  const body = [
    "",
    "  ",
    "- <!-- bid:019ebda4-d6ad-76b0-a968-1759681734b7 -->",
    "  tags:: Task",
    "",
  ].join("\n");
  assert.deepEqual(previewLines(body), []);
});

// --- maxLines cap -----------------------------------------------------------

test("respects maxLines cap (default 6)", () => {
  const body = Array.from({ length: 10 }, (_, i) => `- line ${i}`).join("\n");
  const out = previewLines(body);
  assert.equal(out.length, 6);
  assert.deepEqual(
    out.map((l) => l.text),
    ["line 0", "line 1", "line 2", "line 3", "line 4", "line 5"],
  );
});

test("maxLines=0 yields no lines (does not throw)", () => {
  assert.deepEqual(previewLines("- hello\n- world", { maxLines: 0 }), []);
});

// --- indent preservation ----------------------------------------------------

test("preserves leading indent of bullet lines", () => {
  const body = [
    "- top",
    "  - child A",
    "  - child B",
    "- sibling",
  ].join("\n");
  assert.deepEqual(previewLines(body), [
    { text: "top", indent: 0 },
    { text: "child A", indent: 2 },
    { text: "child B", indent: 2 },
    { text: "sibling", indent: 0 },
  ]);
});

test("indented child bullet with property continuation still shows text", () => {
  const body = [
    "- parent",
    "  - kid",
    "    status:: done",
  ].join("\n");
  assert.deepEqual(previewLines(body), [
    { text: "parent", indent: 0 },
    { text: "kid", indent: 2 },
  ]);
});

// --- CRLF / trailing whitespace --------------------------------------------

test("handles CRLF line endings and trailing whitespace", () => {
  const body = "- hello   \r\n- world\t  \r\n";
  assert.deepEqual(previewLines(body), [
    { text: "hello", indent: 0 },
    { text: "world", indent: 0 },
  ]);
});

// --- placeholder function shape --------------------------------------------

test("return shape is stable: { text, indent } objects, not strings", () => {
  const out = previewLines("- hello");
  assert.equal(out.length, 1);
  assert.equal(typeof out[0].text, "string");
  assert.equal(typeof out[0].indent, "number");
});
