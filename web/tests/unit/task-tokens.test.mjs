import { test } from "node:test";
import assert from "node:assert/strict";
import {
  detectTokens,
  detectTaskTokens,
  literalRanges,
} from "../../src/lib/task-tokens.ts";

// The invariant under test: the NLP blur lift must never detect — and so
// never strip — tokens inside literal ranges on the first line (wiki links,
// markdown links/images, bare URLs, inline code). `\b`-anchored trigger
// regexes fire after `/`, `[`, and a backtick, so without the pre-claimed
// exclusion a `p1` inside a URL was lifted as priority AND excised from the
// link target on commit.

/** A representative detect spec (a #Task-style tag's properties). */
const SPEC = {
  defaultDateProperty: "scheduled",
  properties: [
    { key: "priority", valueType: "select", choices: ["p1", "p2", "p3", "p4"], triggers: ["p1", "p2", "p3", "p4"] },
    { key: "points", valueType: "number", choices: [], triggers: ["pts", "points"] },
    { key: "deadline", valueType: "date", choices: [], triggers: ["due"] },
  ],
};

function keys(tokens) {
  return tokens.map((t) => t.key).sort();
}

test("literalRanges finds wiki links, markdown links, inline code, and bare URLs", () => {
  const line = "see [[plan]] and [spec](https://x.com/p1) or `p2` at https://y.io/z";
  const ranges = literalRanges(line);
  const covered = (s) => {
    const at = line.indexOf(s);
    return ranges.some(([a, b]) => at >= a && at + s.length <= b);
  };
  assert.ok(covered("[[plan]]"), "wiki link covered");
  assert.ok(covered("[spec](https://x.com/p1)"), "markdown link covered");
  assert.ok(covered("`p2`"), "inline code covered");
  assert.ok(covered("https://y.io/z"), "bare URL covered");
});

test("select trigger inside a bare URL is not detected (and not stripped)", () => {
  const text = "review https://x.com/p1/doc p2";
  const tokens = detectTokens(text, SPEC);
  assert.deepEqual(keys(tokens), ["priority"], "only the bare p2 detected");
  assert.equal(text.slice(tokens[0].from, tokens[0].to), "p2");
  const det = detectTaskTokens(text, SPEC);
  assert.deepEqual(det.props, [{ key: "priority", value: "p2" }]);
  assert.ok(det.stripped.includes("https://x.com/p1/doc"), "URL survives the strip intact");
});

test("select trigger inside a markdown link target is not detected", () => {
  const text = "read [spec](https://x.com/p1/doc) today-ish";
  const tokens = detectTokens(text, SPEC).filter((t) => t.key === "priority");
  assert.equal(tokens.length, 0);
  const det = detectTaskTokens(text, SPEC);
  assert.ok(det.stripped.includes("[spec](https://x.com/p1/doc)"), "link target untouched");
});

test("select trigger inside inline code is not detected", () => {
  const text = "run `p1` now";
  const tokens = detectTokens(text, SPEC);
  assert.equal(tokens.filter((t) => t.key === "priority").length, 0);
  assert.equal(detectTaskTokens(text, SPEC).stripped, text, "nothing stripped");
});

test("select trigger inside a wiki link is not detected", () => {
  const text = "ship [[p1 spec]] p2";
  const tokens = detectTokens(text, SPEC);
  assert.deepEqual(keys(tokens), ["priority"]);
  assert.equal(text.slice(tokens[0].from, tokens[0].to), "p2", "only the bare p2");
  const det = detectTaskTokens(text, SPEC);
  assert.ok(det.stripped.includes("[[p1 spec]]"), "wiki link survives intact");
});

test("number trigger inside a URL is not detected", () => {
  const text = "estimate https://x.com/5pts/page";
  const tokens = detectTokens(text, SPEC);
  assert.equal(tokens.filter((t) => t.key === "points").length, 0);
});

test("bare date word inside a wiki link is not lifted (link target preserved)", () => {
  const text = "plan [[meeting tomorrow review]]";
  const tokens = detectTokens(text, SPEC);
  assert.equal(tokens.length, 0, "no date token inside the link");
  const det = detectTaskTokens(text, SPEC);
  assert.equal(det.stripped, text, "the wiki-link target is not rewritten");
  assert.deepEqual(det.props, []);
});

test("date-trigger phrase inside a markdown link is not lifted", () => {
  const text = "check [due tomorrow](https://x.com/q)";
  const det = detectTaskTokens(text, SPEC);
  assert.equal(det.stripped, text);
  assert.deepEqual(det.props, []);
});

test("plain tokens outside literals still detect and strip (no regression)", () => {
  const text = "fix the parser p1 due tomorrow 3 pts";
  const tokens = detectTokens(text, SPEC);
  assert.deepEqual(keys(tokens), ["deadline", "points", "priority"]);
  const det = detectTaskTokens(text, SPEC);
  assert.equal(det.stripped, "fix the parser");
  const byKey = Object.fromEntries(det.props.map((p) => [p.key, p.value]));
  assert.equal(byKey.priority, "p1");
  assert.equal(byKey.points, "3");
  assert.match(byKey.deadline, /^\d{4}-\d{2}-\d{2}$/, "deadline parsed to an ISO date");
});

test("tokens adjacent to a literal range still detect", () => {
  const text = "p1 [[notes]] due tomorrow";
  const tokens = detectTokens(text, SPEC);
  assert.deepEqual(keys(tokens), ["deadline", "priority"]);
  const det = detectTaskTokens(text, SPEC);
  assert.ok(det.stripped.includes("[[notes]]"), "link untouched between lifted tokens");
});

test("detection stays first-line-only (second-line URL irrelevant, second-line token untouched)", () => {
  const text = "fix p1\nsee https://x.com/p2 and p3";
  const det = detectTaskTokens(text, SPEC);
  assert.equal(det.stripped, "fix\nsee https://x.com/p2 and p3");
  assert.deepEqual(det.props, [{ key: "priority", value: "p1" }]);
});
