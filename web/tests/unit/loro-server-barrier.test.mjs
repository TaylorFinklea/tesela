import { test } from "node:test";
import { strict as assert } from "node:assert";

import { ServerBarrierTracker } from "../../src/lib/loro/server-barrier.ts";

const BARRIER_ID = "11111111-1111-4111-8111-111111111111";

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
