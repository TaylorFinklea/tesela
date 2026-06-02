// Unit tests for JournalView's trailing-empty-bullet decision/builder.
//
// These pure helpers decide whether to append a focusable trailing empty
// bullet to a daily note's body, and build the bid-stamped append. The
// CRITICAL INVARIANT: the editing surface always has EXACTLY ONE focusable
// trailing empty — no accretion (a stranded mid-body empty must NOT trigger a
// new append), but a body with zero empties anywhere MUST get exactly one.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  bodyHasTrailingEmpty,
  appendTrailingEmpty,
} from "../../src/lib/ensure-trailing-empty.ts";

// --- bodyHasTrailingEmpty: the any-empty scan -------------------------------

test("empty body has no trailing empty", () => {
  assert.equal(bodyHasTrailingEmpty(""), false);
});

test("body ending in content (no empty anywhere) → false (must append)", () => {
  assert.equal(bodyHasTrailingEmpty("- hello\n- world"), false);
});

test("body whose LAST line is an empty bullet → true (no append)", () => {
  assert.equal(bodyHasTrailingEmpty("- hello\n- "), true);
  assert.equal(bodyHasTrailingEmpty("- hello\n-"), true);
});

test("indented trailing empty bullet → true", () => {
  assert.equal(bodyHasTrailingEmpty("- hello\n  - "), true);
});

test("STRANDED mid-body empty (last line is content) → true (no accretion)", () => {
  // The engine appended a new end node ("dude") AFTER a previously-trailing
  // empty, stranding the empty mid-body. A last-line-only check would miss it
  // and accrete; the scan finds it and suppresses.
  assert.equal(bodyHasTrailingEmpty("- hello\n- \n- dude"), true);
});

test("bid-stamped empty bullet still recognized (round-trip on next mount)", () => {
  // appendTrailingEmpty emits `- <!-- bid:UUID -->`; on the next mount the
  // scan MUST recognize it as an existing empty (bid-strip → `- ` matches),
  // else accretion returns.
  const stamped = "- hello\n- <!-- bid:11111111-2222-3333-4444-555555555555 -->";
  assert.equal(bodyHasTrailingEmpty(stamped), true);
});

test("bid-stamped empty stranded mid-body → true", () => {
  const stamped =
    "- hello\n- <!-- bid:11111111-2222-3333-4444-555555555555 -->\n- dude";
  assert.equal(bodyHasTrailingEmpty(stamped), true);
});

test("a bullet with text + a bid marker is NOT an empty", () => {
  assert.equal(
    bodyHasTrailingEmpty(
      "- dude <!-- bid:11111111-2222-3333-4444-555555555555 -->",
    ),
    false,
  );
});

// --- appendTrailingEmpty: the bid-stamped emit ------------------------------

const BID = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";

test("appends a bid-stamped empty bullet (not a bare `- `)", () => {
  const content = "- hello";
  const out = appendTrailingEmpty(content, "- hello", BID);
  assert.equal(out, `- hello\n- <!-- bid:${BID} -->\n`);
});

test("appended empty round-trips through the scan (no accretion next mount)", () => {
  const out = appendTrailingEmpty("- hello", "- hello", BID);
  // Mimic the next mount: trim trailing newlines like the component does.
  const nextBody = out.replace(/\n+$/, "");
  assert.equal(bodyHasTrailingEmpty(nextBody), true);
});

test("preserves front-matter: empty is appended AFTER the body, not into FM", () => {
  const content = "---\ntitle: 2026-06-02\n---\n- hello";
  const body = "- hello";
  const out = appendTrailingEmpty(content, body, BID);
  assert.equal(
    out,
    `---\ntitle: 2026-06-02\n---\n- hello\n- <!-- bid:${BID} -->\n`,
  );
});

test("empty body → single trailing empty (no leading newline)", () => {
  const out = appendTrailingEmpty("", "", BID);
  assert.equal(out, `- <!-- bid:${BID} -->\n`);
});

// --- The two invariant states the task calls out ---------------------------

test("INVARIANT (a): content WITH a stranded mid-body empty → NO new empty", () => {
  const body = "- hello\n- \n- dude";
  assert.equal(bodyHasTrailingEmpty(body), true); // → ensureTrailingEmpty returns false
});

test("INVARIANT (b): content with ZERO empties → exactly ONE new empty", () => {
  const body = "- hello\n- dude";
  assert.equal(bodyHasTrailingEmpty(body), false); // → must append
  const out = appendTrailingEmpty("- hello\n- dude", body, BID);
  // Exactly one empty bullet line was added at the end.
  const emptyLines = out
    .split("\n")
    .filter((l) => /^\s*-\s*(<!--\s*bid:[^>]*-->)?\s*$/.test(l));
  assert.equal(emptyLines.length, 1);
  assert.equal(out, `- hello\n- dude\n- <!-- bid:${BID} -->\n`);
});
