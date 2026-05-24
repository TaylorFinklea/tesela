import { test } from "node:test";
import assert from "node:assert/strict";
import { setBlockProperty, setBlockText, deleteBlock } from "../../src/lib/block-mutations.ts";

// Realistic daily-note shape: closing frontmatter fence, blank line, then
// bullets. This is the on-disk form for ~every daily/notes file the user
// authors, including the file the `x` regression was reported against.
const DAILY = [
  "---",
  'title: "2026-04-15"',
  'tags: ["daily"]',
  "---",
  "",
  "- Figure out finances #issue <!-- bid:abc123 -->",
  "  status:: todo",
  "  scheduled:: 2026-04-15",
  "- Buy groceries <!-- bid:def456 -->",
  "  status:: todo",
  "",
].join("\n");

test("setBlockProperty — :0 addresses the first bullet (not the blank line after frontmatter)", () => {
  // Regression for the inbox `x` triage bug: the client's body-splitting
  // kept the blank line between the closing `---` and the first bullet as
  // body line 0, so block_id `<note>:0` (which the server emits for the
  // first bullet) addressed the blank line instead. The visible effect was
  // a `status:: done` continuation line landing BEFORE the bullet rather
  // than mutating the existing `status:: todo`.
  const updated = setBlockProperty(DAILY, "2026-04-15:0", "status", "done");
  // The first bullet's existing `status:: todo` must be replaced in place.
  assert.match(updated, /\n- Figure out finances #issue[^\n]*\n  status:: done\n  scheduled:: 2026-04-15\n/);
  // And no stray `status::` line above the bullet.
  assert.doesNotMatch(updated, /---\n\n  status:: done\n- Figure out finances/);
});

test("setBlockProperty — :3 addresses the second bullet (line indexing matches server)", () => {
  // The server's `parse_blocks` indexes line numbers into the
  // gray-matter-trimmed body. In DAILY, that body is:
  //   0: `- Figure out finances …`
  //   1: `  status:: todo`
  //   2: `  scheduled:: 2026-04-15`
  //   3: `- Buy groceries …`
  // So the second bullet has block_id `<note>:3`.
  const updated = setBlockProperty(DAILY, "2026-04-15:3", "status", "done");
  assert.match(updated, /\n- Buy groceries[^\n]*\n  status:: done\n/);
});

test("setBlockProperty — no frontmatter still works", () => {
  const body = "- task one <!-- bid:1 -->\n  status:: todo\n";
  const updated = setBlockProperty(body, "n:0", "status", "done");
  assert.equal(updated, "- task one <!-- bid:1 -->\n  status:: done\n");
});

test("setBlockProperty — multiple blank lines after frontmatter still align", () => {
  const body = [
    "---",
    'title: "x"',
    "---",
    "",
    "",
    "",
    "- only bullet <!-- bid:1 -->",
    "",
  ].join("\n");
  const updated = setBlockProperty(body, "x:0", "status", "done");
  // The bullet must still get its `status:: done` continuation; the
  // blank gap before the bullet stays untouched.
  assert.match(updated, /---\n\n\n\n- only bullet[^\n]*\n  status:: done\n/);
});

test("setBlockText — first-line edit also indexes from the first non-blank body line", () => {
  // setBlockText shares the same body-splitting code path as
  // setBlockProperty, so the same regression applied.
  const updated = setBlockText(DAILY, "2026-04-15:0", "Figure out finances #issue (new)");
  assert.match(updated, /\n- Figure out finances #issue \(new\)\n  status:: todo\n/);
});

test("deleteBlock — also indexes from the first non-blank body line", () => {
  // deleteBlock shares the same body-splitting code path.
  const updated = deleteBlock(DAILY, "2026-04-15:0");
  // First bullet + its two continuation lines should be gone; the second
  // bullet must survive intact.
  assert.doesNotMatch(updated, /Figure out finances/);
  assert.match(updated, /\n- Buy groceries <!-- bid:def456 -->\n  status:: todo\n/);
});
