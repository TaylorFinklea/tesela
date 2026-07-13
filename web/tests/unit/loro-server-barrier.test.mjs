import { test } from "node:test";
import { strict as assert } from "node:assert";
import { readFileSync } from "node:fs";

import {
  runServerBarrierTransaction,
  ServerBarrierTracker,
} from "../../src/lib/loro/server-barrier.ts";
import * as serverBarrierModule from "../../src/lib/loro/server-barrier.ts";

const BARRIER_ID = "11111111-1111-4111-8111-111111111111";
const wsClientSource = readFileSync(
  new URL("../../src/lib/ws-client.svelte.ts", import.meta.url),
  "utf8",
);

function makeHarness() {
  const sent = [];
  const timers = [];
  const socket = { open: true };
  const tracker = new ServerBarrierTracker({
    createId: () => BARRIER_ID,
    isOpen: (candidate) => candidate.open,
    sendText: (_candidate, text) => {
      sent.push(text);
      return true;
    },
    scheduleTimeout: (cb) => {
      const handle = { cb, cleared: false };
      timers.push(handle);
      return handle;
    },
    clearTimeout: (handle) => {
      handle.cleared = true;
    },
  });
  return { tracker, socket, sent, timers };
}

test("request sends one UUID-tagged control frame and waits for the exact positive ack", async () => {
  const { tracker, socket, sent } = makeHarness();
  let settled = false;
  const pending = tracker.request(socket, 7).then(() => {
    settled = true;
  });

  assert.deepEqual(JSON.parse(sent[0]), { event: "loro_barrier", barrier_id: BARRIER_ID });
  await Promise.resolve();
  assert.equal(settled, false);

  assert.equal(tracker.handleAcknowledgement(socket, 7, {
    event: "loro_barrier_ack",
    barrier_id: "22222222-2222-4222-8222-222222222222",
    ok: true,
  }), false, "a mismatched id is inert");
  await Promise.resolve();
  assert.equal(settled, false);

  assert.equal(tracker.handleAcknowledgement(socket, 7, {
    event: "loro_barrier_ack",
    barrier_id: BARRIER_ID,
    ok: true,
  }), true);
  await pending;
  assert.equal(settled, true);
  assert.equal(tracker.pendingCount(), 0);
});

test("a negative acknowledgement rejects and clears the request", async () => {
  const { tracker, socket } = makeHarness();
  const pending = tracker.request(socket, 1);
  tracker.handleAcknowledgement(socket, 1, {
    event: "loro_barrier_ack",
    barrier_id: BARRIER_ID,
    ok: false,
  });
  await assert.rejects(pending, /server rejected/i);
  assert.equal(tracker.pendingCount(), 0);
});

test("timeout rejects and clears the request", async () => {
  const { tracker, socket, timers } = makeHarness();
  const pending = tracker.request(socket, 1);
  timers[0].cb();
  await assert.rejects(pending, /timed out/i);
  assert.equal(tracker.pendingCount(), 0);
});

test("socket close rejects every request from that generation", async () => {
  const { tracker, socket } = makeHarness();
  const pending = tracker.request(socket, 3);
  tracker.rejectConnection(socket, 3, new Error("socket closed"));
  await assert.rejects(pending, /socket closed/i);
  assert.equal(tracker.pendingCount(), 0);
});

test("unavailable or dropped sends reject without retaining state", async () => {
  const unavailable = makeHarness();
  unavailable.socket.open = false;
  await assert.rejects(unavailable.tracker.request(unavailable.socket, 1), /not open/i);
  assert.equal(unavailable.tracker.pendingCount(), 0);

  const dropped = makeHarness();
  const rejectingTracker = new ServerBarrierTracker({
    createId: () => BARRIER_ID,
    isOpen: () => true,
    sendText: () => false,
    scheduleTimeout: (cb) => ({ cb }),
    clearTimeout: () => {},
  });
  await assert.rejects(rejectingTracker.request(dropped.socket, 1), /send/i);
  assert.equal(rejectingTracker.pendingCount(), 0);
});

test("an acknowledgement from a stale socket generation rejects rather than resolving", async () => {
  const { tracker, socket } = makeHarness();
  const pending = tracker.request(socket, 4);
  tracker.handleAcknowledgement(socket, 5, {
    event: "loro_barrier_ack",
    barrier_id: BARRIER_ID,
    ok: true,
  });
  tracker.rejectConnection(socket, 4, new Error("connection replaced"));
  await assert.rejects(pending, /connection replaced/i);
});

test("negative acknowledgement rejects the prepared registry transaction", async () => {
  const { tracker, socket } = makeHarness();
  const calls = [];
  const completion = runServerBarrierTransaction({
    prepare: () => ({
      acknowledge: () => calls.push("acknowledge"),
      reject: () => calls.push("reject"),
    }),
    isConnectionCurrent: () => true,
    request: () => tracker.request(socket, 1),
  });

  tracker.handleAcknowledgement(socket, 1, {
    event: "loro_barrier_ack",
    barrier_id: BARRIER_ID,
    ok: false,
  });
  await assert.rejects(completion, /server rejected/i);
  assert.deepEqual(calls, ["reject"]);
});

