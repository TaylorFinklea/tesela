// Unit tests for the per-note block-ops debounce/coalesce/serialization saver
// (sync redesign 2026-06-02, S1 follow-up).
//
// S1 dropped the 500ms debounce the whole-body-PUT path had, so the editor
// POSTed `/notes/{id}/blocks` on EVERY keystroke. `BlockOpsSaver` restores
// per-note coalescing: a burst of enqueues within the window collapses into
// ONE trailing-edge POST carrying the LATEST op per block. Once admitted, an
// in-flight POST is drained before its coalesced successor starts: aborting a
// fetch cannot prove the server stopped processing the write.
//
// The debounce uses real setTimeout; we drive it deterministically with
// node:test's mock timers (the suite's "test clock" pattern — cf.
// ws-refresh-coordinator's manual flush / window-backdate helpers).

import { test } from "node:test";
import { strict as assert } from "node:assert";

import * as saverModule from "../../src/lib/block-ops-saver.ts";

const { BlockOpsSaver } = saverModule;

/** Build a concrete upsert op for a given bid/text. */
function upsert(bid, text, indent_level = 0, parent_bid = null) {
  return { kind: "upsert", bid, text, parent_bid, indent_level };
}

/** Build a concrete delete op for a given bid. */
function del(bid) {
  return { kind: "delete", bid };
}

