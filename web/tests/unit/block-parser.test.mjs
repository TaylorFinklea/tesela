// Unit tests for the block-parser pure helpers.
//
// `splitInlineAndTrailingTags` mirrors the Rust implementation in
// `crates/tesela-core/src/block.rs:split_inline_and_trailing_tags`. The
// behavior must stay in sync — both implementations are exercised against
// the same set of test cases here (TS side) and in `block.rs` (Rust side).

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { splitInlineAndTrailingTags } from "../../src/lib/block-parser.ts";

test("split: no tags → empty arrays", () => {
  const r = splitInlineAndTrailingTags("just text");
  assert.deepEqual(r, { inline: [], trailing: [] });
});

test("split: pure inline tag mid-block", () => {
  const r = splitInlineAndTrailingTags("see #foo here");
  assert.deepEqual(r.inline, ["foo"]);
  assert.deepEqual(r.trailing, []);
});

test("split: pure trailing tag at end", () => {
  const r = splitInlineAndTrailingTags("task name #important");
  assert.deepEqual(r.inline, []);
  assert.deepEqual(r.trailing, ["important"]);
});

test("split: multiple trailing tags share one cluster", () => {
  const r = splitInlineAndTrailingTags("task #foo #bar #baz");
  assert.deepEqual(r.inline, []);
  assert.deepEqual(r.trailing, ["foo", "bar", "baz"]);
});

test("split: inline + trailing", () => {
  const r = splitInlineAndTrailingTags("see #foo here #bar");
  assert.deepEqual(r.inline, ["foo"]);
  assert.deepEqual(r.trailing, ["bar"]);
});

test("split: trailing whitespace doesn't break the cluster", () => {
  const r = splitInlineAndTrailingTags("x #a   ");
  assert.deepEqual(r.trailing, ["a"]);
});

test("split: cluster halts at first non-tag, non-whitespace char", () => {
  // "x #a y #b" — `#a` is inline (followed by " y"), `#b` is trailing.
  const r = splitInlineAndTrailingTags("x #a y #b");
  assert.deepEqual(r.inline, ["a"]);
  assert.deepEqual(r.trailing, ["b"]);
});

test("split: bare # is not a tag", () => {
  // "value is #" — no tag-name chars after `#`.
  const r = splitInlineAndTrailingTags("value is #");
  assert.deepEqual(r, { inline: [], trailing: [] });
});

test("split: tag with hyphens and slashes (path-form)", () => {
  const r = splitInlineAndTrailingTags("task #nature/birds/cardinal");
  assert.deepEqual(r.trailing, ["nature/birds/cardinal"]);
});

test("split: bullet line with trailing chip", () => {
  // The full raw block content includes the leading `- ` bullet.
  const r = splitInlineAndTrailingTags("- write tests #urgent");
  assert.deepEqual(r.inline, []);
  assert.deepEqual(r.trailing, ["urgent"]);
});

test("split: matches Rust impl on the canonical edge case (multiple cluster tokens with newlines)", () => {
  // The Rust impl treats horizontal whitespace AND newlines as cluster
  // separators. So "- a\n#tag1\n#tag2" → trailing cluster has both.
  const r = splitInlineAndTrailingTags("- a\n#tag1\n#tag2");
  assert.deepEqual(r.trailing, ["tag1", "tag2"]);
});

test("split: same #tag both inline and trailing yields both entries", () => {
  const r = splitInlineAndTrailingTags("#foo bar #foo");
  assert.deepEqual(r.inline, ["foo"]);
  assert.deepEqual(r.trailing, ["foo"]);
});