test("barrier timeout rejects the prepared registry transaction", async () => {
  const { tracker, socket, timers } = makeHarness();
  const calls = [];
  const completion = runServerBarrierTransaction({
    prepare: () => ({
      acknowledge: () => calls.push("acknowledge"),
      reject: () => calls.push("reject"),
    }),
    isConnectionCurrent: () => true,
    request: () => tracker.request(socket, 1),
  });

  timers[0].cb();
  await assert.rejects(completion, /timed out/i);
  assert.deepEqual(calls, ["reject"]);
});

test("connection change during preparation rolls back before requesting a barrier", async () => {
  let current = true;
  let requested = false;
  const calls = [];
  await assert.rejects(
    runServerBarrierTransaction({
      prepare: () => {
        current = false;
        return {
          acknowledge: () => calls.push("acknowledge"),
          reject: () => calls.push("reject"),
        };
      },
      isConnectionCurrent: () => current,
      request: async () => {
        requested = true;
      },
    }),
    /changed during Loro barrier preparation/i,
  );
  assert.equal(requested, false);
  assert.deepEqual(calls, ["reject"]);
});

test("the WebSocket wrapper prepares documents even when no socket is available", () => {
  const start = wsClientSource.indexOf("export async function awaitLoroServerBarrier");
  const source = wsClientSource.slice(start, wsClientSource.indexOf("\n}", start) + 2);
  const prepare = source.indexOf("prepare:");
  const availability = source.indexOf("capturedSocket");

  assert.ok(prepare >= 0 && availability >= 0);
  assert.doesNotMatch(
    source.slice(0, prepare),
    /throw new Error\("Loro barrier socket is not open"\)/,
    "the registry transaction must prepare and roll back before socket failure escapes",
  );
  assert.match(source, /if \(!capturedSocket[^)]*\) return false/);
});

test("post-capture document changes reject an otherwise positive server acknowledgement", async () => {
  const calls = [];
  await assert.rejects(
    runServerBarrierTransaction({
      prepare: () => ({
        acknowledge: () => {
          calls.push("acknowledge");
          return false;
        },
        reject: () => calls.push("reject"),
      }),
      isConnectionCurrent: () => true,
      request: async () => {},
    }),
    /document changed while the server barrier was pending/i,
  );
  assert.deepEqual(calls, ["acknowledge"]);
});

test("barrier proof retries isolate keys and back off only the failures", async () => {
  assert.equal(
    typeof serverBarrierModule.ServerBarrierRetryQueue,
    "function",
    "the barrier module exposes the durability retry coordinator",
  );

  const scheduled = [];
  const attempts = [];
  let alphaFailures = 2;
  const queue = new serverBarrierModule.ServerBarrierRetryQueue({
    initialDelayMs: 10,
    maxDelayMs: 40,
    schedule: (cb, delayMs) => {
      const handle = { cb, delayMs, cancelled: false };
      scheduled.push(handle);
      return handle;
    },
    cancelSchedule: (handle) => {
      handle.cancelled = true;
    },
    run: async (keys) => {
      attempts.push([...keys]);
      assert.equal(keys.length, 1, "each proof is isolated from poisoned peers");
      if (keys[0] === "alpha" && alphaFailures > 0) {
        alphaFailures -= 1;
        throw new Error("socket unavailable");
      }
    },
  });

  queue.enqueue(["alpha"]);
  queue.enqueue(["beta", "alpha"]);
  assert.equal(scheduled.length, 1, "same-turn requests share one timer");
  assert.equal(scheduled[0].delayMs, 10);

  scheduled.shift().cb();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(attempts, [["alpha"], ["beta"]]);
  assert.equal(scheduled.length, 1);
  assert.equal(scheduled[0].delayMs, 20, "a failed proof backs off");

  queue.enqueue(["gamma"]);

  scheduled.shift().cb();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(attempts, [["alpha"], ["beta"], ["alpha"], ["gamma"]]);
  assert.equal(scheduled.length, 1, "only the poisoned key remains pending");
  assert.equal(scheduled[0].delayMs, 40);

  scheduled.shift().cb();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(
    attempts,
    [["alpha"], ["beta"], ["alpha"], ["gamma"], ["alpha"]],
  );
  assert.equal(scheduled.length, 0, "success drains the pending proof set");

  queue.enqueue(["gamma"]);
  assert.equal(scheduled.length, 1);
  assert.equal(scheduled[0].delayMs, 10, "success resets the backoff");
  queue.resolve(["gamma"]);
  assert.equal(scheduled[0].cancelled, true, "external proof cancels an obsolete retry timer");
});

test("external proof resets retry backoff once the queue is empty", async () => {
  const scheduled = [];
  const queue = new serverBarrierModule.ServerBarrierRetryQueue({
    initialDelayMs: 10,
    maxDelayMs: 40,
    schedule: (cb, delayMs) => {
      const handle = { cb, delayMs, cancelled: false };
      scheduled.push(handle);
      return handle;
    },
    cancelSchedule: (handle) => {
      handle.cancelled = true;
    },
    run: async () => {
      throw new Error("socket unavailable");
    },
  });

  queue.enqueue(["alpha"]);
  scheduled.shift().cb();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(scheduled[0].delayMs, 20);

  queue.resolve(["alpha"]);
  assert.equal(scheduled.shift().cancelled, true);
  queue.enqueue(["beta"]);
  assert.equal(scheduled[0].delayMs, 10);
});