/** Build a concrete move op for a given bid. */
function move(bid, parent_bid = null, indent_level = 0) {
  return { kind: "move", bid, parent_bid, indent_level };
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

function deferredMutation() {
  let resolve, reject;
  const promise = new Promise((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

test("per-note mutation barrier drains a successor added while its predecessor is in flight", async () => {
  assert.equal(
    typeof saverModule.PerNoteMutationBarrier,
    "function",
    "expected a reusable per-note mutation barrier",
  );
  const barrier = new saverModule.PerNoteMutationBarrier();
  const predecessor = deferredMutation();
  const successor = deferredMutation();
  void barrier.track("noteA", () => predecessor.promise);

  let settled = false;
  const completion = barrier.settle("noteA").then(() => {
    settled = true;
  });
  void barrier.track("noteA", () => successor.promise);

  predecessor.resolve();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(settled, false, "a late successor remains inside the barrier");

  successor.resolve();
  await completion;
  assert.equal(settled, true);
});

test("per-note mutation barrier propagates a write failure after draining the note", async () => {
  assert.equal(typeof saverModule.PerNoteMutationBarrier, "function");
  const barrier = new saverModule.PerNoteMutationBarrier();
  const mutation = deferredMutation();
  const tracked = barrier.track("noteA", () => mutation.promise);
  const completion = barrier.settle("noteA");
  const failure = new Error("property write failed");

  mutation.reject(failure);
  await assert.rejects(tracked, failure);
  await assert.rejects(completion, failure);
});

test("per-note mutation barrier latches a rejection that finishes before settle starts", async () => {
  assert.equal(typeof saverModule.PerNoteMutationBarrier, "function");
  const barrier = new saverModule.PerNoteMutationBarrier();
  const mutation = deferredMutation();
  const tracked = barrier.track("noteA", () => mutation.promise);
  const failure = new Error("property write failed early");

  mutation.reject(failure);
  await assert.rejects(tracked, failure);
  await assert.rejects(barrier.settle("noteA"), failure);
  await assert.rejects(
    barrier.settle("noteA"),
    failure,
    "a second move attempt must stay blocked until authoritative reconciliation",
  );
});

test("per-note mutation barrier starts rapid mutations in registration order", async () => {
  assert.equal(typeof saverModule.PerNoteMutationBarrier, "function");
  const barrier = new saverModule.PerNoteMutationBarrier();
  const predecessor = deferredMutation();
  const successor = deferredMutation();
  const starts = [];

  const first = barrier.track("noteA", () => {
    starts.push("first");
    return predecessor.promise;
  });
  const second = barrier.track("noteA", () => {
    starts.push("second");
    return successor.promise;
  });
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(starts, ["first"]);

  predecessor.resolve();
  await first;
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(starts, ["first", "second"]);

  successor.resolve();
  await second;
  await barrier.settle("noteA");
});

test("per-note mutation reservation synchronously rejects late writes without latching failure", async () => {
  assert.equal(typeof saverModule.PerNoteMutationBarrier, "function");
  const barrier = new saverModule.PerNoteMutationBarrier();
  const reservation = barrier.reserve(["noteA"]);
  let started = false;

  assert.equal(barrier.isReserved("noteA"), true);
  await assert.rejects(
    barrier.track("noteA", async () => {
      started = true;
    }),
    /reserved for block relocation/i,
  );
  assert.equal(started, false, "a late mutation must not reach the network factory");

  reservation.release();
  assert.equal(barrier.isReserved("noteA"), false);
  await barrier.track("noteA", async () => {
    started = true;
  });
  assert.equal(started, true);
  await barrier.settle("noteA");
});

test("per-note mutation reservation drains writes captured before acquisition", async () => {
  assert.equal(typeof saverModule.PerNoteMutationBarrier, "function");
  const barrier = new saverModule.PerNoteMutationBarrier();
  const predecessor = deferredMutation();
  void barrier.track("noteA", () => predecessor.promise);
  const reservation = barrier.reserve(["noteA"]);

  let settled = false;
  const completion = reservation.settle().then(() => {
    settled = true;
  });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(settled, false);

  predecessor.resolve();
  await completion;
  assert.equal(settled, true);
  assert.equal(barrier.isReserved("noteA"), true, "settling must retain the reservation");
  reservation.release();
});

test("overlapping reservation failure is atomic across notes", () => {
  assert.equal(typeof saverModule.PerNoteMutationBarrier, "function");
  const barrier = new saverModule.PerNoteMutationBarrier();
  const first = barrier.reserve(["noteA"]);

  assert.throws(
    () => barrier.reserve(["noteB", "noteA"]),
    /reserved for block relocation/i,
  );
  assert.equal(barrier.isReserved("noteA"), true);
  assert.equal(barrier.isReserved("noteB"), false, "failed acquisition must not partially reserve");

  first.release();
});

test("combined block-move reservation drains direct note writes and rejects late ones", async () => {
  assert.equal(typeof saverModule.createCombinedMutationBarrier, "function");
  const uiBarrier = new saverModule.PerNoteMutationBarrier();
  const noteWriteBarrier = new saverModule.PerNoteMutationBarrier();
  const combined = saverModule.createCombinedMutationBarrier(uiBarrier, noteWriteBarrier);
  const predecessor = deferredMutation();
  const tracked = noteWriteBarrier.track("noteA", () => predecessor.promise);
  const reservation = combined.reserve(["noteA"]);

  let settled = false;
  const completion = reservation.settle().then(() => {
    settled = true;
  });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(settled, false, "an unmounted/direct predecessor remains inside preflight");
  await assert.rejects(
    noteWriteBarrier.track("noteA", async () => {}),
    /reserved for block relocation/i,
  );

  predecessor.resolve();
  await tracked;
  await completion;
  assert.equal(uiBarrier.isReserved("noteA"), true);
  assert.equal(noteWriteBarrier.isReserved("noteA"), true);
  reservation.release();
  assert.equal(uiBarrier.isReserved("noteA"), false);
  assert.equal(noteWriteBarrier.isReserved("noteA"), false);
});

test("per-note mutation reservation synchronously notifies every mounted subscriber", () => {
  const barrier = new saverModule.PerNoteMutationBarrier();
  const noteAStates = [];
  const noteBStates = [];
  const unsubscribeA1 = barrier.subscribe("noteA", (reserved) => noteAStates.push(reserved));
  const unsubscribeA2 = barrier.subscribe("noteA", (reserved) => noteAStates.push(reserved));
  const unsubscribeB = barrier.subscribe("noteB", (reserved) => noteBStates.push(reserved));

  assert.deepEqual(noteAStates, [false, false], "subscriptions publish their initial state");
  assert.deepEqual(noteBStates, [false]);

  const reservation = barrier.reserve(["noteA", "noteB"]);
  assert.deepEqual(noteAStates, [false, false, true, true]);
  assert.deepEqual(noteBStates, [false, true]);

  reservation.release();
  assert.deepEqual(noteAStates, [false, false, true, true, false, false]);
  assert.deepEqual(noteBStates, [false, true, false]);

  unsubscribeA1();
  unsubscribeA2();
  unsubscribeB();
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

test("delete: a single delete op flushes as one POST (no PUT fallback) — S4", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallback = t.mock.fn();
  const saver = new BlockOpsSaver(spy.fn, fallback);

  saver.enqueue("noteA", [del("dead-1")]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1, "the delete POSTs once");
  assert.deepEqual(spy.calls[0].ops, [del("dead-1")]);
  assert.equal(fallback.mock.callCount(), 0, "a delete never triggers the whole-body PUT");
});

test("delete: a multi-block delete batches into one POST keyed by bid — S4", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [del("a"), del("b"), del("c")]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1);
  const bids = spy.calls[0].ops.map((o) => o.bid).sort();
  assert.deepEqual(bids, ["a", "b", "c"], "all three deletes ride in one POST");
  assert.ok(spy.calls[0].ops.every((o) => o.kind === "delete"));
});

test("delete: a delete coalesces alongside a pending text edit to another block — S4", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  // A pending text edit, then a delete of a different block in the same window.
  saver.enqueue("noteA", [upsert("keep", "edited")]);
  saver.enqueue("noteA", [del("gone")]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1, "both ride one trailing-edge POST");
  const byBid = Object.fromEntries(spy.calls[0].ops.map((o) => [o.bid, o.kind]));
  assert.deepEqual(byBid, { keep: "upsert", gone: "delete" });
});

test("kind-aware coalesce: a move over a pending upsert folds structure in, keeping the typed text", (t) => {
  // The data-loss shape: type into a block (upsert with the final text), then
  // Tab to indent within the 500ms window (move for the SAME bid). Blind
  // latest-wins replaced the upsert with the text-less move — the typing
  // burst was never sent, while lastSentBody had already advanced so the
  // own-echo guard hid the loss until a reseed reverted it.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "typed text", 0, null)]);
  saver.enqueue("noteA", [move("bid-1", "parent-9", 1)]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1, "one trailing-edge POST");
  assert.deepEqual(
    spy.calls[0].ops,
    [upsert("bid-1", "typed text", 1, "parent-9")],
    "the upsert survives, carrying the move's parent/indent",
  );
});

test("kind-aware coalesce: the fold preserves an upsert's after_bid positional hint", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [{ ...upsert("bid-1", "split half"), after_bid: "pred-1" }]);
  saver.enqueue("noteA", [move("bid-1", null, 0)]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1);
  assert.equal(spy.calls[0].ops[0].kind, "upsert");
  assert.equal(spy.calls[0].ops[0].text, "split half");
  assert.equal(spy.calls[0].ops[0].after_bid, "pred-1", "positional hint survives the fold");
});

