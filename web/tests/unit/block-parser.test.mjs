// Unit tests for the block-parser pure helpers.
//
// `splitInlineAndTrailingTags` mirrors the Rust implementation in
// `crates/tesela-core/src/block.rs:split_inline_and_trailing_tags`. The
// behavior must stay in sync — both implementations are exercised against
// the same set of test cases here (TS side) and in `block.rs` (Rust side).

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  blockDisplayText,
  parseBlocks,
  renderBlockBody,
  segmentText,
  splitInlineAndTrailingTags,
} from "../../src/lib/block-parser.ts";

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

test("parseBlocks strips the owning rightmost bid and preserves earlier literal content", () => {
  const bid = "6ae83fc1-9ee9-4626-9efe-58e0d83e7176";
  const blocks = parseBlocks(
    "2026-06-11",
    `- Figure <!-- bid:${bid} --> out finances <!-- bid:${bid} -->\n  tags:: Issue`,
  );

  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].bid, bid);
  assert.equal(blocks[0].text, `Figure <!-- bid:${bid} --> out finances`);
  assert.equal(blocks[0].raw_text, `Figure <!-- bid:${bid} --> out finances\ntags:: Issue`);
  assert.deepEqual(blocks[0].tags, ["Issue"]);
});

test("tab-prefixed Markdown list continuations remain inside their owning block", () => {
  const bid = "e07cb4d8-58b2-4a13-a981-cf0e8be88e15";
  const body = [
    `  - - Pi <!-- bid:${bid} -->`,
    "    \t- pi install npm:@quintinshaw/pi-dynamic-workflows",
    "    \t- /trust",
  ].join("\n");

  const blocks = parseBlocks("2026-07-14", body);

  assert.equal(blocks.length, 1);
  assert.equal(blocks[0].bid, bid);
  assert.equal(
    blocks[0].raw_text,
    "- Pi\n\t- pi install npm:@quintinshaw/pi-dynamic-workflows\n\t- /trust",
  );
  assert.equal(renderBlockBody(blocks), body);
});

test("canonical lifted heading, prose, and fence display and edit round-trip", () => {
  const headingBid = "11111111-1111-1111-1111-111111111111";
  const proseBid = "22222222-2222-2222-2222-222222222222";
  const fenceBid = "33333333-3333-3333-3333-333333333333";
  const literalBid = "44444444-4444-4444-4444-444444444444";
  const body = [
    `- # Heading <!-- bid:${headingBid} -->`,
    `- Prose one <!-- bid:${proseBid} -->`,
    "  prose two",
    `- <!-- bid:${fenceBid} -->`,
    "  ```query",
    "  status:: done",
    "  #literal",
    "  - payload, not a block",
    "  ",
    "    extra-indented payload  ",
    `  <!-- bid:${literalBid} -->`,
    "  ```",
  ].join("\n");

  const blocks = parseBlocks("mixed", body);
  assert.equal(blocks.length, 3);
  assert.deepEqual(
    blocks.map((block) => block.raw_text),
    [
      "# Heading",
      "Prose one\nprose two",
      `\`\`\`query\nstatus:: done\n#literal\n- payload, not a block\n\n  extra-indented payload  \n<!-- bid:${literalBid} -->\n\`\`\``,
    ],
  );
  assert.deepEqual(blocks.map((block) => block.bid), [headingBid, proseBid, fenceBid]);
  assert.equal(blocks[2].text, "```query");
  assert.deepEqual(blocks[2].properties, {});
  assert.deepEqual(blocks[2].tags, []);
  assert.equal(blocks[2].raw_text.includes(fenceBid), false, "owned bid stays out of editor text");
  assert.equal(blocks[2].raw_text.includes(literalBid), true, "fenced bid-shaped payload stays literal");
  assert.deepEqual(segmentText(blocks[2].raw_text), [
    {
      type: "code",
      lang: "query",
      value: `status:: done\n#literal\n- payload, not a block\n\n  extra-indented payload  \n<!-- bid:${literalBid} -->`,
    },
  ]);

  blocks[0].raw_text = "# Heading edited";
  blocks[0].text = "# Heading edited";
  blocks[1].raw_text = "Prose one\nprose two edited";
  blocks[2].raw_text = blocks[2].raw_text.replace("status:: done", "status:: todo");
  const rendered = renderBlockBody(blocks);
  const reparsed = parseBlocks("mixed", rendered);

  assert.equal(reparsed.length, 3);
  assert.deepEqual(reparsed.map((block) => block.bid), [headingBid, proseBid, fenceBid]);
  assert.deepEqual(
    reparsed.map((block) => block.raw_text),
    [
      "# Heading edited",
      "Prose one\nprose two edited",
      `\`\`\`query\nstatus:: todo\n#literal\n- payload, not a block\n\n  extra-indented payload  \n<!-- bid:${literalBid} -->\n\`\`\``,
    ],
  );
  assert.equal(rendered.includes(`- <!-- bid:${fenceBid} -->\n  \`\`\`query`), true);
});

test("segmentText renders fenced code as a literal code segment", () => {
  const segments = segmentText("before [[Page]]\n```ts\nconst tag = '#not-a-tag';\n[[literal]]\n```\nafter");

  assert.deepEqual(segments, [
    { type: "text", value: "before " },
    { type: "link", value: "Page", href: "/p/page" },
    { type: "text", value: "\n" },
    { type: "code", lang: "ts", value: "const tag = '#not-a-tag';\n[[literal]]" },
    { type: "text", value: "\nafter" },
  ]);
});

test("segmentText treats an unclosed fenced code block as literal code to the end", () => {
  const segments = segmentText("```bash\necho [[not-a-link]] #not-a-tag");

  assert.deepEqual(segments, [
    { type: "code", lang: "bash", value: "echo [[not-a-link]] #not-a-tag" },
  ]);
});

test("blockDisplayText keeps fenced code but omits property lines outside fences", () => {
  const block = {
    text: "Example",
    raw_text: "Example #visible\n```js\nconst x = 1;\n```\nstatus:: todo\ndeadline:: [[2026-06-12]]",
  };

  assert.equal(blockDisplayText(block), "Example\n```js\nconst x = 1;\n```");
});

test("blockDisplayText preserves existing first-line display for blocks without fences", () => {
  const block = {
    text: "Example",
    raw_text: "Example\nstatus:: todo",
  };

  assert.equal(blockDisplayText(block), "Example");
});

test("parseBlocks does not extract tags or properties from fenced code", () => {
  const blocks = parseBlocks("note", "- Example #visible\n  ```js\n  const tag = '#hidden';\n  status:: done\n  ```\n  status:: todo");

  assert.deepEqual(blocks[0].tags, ["visible"]);
  assert.deepEqual(blocks[0].inline_tags, ["visible"]);
  assert.deepEqual(blocks[0].properties, { status: "todo" });
});

test("parseBlocks leaves malformed property-looking first-line text visible", () => {
  const blocks = parseBlocks("note", "- Deadline::cheduled::");

  assert.equal(blocks[0].text, "Deadline::cheduled::");
  assert.equal(blocks[0].raw_text, "Deadline::cheduled::");
  assert.deepEqual(blocks[0].properties, {});
});

test("parseBlocks does not convert malformed property-looking continuation lines into properties", () => {
  const blocks = parseBlocks("note", "- Task #Task\n  Deadline::cheduled::\n  status:: todo");

  assert.equal(blocks[0].raw_text, "Task #Task\nDeadline::cheduled::\nstatus:: todo");
  assert.deepEqual(blocks[0].tags, ["Task"]);
  assert.deepEqual(blocks[0].properties, { status: "todo" });
});
