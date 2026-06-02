// Unit tests for the per-note block-ops debounce/coalesce/abort saver
// (sync redesign 2026-06-02, S1 follow-up).
//
// S1 dropped the 500ms debounce the whole-body-PUT path had, so the editor
// POSTed `/notes/{id}/blocks` on EVERY keystroke. `BlockOpsSaver` restores
// per-note coalescing: a burst of enqueues within the window collapses into
// ONE trailing-edge POST carrying the LATEST op per block, and a superseded
// in-flight POST is aborted (its AbortError swallowed, NOT PUT-fallen-back).
//
// The debounce uses real setTimeout; we drive it deterministically with
// node:test's mock timers (the suite's "test clock" pattern — cf.
// ws-refresh-coordinator's manual flush / window-backdate helpers).

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { BlockOpsSaver, isAbortError } from "../../src/lib/block-ops-saver.ts";

/** Build a concrete upsert op for a given bid/text. */
function upsert(bid, text, indent_level = 0, parent_bid = null) {
  return { kind: "upsert", bid, text, parent_bid, indent_level };
}

/** A controllable upsert spy: records every call and lets the test resolve /
 *  reject each POST's promise, and observe whether its signal aborted. */
function makeUpsertSpy() {
  const calls = [];
  const fn = (noteId, ops, signal) => {
    let resolve, reject;
    const promise = new Promise((res, rej) => {
      resolve = res;
      reject = rej;
    });
    const rec = { noteId, ops, signal, aborted: false, resolve, reject, promise };
    signal.addEventListener("abort", () => {
      rec.aborted = true;
      // Mirror fetch: an aborted request rejects with an AbortError.
      const err = new Error("aborted");
      err.name = "AbortError";
      reject(err);
    });
    calls.push(rec);
    return promise;
  };
  return { calls, fn };
}

test("isAbortError detects DOMException-style and plain AbortError", () => {
  const e1 = new Error("x");
  e1.name = "AbortError";
  assert.equal(isAbortError(e1), true);
  assert.equal(isAbortError({ name: "AbortError" }), true);
  assert.equal(isAbortError(new Error("nope")), false);
  assert.equal(isAbortError(null), false);
});

test("coalesce: N rapid enqueues for the same block within the window → ONE POST with the latest text", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallback = t.mock.fn();
  const saver = new BlockOpsSaver(spy.fn, fallback);

  // Five keystrokes on the same block, each within the debounce window.
  for (const text of ["h", "he", "hel", "hell", "hello"]) {
    saver.enqueue("noteA", [upsert("bid-1", text)]);
  }
  // Nothing fires before the window elapses.
  assert.equal(spy.calls.length, 0, "no POST until the trailing edge");

  // Trailing edge.
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1, "the whole burst collapses into one POST");
  assert.equal(spy.calls[0].noteId, "noteA");
  assert.deepEqual(
    spy.calls[0].ops,
    [upsert("bid-1", "hello")],
    "only the latest op for the block survives",
  );
  assert.equal(fallback.mock.callCount(), 0, "no PUT fallback on success");
});

test("coalesce: edits to different blocks in one window → one POST with one op per block", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "a1")]);
  saver.enqueue("noteA", [upsert("bid-2", "b1")]);
  saver.enqueue("noteA", [upsert("bid-1", "a2")]); // supersedes bid-1's op
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1);
  const byBid = Object.fromEntries(spy.calls[0].ops.map((o) => [o.bid, o.text]));
  assert.deepEqual(byBid, { "bid-1": "a2", "bid-2": "b1" }, "latest per block wins");
});