test("kind-aware coalesce: an upsert over a pending move supersedes it (text + structure ride the upsert)", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [move("bid-1", "parent-9", 1)]);
  saver.enqueue("noteA", [upsert("bid-1", "typed after indent", 1, "parent-9")]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1);
  assert.deepEqual(spy.calls[0].ops, [upsert("bid-1", "typed after indent", 1, "parent-9")]);
});

test("kind-aware coalesce: a delete supersedes a pending upsert (latest-wins is correct there)", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "doomed text")]);
  saver.enqueue("noteA", [del("bid-1")]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1);
  assert.deepEqual(spy.calls[0].ops, [del("bid-1")]);
});

test("kind-aware coalesce: a lone move (no pending upsert) flushes as a move", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [move("bid-1", "parent-9", 1)]);
  t.mock.timers.tick(500);

  assert.equal(spy.calls.length, 1);
  assert.deepEqual(spy.calls[0].ops, [move("bid-1", "parent-9", 1)]);
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

test("a later POST waits for the admitted in-flight write instead of aborting it", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallback = t.mock.fn();
  const saver = new BlockOpsSaver(spy.fn, fallback);

  // First batch flushes and the POST is in-flight (unresolved).
  saver.enqueue("noteA", [upsert("bid-1", "first")]);
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1, "first POST in-flight");
  assert.equal(spy.calls[0].aborted, false);

  // A second batch reaches its trailing edge before the first resolves. It
  // remains queued because canceling fetch would not prove the server stopped.
  saver.enqueue("noteA", [upsert("bid-1", "second")]);
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1, "successor waits behind the admitted write");
  assert.equal(spy.calls[0].aborted, false);

  spy.calls[0].resolve({});
  await spy.calls[0].promise;
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(spy.calls.length, 2, "successor starts after its predecessor settles");
  assert.deepEqual(spy.calls[1].ops, [upsert("bid-1", "second")], "the latest wins");
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

