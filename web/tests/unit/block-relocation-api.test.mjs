import assert from "node:assert/strict";
import test from "node:test";

import * as blockTreeMove from "../../src/lib/block-tree-move.ts";

const request = {
  move_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
  source_note_id: "2026-07-12",
  root_bid: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
  destination_note_id: "2026-07-13",
  target_bid: null,
  placement: "append",
};

test("relocation executor preserves the exact transport contract and save ordering", async () => {
  const execute = blockTreeMove.executeBlockSubtreeRelocation;
  assert.equal(typeof execute, "function", "expected the API's relocation executor");

  const controller = new AbortController();
  const response = {
    move_id: request.move_id,
    notes: [{ id: request.source_note_id }, { id: request.destination_note_id }],
  };
  const calls = [];

  const result = await execute(request, controller.signal, {
    post: async (path, body, signal) => {
      calls.push({ kind: "post", path, body, signal });
      return response;
    },
    recordLocalSave: (id) => calls.push({ kind: "save", id }),
  });

  assert.strictEqual(result, response);
  assert.deepEqual(
    calls.map((call) =>
      call.kind === "save" ? ["save", call.id] : ["post", call.path],
    ),
    [
      ["save", request.source_note_id],
      ["save", request.destination_note_id],
      ["post", "/blocks/move-subtree"],
      ["save", request.source_note_id],
      ["save", request.destination_note_id],
    ],
  );
  assert.strictEqual(calls[2].body, request, "POST must receive the exact request object");
  assert.strictEqual(calls[2].signal, controller.signal, "POST must receive the same signal");
});

test("relocation executor propagates transport rejection without post-response saves", async () => {
  const execute = blockTreeMove.executeBlockSubtreeRelocation;
  assert.equal(typeof execute, "function", "expected the API's relocation executor");

  const rejection = new Error("transport failed");
  const saves = [];
  await assert.rejects(
    execute(request, undefined, {
      post: async () => {
        throw rejection;
      },
      recordLocalSave: (id) => saves.push(id),
    }),
    (error) => error === rejection,
  );

  assert.deepEqual(saves, [request.source_note_id, request.destination_note_id]);
});
