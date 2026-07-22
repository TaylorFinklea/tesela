import { test } from "node:test";
import { strict as assert } from "node:assert";
import { rankPageCandidates, resolveNodeValue } from "../../src/lib/node-relations.ts";

const target = "11111111-1111-5111-8111-111111111111";
const directory = [
  {
    page_id: target,
    loro_doc_id: "a".repeat(32),
    slug: "tesela",
    title: "Tesela Notes",
    aliases: ["Graph"],
    deleted: false,
    forward_to_loro_doc_id: null,
    conflict: false,
  },
];

test("resolves canonical PageId and preserves unresolved legacy text", () => {
  assert.deepEqual(resolveNodeValue(target, directory), {
    state: "resolved", pageId: target, slug: "tesela", title: "Tesela Notes",
  });
  const legacy = resolveNodeValue("old page name", directory);
  assert.equal(legacy.state, "unresolved");
  assert.match(legacy.label, /old page name/);
});

test("picker filters live non-conflicting pages", () => {
  assert.deepEqual(rankPageCandidates(directory, "graph").map((entry) => entry.page_id), [target]);
  assert.deepEqual(rankPageCandidates([{ ...directory[0], conflict: true }], "").map((entry) => entry.page_id), []);
});

test("page properties include canonical body properties written by the page API", async () => {
  const { pagePropertyEntries } = await import("../../src/lib/node-relations.ts");
  assert.deepEqual(
    pagePropertyEntries(
      "project:: 11111111-1111-5111-8111-111111111111\n- source block\n  child:: block-owned",
      { owner: "Taylor", tesela_page_id: "reserved" },
    ),
    [
      { k: "owner", v: "Taylor" },
      { k: "tesela_page_id", v: "reserved" },
      { k: "project", v: "11111111-1111-5111-8111-111111111111" },
    ],
  );
});