test("AbortError is ambiguous and falls back instead of being treated as canceled", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallback = t.mock.fn();
  const saver = new BlockOpsSaver(spy.fn, fallback);

  saver.enqueue("noteA", [upsert("bid-1", "x")]);
  t.mock.timers.tick(500);
  const abort = new Error("transport canceled after admission");
  abort.name = "AbortError";
  spy.calls[0].reject(abort);
  await spy.calls[0].promise.catch(() => {});
  await Promise.resolve();

  assert.equal(fallback.mock.callCount(), 1);
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

test("supersedeWithBody leaves an admitted block-ops POST alive", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "typed")]);
  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1, "first POST in-flight");

  saver.supersedeWithBody("noteA", () => {});
  assert.equal(spy.calls[0].aborted, false, "admitted writes cannot be canceled durably");
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

test("save admission repeats a parent drain when a child re-admits its late fallback", async () => {
  assert.equal(typeof saverModule.PerNoteSaveAdmissionRegistry, "function");
  const registry = new saverModule.PerNoteSaveAdmissionRegistry();
  const releaseChildDrain = deferredMutation();
  let parentLease = null;
  let childLease = null;
  let parentDrains = 0;

  const admitParent = () => {
    parentLease = registry.admit("noteA", async () => {
      parentDrains += 1;
      const lease = parentLease;
      parentLease = null;
      lease.release();
    });
  };

  admitParent();
  childLease = registry.admit("noteA", async () => {
    await releaseChildDrain.promise;
    // Mirrors a BlockOpsSaver failure fallback: the parent already drained
    // once, but this causal successor becomes active before the child releases.
    admitParent();
    const lease = childLease;
    childLease = null;
    lease.release();
  });

  let settled = false;
  const completion = registry.settle(["noteA"]).then(() => {
    settled = true;
  });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(parentDrains, 1, "the parent may become quiet before its child");
  assert.equal(settled, false, "the active child keeps global settlement open");

  releaseChildDrain.resolve();
  await completion;
  assert.equal(parentDrains, 2, "the re-admitted fallback is drained in a new generation");
});

test("teardown admission remains globally drainable through an in-flight predecessor and successor", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  assert.equal(typeof saverModule.PerNoteSaveAdmissionRegistry, "function");
  const registry = new saverModule.PerNoteSaveAdmissionRegistry();
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn(), 500, registry);

  saver.enqueue("noteA", [upsert("bid-1", "first")]);
  t.mock.timers.tick(500);
  saver.enqueue("noteA", [upsert("bid-1", "second")]);
  const disposing = saver.dispose();
  let relocationSettled = false;
  const relocationDrain = registry.settle(["noteA"]).then(() => {
    relocationSettled = true;
  });
  assert.equal(spy.calls.length, 1, "the predecessor remains the only admitted write");

  spy.calls[0].resolve({});
  await spy.calls[0].promise;
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(spy.calls.length, 2, "the teardown successor starts before admission releases");
  assert.deepEqual(spy.calls[1].ops, [upsert("bid-1", "second")]);
  assert.equal(relocationSettled, false, "global settlement still owns the queued successor");

  spy.calls[1].resolve({});
  await Promise.all([disposing, relocationDrain]);
  await registry.settle(["noteA"]);
});

test("a failed teardown admission stays registered and blocks every later settlement", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const registry = new saverModule.PerNoteSaveAdmissionRegistry();
  const spy = makeUpsertSpy();
  const fallbackError = new Error("teardown fallback failed");
  const saver = new BlockOpsSaver(
    spy.fn,
    async () => {
      throw fallbackError;
    },
    500,
    registry,
  );

  saver.enqueue("noteA", [upsert("bid-1", "uncertain")]);
  t.mock.timers.tick(500);
  const disposing = saver.dispose();
  spy.calls[0].reject(new Error("POST failed"));

  await assert.rejects(disposing, fallbackError);
  await assert.rejects(registry.settle(["noteA"]), fallbackError);
  await assert.rejects(
    registry.settle(["noteA"]),
    fallbackError,
    "failure ownership must survive the component that created it",
  );
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

test("settle flushes a queued request immediately and awaits its completion", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "queued")]);
  let settled = false;
  const completion = saver.settle("noteA").then(() => {
    settled = true;
  });

  assert.equal(spy.calls.length, 1, "settle bypasses the debounce timer");
  assert.equal(settled, false, "settle waits for the POST");
  spy.calls[0].resolve({});
  await completion;
  assert.equal(settled, true);

  t.mock.timers.tick(1000);
  assert.equal(spy.calls.length, 1, "the cancelled timer cannot double-send");
});

