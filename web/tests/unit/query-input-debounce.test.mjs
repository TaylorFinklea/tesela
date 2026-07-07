// Unit tests for QueryInput's tiny trailing-edge debouncer
// (web/src/lib/query-input/debounce.ts) — tesela-vp9.2. Drives the
// ~150ms-delayed parseQueryWithDiagnostics pass.
import { test } from "node:test";
import assert from "node:assert/strict";

import { createDebouncer } from "../../src/lib/query-input/debounce.ts";

function wait(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

test("fires once, after the delay, with the last call's arguments", async () => {
  const calls = [];
  const d = createDebouncer((v) => calls.push(v), 20);
  d.call("a");
  d.call("b");
  d.call("c");
  assert.deepEqual(calls, []); // nothing yet — still debouncing
  await wait(60);
  assert.deepEqual(calls, ["c"]);
});

test("each call resets the timer — rapid calls under the delay never fire", async () => {
  const calls = [];
  const d = createDebouncer((v) => calls.push(v), 30);
  for (let i = 0; i < 5; i++) {
    d.call(i);
    await wait(10); // always under the 30ms delay — keeps resetting
  }
  assert.deepEqual(calls, []);
  await wait(50);
  assert.deepEqual(calls, [4]);
});

test("cancel() suppresses a pending call", async () => {
  const calls = [];
  const d = createDebouncer((v) => calls.push(v), 20);
  d.call("x");
  d.cancel();
  await wait(40);
  assert.deepEqual(calls, []);
});

test("supports multiple independent argument calls over time", async () => {
  const calls = [];
  const d = createDebouncer((v) => calls.push(v), 15);
  d.call("first");
  await wait(30);
  assert.deepEqual(calls, ["first"]);
  d.call("second");
  await wait(30);
  assert.deepEqual(calls, ["first", "second"]);
});
