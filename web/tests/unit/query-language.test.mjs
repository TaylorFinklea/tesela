// Unit tests for the TS query-language helpers.
//
// Mirrors `crates/tesela-core/src/query.rs` tests. Specifically covers the
// Phase 16 `pagetag:` and `blocktag:` extensions to verify the TS impl
// stays in sync with the Rust impl.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  parseQuery,
  blockMatches,
} from "../../src/lib/query-language.ts";

function block({ tags = [], inherited = [], properties = {} } = {}) {
  return {
    id: "n:1",
    text: "x",
    raw_text: "- x",
    tags,
    inline_tags: [],
    trailing_tags: [],
    inherited_tags: inherited,
    properties,
    indent_level: 0,
    note_id: "n",
  };
}

test("tag: matches direct block tag", () => {
  const q = parseQuery("tag:Task");
  assert.ok(blockMatches(block({ tags: ["Task"] }), q));
  assert.ok(!blockMatches(block({ tags: ["Other"] }), q));
});

test("tag: matches via inherited chain (inherited from ancestor)", () => {
  const q = parseQuery("tag:Project");
  assert.ok(blockMatches(block({ inherited: ["Project"] }), q));
});

test("pagetag: aliases tag: at the matching level (includes inherited)", () => {
  const q = parseQuery("pagetag:Task");
  assert.ok(blockMatches(block({ tags: ["Task"] }), q));
  assert.ok(blockMatches(block({ inherited: ["Task"] }), q));
});

test("blocktag: matches only direct block tags (excludes inherited)", () => {
  const q = parseQuery("blocktag:Task");
  assert.ok(blockMatches(block({ tags: ["Task"] }), q));
  assert.ok(!blockMatches(block({ inherited: ["Task"] }), q));
});

test("blocktag: negation flips for inherited-only block", () => {
  const q = parseQuery("-blocktag:Done");
  // Direct block tag is Done → -blocktag:Done false
  assert.ok(!blockMatches(block({ tags: ["Done"] }), q));
  // Done in inherited only → -blocktag:Done true (no literal block-level Done)
  assert.ok(blockMatches(block({ tags: ["Task"], inherited: ["Done"] }), q));
});

test("pagetag: negation matches absent tag", () => {
  const q = parseQuery("-pagetag:Done");
  assert.ok(blockMatches(block({ tags: ["Task"] }), q));
  assert.ok(!blockMatches(block({ tags: ["Done"] }), q));
});

test("comparison operators are not meaningful for tag/pagetag/blocktag", () => {
  // `tag:>=Task` is nonsensical — should return false rather than
  // throwing.
  const q = parseQuery("tag:>=Task");
  assert.ok(!blockMatches(block({ tags: ["Task"] }), q));
});

test("case-insensitive match across all three keys", () => {
  for (const key of ["tag", "pagetag", "blocktag"]) {
    const q = parseQuery(`${key}:TASK`);
    assert.ok(
      blockMatches(block({ tags: ["task"] }), q),
      `${key} should match case-insensitively`,
    );
  }
});