test("supersede: a later POST aborts the in-flight one; the AbortError is swallowed (no PUT fallback)", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallback = t.mock.fn();
  const saver = new BlockOpsSaver(spy.fn, fallback);

  // First batch flushes and the POST is in-flight (unresolved).
  saver.enqueue("noteA", [upsert("bid-1", "first")]);
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1, "first POST in-flight");
  assert.equal(spy.calls[0].aborted, false);

  // A second batch flushes before the first resolves → aborts the first.
  saver.enqueue("noteA", [upsert("bid-1", "second")]);
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 2, "second POST fired");
  assert.equal(spy.calls[0].aborted, true, "the superseded in-flight POST was aborted");
  assert.deepEqual(spy.calls[1].ops, [upsert("bid-1", "second")], "the latest wins");

  // Let the aborted promise's rejection settle. It must NOT trigger the PUT
  // fallback (that would double-write the superseding edit).
  await spy.calls[0].promise.catch(() => {});
  await Promise.resolve();
  assert.equal(fallback.mock.callCount(), 0, "abort is swallowed, not treated as a failure");

  // The live POST succeeds.
  spy.calls[1].resolve({});
  await spy.calls[1].promise;
  assert.equal(fallback.mock.callCount(), 0);
});

test("genuine (non-abort) failure → PUT fallback fires once", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallback = t.mock.fn();
  const saver = new BlockOpsSaver(spy.fn, fallback);

  saver.enqueue("noteA", [upsert("bid-1", "x")]);
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1);

  spy.calls[0].reject(new Error("500 boom"));
  await spy.calls[0].promise.catch(() => {});
  await Promise.resolve();
  assert.equal(fallback.mock.callCount(), 1, "genuine failure falls back to whole-body PUT");
  assert.deepEqual(fallback.mock.calls[0].arguments, ["noteA"]);
});

test("supersedeWithBody: cancels the pending block-ops batch (timer + in-flight) without double-sending, then PUTs", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  // A pending (un-flushed) coalesced batch.
  saver.enqueue("noteA", [upsert("bid-1", "typed")]);
  assert.equal(saver.hasPending("noteA"), true);

  // A structural edit supersedes it with a whole-body PUT.
  const put = t.mock.fn();
  saver.supersedeWithBody("noteA", put);
  assert.equal(put.mock.callCount(), 1, "the body PUT fired");
  assert.equal(saver.hasPending("noteA"), false, "pending block-ops were cleared");

  // The cancelled batch must NOT also POST when the (now-cleared) timer would
  // have fired.
  t.mock.timers.tick(1000);
  assert.equal(spy.calls.length, 0, "no block-ops POST after supersede — one path per save");
});

test("supersedeWithBody aborts an in-flight block-ops POST before PUTting", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "typed")]);
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1, "first POST in-flight");

  saver.supersedeWithBody("noteA", () => {});
  assert.equal(spy.calls[0].aborted, true, "in-flight POST aborted on supersede");
});

test("flush (forced, e.g. on blur) lands a pending batch immediately without waiting for the timer", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "edit")]);
  assert.equal(spy.calls.length, 0, "nothing yet");
  saver.flush("noteA");
  assert.equal(spy.calls.length, 1, "blur flush fires the POST immediately");
  assert.deepEqual(spy.calls[0].ops, [upsert("bid-1", "edit")]);

  // The (now-disarmed) debounce timer must not double-fire.
  t.mock.timers.tick(1000);
  assert.equal(spy.calls.length, 1, "no double-send after a forced flush");
});

test("flush is a no-op when nothing is pending", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());
  saver.flush("noteA");
  assert.equal(spy.calls.length, 0);
});

test("flushAll (teardown) flushes every note's pending batch", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("a", "1")]);
  saver.enqueue("noteB", [upsert("b", "2")]);
  saver.flushAll();
  assert.equal(spy.calls.length, 2);
  assert.deepEqual(spy.calls.map((c) => c.noteId).sort(), ["noteA", "noteB"]);
});

test("per-note isolation: a flush for one note leaves another note's pending batch armed", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("a", "1")]);
  saver.enqueue("noteB", [upsert("b", "2")]);
  saver.flush("noteA");
  assert.equal(spy.calls.length, 1);
  assert.equal(spy.calls[0].noteId, "noteA");
  assert.equal(saver.hasPending("noteB"), true, "noteB still pending");

  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 2, "noteB flushes on its own trailing edge");
  assert.equal(spy.calls[1].noteId, "noteB");
});
