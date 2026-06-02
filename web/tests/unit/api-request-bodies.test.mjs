// Unit tests for the PUT /notes/{id} request-body builder (base-diff fix,
// 2026-06-02).
//
// The whole-note PUT now sends an optional `base_content` (the author's edit
// BASE) so the server diffs `base_content → content` — the author's REAL
// changes — instead of `server_file → content`, and an untouched block is
// never re-asserted over a concurrent peer edit. The wire field MUST be
// `base_content` (snake_case) to match `UpdateNoteReq` in
// `crates/tesela-server/src/routes/notes.rs`, and it MUST be OMITTED (not
// `undefined`) when no base is supplied so a base-less PUT is byte-identical
// to a legacy client (server falls back to its historical server-file diff).

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { buildUpdateNoteBody } from "../../src/lib/api-request-bodies.ts";

test("includes base_content (snake_case) when a base is supplied", () => {
  const body = buildUpdateNoteBody("- new body\n", "- base body\n");
  assert.deepEqual(body, { content: "- new body\n", base_content: "- base body\n" });
});

test("omits base_content entirely when no base is supplied (backward compat)", () => {
  const body = buildUpdateNoteBody("- new body\n");
  assert.deepEqual(body, { content: "- new body\n" });
  assert.equal("base_content" in body, false, "the field is absent, not undefined");
});

test("an empty-string base is still sent (a real base of an empty note)", () => {
  const body = buildUpdateNoteBody("- typed\n", "");
  assert.deepEqual(body, { content: "- typed\n", base_content: "" });
  assert.equal("base_content" in body, true);
});
