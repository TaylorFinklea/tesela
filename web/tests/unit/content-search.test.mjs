import assert from "node:assert/strict";
import test from "node:test";
import {
  debounce,
  findContentBlockId,
  mapContentHits,
  snippetRuns,
} from "../../src/lib/content-search.ts";

const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const hit = (noteId, snippet) => ({
  note_id: noteId,
  title: `Title ${noteId}`,
  snippet,
  rank: -1,
  tags: [],
  path: `notes/${noteId}.md`,
});

test("debounce keeps only the last query in a typing burst", async () => {
  const calls = [];
  const schedule = debounce((query) => calls.push(query), 20);

  schedule("tes");
  schedule("tesela");
  await wait(35);

  assert.deepEqual(calls, ["tesela"]);
  schedule.cancel();
});

test("mapContentHits drops non-body hits, caps results, and preserves jump data", () => {
  const hits = [
    hit("title-only", ""),
    ...Array.from({ length: 21 }, (_, i) => hit(`note-${i}`, `<b>needle</b> body ${i}`)),
  ];

  const rows = mapContentHits(hits, "  needle ");

  assert.equal(rows.length, 20);
  assert.equal(rows[0].kind, "content");
  assert.equal(rows[0].noteId, "note-0");
  assert.equal(rows[0].query, "needle");
  assert.equal(rows[0].snippet, "<b>needle</b> body 0");
  assert.notEqual(rows[0].key, rows[1].key);
});

test("snippetRuns turns FTS bold markers into safe text runs", () => {
  assert.deepEqual(snippetRuns("...before <b>needle</b> after..."), [
    { text: "...before ", match: false },
    { text: "needle", match: true },
    { text: " after...", match: false },
  ]);
});

test("findContentBlockId picks a body-only match instead of the title block", () => {
  const blocks = [
    { id: "note:title", raw_text: "- A title about projects" },
    { id: "note:body", raw_text: "- The body-only retrieval needle is here" },
  ];

  assert.equal(
    findContentBlockId(blocks, "needle", "...retrieval <b>needle</b> is here"),
    "note:body",
  );
});
