// Wire-protocol helpers for the live dictation WS (dictation P2).
// Mirrors the server's StreamEvent serde shape
// (crates/tesela-server/src/asr_engine.rs) — if these fail after a
// server change, the two sides have drifted.
import test from "node:test";
import assert from "node:assert/strict";

import { parseServerFrame, STOP_FRAME, formatElapsed } from "../../src/lib/voice/protocol.ts";

test("stop frame matches the server's control contract", () => {
  assert.deepEqual(JSON.parse(STOP_FRAME), { type: "stop" });
});

test("parses every server frame type", () => {
  assert.deepEqual(parseServerFrame('{"type":"ready","model_id":"m","streaming":true}'), {
    type: "ready",
    model_id: "m",
    streaming: true,
  });
  assert.deepEqual(
    parseServerFrame('{"type":"partial","committed":"a ","tentative":"b","revision":3}'),
    { type: "partial", committed: "a ", tentative: "b", revision: 3 },
  );
  assert.deepEqual(
    parseServerFrame('{"type":"final","text":"done","model_id":"m","duration_ms":42}'),
    { type: "final", text: "done", model_id: "m", duration_ms: 42 },
  );
  assert.deepEqual(parseServerFrame('{"type":"error","message":"boom"}'), {
    type: "error",
    message: "boom",
  });
});

test("rejects malformed frames as null, never throws", () => {
  assert.equal(parseServerFrame("not json"), null);
  assert.equal(parseServerFrame("[]"), null);
  assert.equal(parseServerFrame("null"), null);
  assert.equal(parseServerFrame('{"type":"nope"}'), null);
  // right type, missing fields
  assert.equal(parseServerFrame('{"type":"ready","model_id":"m"}'), null);
  assert.equal(parseServerFrame('{"type":"partial","committed":"a"}'), null);
  assert.equal(parseServerFrame('{"type":"final"}'), null);
  assert.equal(parseServerFrame('{"type":"error"}'), null);
});

test("final tolerates a missing duration", () => {
  assert.deepEqual(parseServerFrame('{"type":"final","text":"t","model_id":"m"}'), {
    type: "final",
    text: "t",
    model_id: "m",
    duration_ms: 0,
  });
});

test("elapsed formatting", () => {
  assert.equal(formatElapsed(0), "0:00");
  assert.equal(formatElapsed(7.9), "0:07");
  assert.equal(formatElapsed(65), "1:05");
  assert.equal(formatElapsed(600), "10:00");
  assert.equal(formatElapsed(-3), "0:00");
});
