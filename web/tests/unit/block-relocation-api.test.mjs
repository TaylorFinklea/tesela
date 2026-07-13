import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const apiClient = readFileSync(
  new URL("../../src/lib/api-client.ts", import.meta.url),
  "utf8",
);

test("relocateBlockSubtree posts the typed move request and suppresses echoes for every affected note", () => {
  assert.match(
    apiClient,
    /import type \{ BlockMoveRequest \} from ["']\$lib\/block-tree-move["'];/,
  );

  const methodStart = apiClient.indexOf("relocateBlockSubtree:");
  assert.notEqual(methodStart, -1, "expected api.relocateBlockSubtree");

  const sourceSave = apiClient.indexOf(
    "recordLocalSave(req.source_note_id)",
    methodStart,
  );
  const destinationSave = apiClient.indexOf(
    "recordLocalSave(req.destination_note_id)",
    methodStart,
  );
  const requestPost = apiClient.indexOf(
    'post<{ move_id: string; notes: Note[] }>("/blocks/move-subtree", req, signal)',
    methodStart,
  );
  const returnedSave = apiClient.indexOf("recordLocalSave(note.id)", requestPost);

  assert.ok(sourceSave > methodStart, "source own-echo window must open before POST");
  assert.ok(
    destinationSave > sourceSave,
    "destination own-echo window must open before POST",
  );
  assert.ok(requestPost > destinationSave, "move request must follow both preflight saves");
  assert.ok(returnedSave > requestPost, "every returned note id must be recorded after POST");
  assert.match(
    apiClient.slice(methodStart, returnedSave + "recordLocalSave(note.id)".length),
    /relocateBlockSubtree:\s*\(req: BlockMoveRequest, signal\?: AbortSignal\)[\s\S]*for \(const note of result\.notes\)[\s\S]*recordLocalSave\(note\.id\)/,
  );
});