test("settle waits for an existing request without aborting it", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "live")]);
  t.mock.timers.tick(500);
  const completion = saver.settle("noteA");

  assert.equal(spy.calls.length, 1, "settle reuses the live request");
  assert.equal(spy.calls[0].aborted, false, "a live request is not used as an ordering abort");
  spy.calls[0].resolve({});
  await completion;
  assert.equal(spy.calls[0].aborted, false);
});

test("settle loops through an enqueue that arrives behind the live request", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "first")]);
  t.mock.timers.tick(500);
  const completion = saver.settle("noteA");
  saver.enqueue("noteA", [upsert("bid-1", "successor")]);

  t.mock.timers.tick(500);
  assert.equal(spy.calls.length, 1, "the successor debounce cannot race the settle barrier");
  assert.equal(spy.calls[0].aborted, false);
  spy.calls[0].resolve({});
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(spy.calls.length, 2, "the queued successor flushes after the live request");
  assert.equal(spy.calls[0].aborted, false, "settle never aborts the predecessor");
  assert.deepEqual(spy.calls[1].ops, [upsert("bid-1", "successor")]);

  spy.calls[1].resolve({});
  await completion;
});

test("a forced flush during settle cannot abort the live predecessor", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const saver = new BlockOpsSaver(spy.fn, t.mock.fn());

  saver.enqueue("noteA", [upsert("bid-1", "first")]);
  t.mock.timers.tick(500);
  const completion = saver.settle("noteA");
  saver.enqueue("noteA", [upsert("bid-1", "successor")]);

  // A blur/teardown-style forced flush can race the relocation barrier. It
  // must leave request 1 alive and let settle serialize request 2 behind it.
  saver.flush("noteA");
  assert.equal(spy.calls.length, 1, "forced flush leaves the successor queued");
  assert.equal(spy.calls[0].aborted, false, "the live predecessor is never aborted");

  spy.calls[0].resolve({});
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(spy.calls.length, 2, "settle starts request 2 only after request 1 resolves");
  assert.deepEqual(spy.calls[1].ops, [upsert("bid-1", "successor")]);
  spy.calls[1].resolve({});
  await completion;
});

test("settle awaits the Promise-capable whole-body fallback", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  let finishFallback;
  const fallback = t.mock.fn(() => new Promise((resolve) => {
    finishFallback = resolve;
  }));
  const saver = new BlockOpsSaver(spy.fn, fallback);

  saver.enqueue("noteA", [upsert("bid-1", "needs fallback")]);
  const completion = saver.settle("noteA");
  let settled = false;
  void completion.then(() => {
    settled = true;
  });
  spy.calls[0].reject(new Error("500 boom"));
  await Promise.resolve();
  await Promise.resolve();

  assert.equal(fallback.mock.callCount(), 1);
  assert.equal(settled, false, "settle remains pending until the fallback PUT finishes");
  finishFallback();
  await completion;
  assert.equal(settled, true);
});

test("settle rejects when the whole-body fallback fails", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallbackError = new Error("PUT failed");
  const saver = new BlockOpsSaver(spy.fn, async () => {
    throw fallbackError;
  });

  saver.enqueue("noteA", [upsert("bid-1", "cannot persist")]);
  const completion = saver.settle("noteA");
  spy.calls[0].reject(new Error("POST failed"));
  await assert.rejects(completion, fallbackError);
});

test("settle latches a fallback failure that completed before preflight", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const spy = makeUpsertSpy();
  const fallbackError = new Error("PUT failed before relocation");
  const saver = new BlockOpsSaver(spy.fn, async () => {
    throw fallbackError;
  });

  saver.enqueue("noteA", [upsert("bid-1", "uncertain")]);
  t.mock.timers.tick(500);
  spy.calls[0].reject(new Error("POST failed"));
  await new Promise((resolve) => setImmediate(resolve));

  await assert.rejects(saver.settle("noteA"), fallbackError);
  await assert.rejects(
    saver.settle("noteA"),
    fallbackError,
    "later move attempts must stay blocked until authoritative reconciliation",
  );
});
