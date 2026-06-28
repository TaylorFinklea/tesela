// Unit tests for the remote-cursors presence store (Phase 2 desktop presence).
import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  applyPresenceFrame,
  remoteCursorsForBlock,
  pruneStale,
  subscribeRemoteCursors,
  localPeerId,
  colorForPeer,
  _resetForTest,
} from "../../src/lib/remote-cursors.ts";

function frame(over = {}) {
  return {
    peer: "peer-other",
    color: "#3b82f6",
    name: "B",
    slug: "2026-06-27",
    bid: "bid-1",
    offset: 4,
    ...over,
  };
}

test("applyPresenceFrame stores a peer's cursor, readable by block", () => {
  _resetForTest();
  applyPresenceFrame(frame(), 1000);
  const got = remoteCursorsForBlock("2026-06-27", "bid-1", 1000);
  assert.equal(got.length, 1);
  assert.equal(got[0].peer, "peer-other");
  assert.equal(got[0].offset, 4);
});

test("cursors are filtered by slug + bid", () => {
  _resetForTest();
  applyPresenceFrame(frame({ bid: "bid-1" }), 1000);
  applyPresenceFrame(frame({ peer: "p2", bid: "bid-2" }), 1000);
  assert.equal(remoteCursorsForBlock("2026-06-27", "bid-1", 1000).length, 1);
  assert.equal(remoteCursorsForBlock("2026-06-27", "bid-2", 1000).length, 1);
  assert.equal(remoteCursorsForBlock("other-note", "bid-1", 1000).length, 0);
});

test("a peer moving from one block to another relocates its single cursor", () => {
  _resetForTest();
  applyPresenceFrame(frame({ bid: "bid-1" }), 1000);
  applyPresenceFrame(frame({ bid: "bid-2", offset: 9 }), 1100); // same peer moved
  assert.equal(remoteCursorsForBlock("2026-06-27", "bid-1", 1100).length, 0, "left old block");
  const now = remoteCursorsForBlock("2026-06-27", "bid-2", 1100);
  assert.equal(now.length, 1);
  assert.equal(now[0].offset, 9);
});

test("our own peer's frames are ignored (no self-cursor)", () => {
  _resetForTest();
  applyPresenceFrame(frame({ peer: localPeerId() }), 1000);
  assert.equal(remoteCursorsForBlock("2026-06-27", "bid-1", 1000).length, 0);
});

test("stale cursors expire and prune", () => {
  _resetForTest();
  applyPresenceFrame(frame(), 1000);
  assert.equal(remoteCursorsForBlock("2026-06-27", "bid-1", 1000 + 9_000).length, 1, "fresh < 10s");
  assert.equal(remoteCursorsForBlock("2026-06-27", "bid-1", 1000 + 11_000).length, 0, "stale > 10s");
  assert.equal(pruneStale(1000 + 11_000), true, "prune reports a removal");
  assert.equal(pruneStale(1000 + 11_000), false, "nothing left to prune");
});

test("subscribers fire on apply", () => {
  _resetForTest();
  let hits = 0;
  const unsub = subscribeRemoteCursors(() => hits++);
  applyPresenceFrame(frame(), 1000);
  assert.equal(hits, 1);
  unsub();
  applyPresenceFrame(frame({ peer: "p3" }), 1000);
  assert.equal(hits, 1, "no more after unsub");
});

test("colorForPeer is deterministic per peer", () => {
  assert.equal(colorForPeer("abc"), colorForPeer("abc"));
});
