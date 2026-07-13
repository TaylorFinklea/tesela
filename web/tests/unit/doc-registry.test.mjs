// Unit tests for the multi-note Loro doc registry core (tesela-baa).
//
// The registry replaces the single "active" NoteDoc: every mounted editor
// surface ref-counts its note's doc open, splices route by slug, inbound
// TLR2 updates route by 16-byte note id, and vim undo/redo follow the
// focused editor's note. Drives the pure core (doc-registry.ts) with fake
// docs + a manual flush scheduler — no wasm, no WS.
//
// The fake doc models the two properties the review pass caught the first
// cut violating: exportSince(null) returns the doc's FULL history (bootstrap
// ops included — a null-cursor flush of a merely-viewed note would blast
// megabytes), and send() can be dropped (WS not open) in which case the
// outbound cursor must NOT advance or the ops strand forever.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { NoteDocRegistry } from "../../src/lib/loro/doc-registry.ts";

const BOOTSTRAP_OPS = 5;

/** Fake RegistryDoc: `history` counts ops (bootstrap seeds 5); versions are
 *  plain numbers; exportSince(v) returns one byte per op newer than v, and
 *  exportSince(null) returns the FULL history. id16 derives from the slug's
 *  first char so tests can route inbound updates. */
function makeFakeDoc(log) {
  return {
    slug: null,
    noteId16: null,
    history: 0,
    open(slug) {
      this.slug = slug;
      this.noteId16 = new Uint8Array(16).fill(slug.charCodeAt(0));
      this.history = BOOTSTRAP_OPS;
      log.push(`open:${slug}`);
      return Promise.resolve();
    },
    close() {
      log.push(`close:${this.slug}`);
    },
    spliceBlock(bid, off, del, ins) {
      if (bid === "missing") return false;
      this.history += 1;
      log.push(`splice:${this.slug}:${bid}:${off}:${del}:${ins}`);
      return true;
    },
    applyInbound(updates) {
      // Imported remote ops advance the doc version like real Loro imports.
      this.history += updates.length;
      log.push(`inbound:${this.slug}:${updates.length}`);
    },
    exportSince(since) {
      const from = since ?? 0;
      return new Uint8Array(Math.max(0, this.history - from));
    },
    currentVersion() {
      return this.history;
    },
    undo() {
      log.push(`undo:${this.slug}`);
      this.history += 1; // the inverse op
      return true;
    },
    redo() {
      log.push(`redo:${this.slug}`);
      return true;
    },
    canUndo() {
      return true;
    },
    canRedo() {
      return false;
    },
  };
}

/** Registry + manual flush scheduler + droppable send (wsUp=false models a
 *  closed socket: the frame is discarded and send returns false). */
function makeHarness() {
  const log = [];
  const sent = [];
  const scheduled = [];
  const state = { wsUp: true };
  const registry = new NoteDocRegistry({
    createDoc: () => makeFakeDoc(log),
    scheduleFlush: (cb) => {
      const handle = { cb, cancelled: false };
      scheduled.push(handle);
      return handle;
    },
    cancelFlush: (handle) => {
      handle.cancelled = true;
    },
    send: (u) => {
      if (!state.wsUp) return false;
      sent.push(u);
      return true;
    },
  });
  const runFlushes = () => {
    while (scheduled.length) {
      const h = scheduled.shift();
      if (!h.cancelled) h.cb();
    }
  };
  return { registry, log, sent, runFlushes, state };
}

test("acquire/release ref-counts; doc closes only at zero refs", async () => {
  const { registry, log } = makeHarness();
  await registry.acquire("2026-07-02");
  await registry.acquire("2026-07-02");
  assert.equal(registry.refs("2026-07-02"), 2);
  assert.deepEqual(log, ["open:2026-07-02"]); // second acquire = refs++ only

  registry.release("2026-07-02");
  assert.equal(registry.size(), 1); // still one holder
  registry.release("2026-07-02");
  assert.equal(registry.size(), 0);
  assert.ok(log.includes("close:2026-07-02"));
});

test("a merely-VIEWED doc ships nothing on release (no full-history blast)", async () => {
  const { registry, sent } = makeHarness();
  await registry.acquire("viewed");
  registry.release("viewed");
  registry.flushAll();
  assert.equal(sent.length, 0);
  assert.equal(registry.size(), 0);
});

test("remote-only imports never trigger an outbound re-broadcast", async () => {
  const { registry, sent } = makeHarness();
  await registry.acquire("alpha");
  const inbound = { doc: new Uint8Array(16).fill("a".charCodeAt(0)), updateBytes: new Uint8Array([9]) };
  registry.applyInbound([inbound]);
  registry.release("alpha");
  registry.flushAll();
  assert.equal(sent.length, 0);
});

test("splice ships ONLY ops since the post-bootstrap baseline, routed by slug", async () => {
  const { registry, log, sent, runFlushes } = makeHarness();
  await registry.acquire("2026-07-02");
  await registry.acquire("2026-07-03");
  assert.equal(registry.size(), 2);

  assert.equal(registry.splice("2026-07-02", "bid-a", 3, 0, "x"), true);
  runFlushes();
  assert.ok(log.includes("splice:2026-07-02:bid-a:3:0:x"));
  assert.equal(sent.length, 1);
  assert.equal(sent[0].doc[0], "2".charCodeAt(0));
  assert.equal(sent[0].updateBytes.length, 1); // the splice, NOT the 5 bootstrap ops

  // Unknown slug and unbound block both fall back (false), nothing shipped.
  assert.equal(registry.splice("nope", "bid-a", 0, 0, "y"), false);
  assert.equal(registry.splice("2026-07-03", "missing", 0, 0, "y"), false);
  runFlushes();
  assert.equal(sent.length, 1);
});

test("burst splices coalesce into one scheduled flush; cursor advances", async () => {
  const { registry, sent, runFlushes } = makeHarness();
  await registry.acquire("n");
  registry.splice("n", "b", 0, 0, "a");
  registry.splice("n", "b", 1, 0, "b");
  registry.splice("n", "b", 2, 0, "c");
  runFlushes();
  assert.equal(sent.length, 1); // three splices, one send
  assert.equal(sent[0].updateBytes.length, 3);

  // Nothing new since the cursor advanced → flush is a no-op.
  registry.flush("n");
  assert.equal(sent.length, 1);
});

test("release flushes pending ops before closing (no lost last keystroke)", async () => {
  const { registry, sent, log } = makeHarness();
  await registry.acquire("n");
  registry.splice("n", "b", 0, 0, "z");
  // Release BEFORE the scheduled flush fires: the final flush must ship it.
  registry.release("n");
  assert.equal(sent.length, 1);
  assert.ok(log.includes("close:n"));
});

test("dropped send (WS down) does not advance the cursor; ops re-ship after reconnect", async () => {
  const { registry, sent, runFlushes, state } = makeHarness();
  await registry.acquire("n");
  registry.splice("n", "b", 0, 0, "z");
  state.wsUp = false;
  runFlushes(); // frame dropped, cursor must stay put
  assert.equal(sent.length, 0);

  state.wsUp = true;
  registry.flushAll(); // the reconnect hook
  assert.equal(sent.length, 1);
  assert.equal(sent[0].updateBytes.length, 1); // the stranded op, recovered
});

test("release while WS is down PARKS the doc; reconnect flush ships and closes it", async () => {
  const { registry, sent, log, state } = makeHarness();
  await registry.acquire("n");
  registry.splice("n", "b", 0, 0, "z");
  state.wsUp = false;
  registry.release("n");
  // Unsent local ops are the only copy — the doc must survive the release.
  assert.equal(registry.size(), 1);
  assert.ok(!log.includes("close:n"));
  assert.equal(sent.length, 0);

  state.wsUp = true;
  registry.flushAll();
  assert.equal(sent.length, 1);
  assert.equal(registry.size(), 0); // parked entry drained + closed
  assert.ok(log.includes("close:n"));
});

test("a parked doc resurrects on re-acquire", async () => {
  const { registry, state } = makeHarness();
  await registry.acquire("n");
  registry.splice("n", "b", 0, 0, "z");
  state.wsUp = false;
  registry.release("n");
  assert.equal(registry.size(), 1);

  await registry.acquire("n"); // navigate back while still offline
  assert.equal(registry.refs("n"), 1);
  assert.equal(registry.size(), 1);
});

test("inbound updates route by note id; unmatched are returned", async () => {
  const { registry, log } = makeHarness();
  await registry.acquire("alpha");
  await registry.acquire("beta");

  const forAlpha = { doc: new Uint8Array(16).fill("a".charCodeAt(0)), updateBytes: new Uint8Array([9]) };
  const forNobody = { doc: new Uint8Array(16).fill(0xff), updateBytes: new Uint8Array([9]) };
  const unmatched = registry.applyInbound([forAlpha, forNobody]);

  assert.ok(log.includes("inbound:alpha:1"));
  assert.ok(!log.some((l) => l.startsWith("inbound:beta")));
  assert.deepEqual(unmatched, [forNobody]);
});

test("undo routes to the focused editor's note; flag clears after a microtask", async () => {
  const { registry, log } = makeHarness();
  await registry.acquire("one");
  await registry.acquire("two");
  registry.splice("one", "b", 0, 0, "x");
  registry.splice("two", "b", 0, 0, "y");

  registry.setFocused("editor-2", "two");
  assert.equal(registry.undoFocused(), true);
  assert.ok(log.includes("undo:two"));
  assert.ok(!log.includes("undo:one"));
  assert.equal(registry.isUndoApplying(), true); // still set synchronously
  await Promise.resolve(); // drain microtasks
  assert.equal(registry.isUndoApplying(), false);
});

test("a late blur from a previous editor cannot clobber a fresh focus", async () => {
  const { registry } = makeHarness();
  await registry.acquire("one");
  await registry.acquire("two");
  registry.setFocused("editor-1", "one");
  registry.setFocused("editor-2", "two"); // focus moved
  registry.clearFocused("editor-1"); // stale blur arrives late
  assert.equal(registry.focusedSlug(), "two");
});

test("undo with nothing focused is a safe no-op (structural undo proceeds)", async () => {
  const { registry } = makeHarness();
  await registry.acquire("one");
  assert.equal(registry.undoFocused(), false);
  assert.equal(registry.canUndoFocused(), false);
});

test("releasing the focused note clears focus routing", async () => {
  const { registry } = makeHarness();
  await registry.acquire("one");
  registry.setFocused("e", "one");
  registry.release("one");
  assert.equal(registry.focusedSlug(), null);
});

test("forced flush cancels its scheduled callback and reports the real handoff result", async () => {
  const { registry, sent, runFlushes, state } = makeHarness();
  await registry.acquire("n");
  registry.splice("n", "b", 0, 0, "x");

  state.wsUp = false;
  assert.equal(registry.flush("n"), false, "a dirty delta rejected by the socket is unsettled");
  assert.equal(sent.length, 0);

  state.wsUp = true;
  assert.equal(registry.flush("n"), true, "a real socket handoff settles the dirty delta");
  assert.equal(sent.length, 1);

  runFlushes();
  assert.equal(sent.length, 1, "the cancelled scheduled callback cannot send a second frame");
});

test("barrier retry re-exports cumulatively from the server-acked checkpoint", async () => {
  const { registry, sent, runFlushes } = makeHarness();
  await registry.acquire("n");

  registry.splice("n", "b", 0, 0, "a");
  runFlushes();
  assert.equal(sent.length, 1, "ordinary handoff advances only the optimistic cursor");

  const barrierFrames = [];
  const handoff = (update) => {
    barrierFrames.push(update);
    return true;
  };
  const first = registry.prepareServerBarrier(["n", "n"], handoff);
  assert.ok(first, "duplicate affected notes prepare one successful barrier handoff");
  assert.equal(barrierFrames.length, 1);
  assert.equal(barrierFrames[0].updateBytes.length, 1);

  // No acknowledgement: the same op remains cumulative. Add another edit
  // after the failed barrier and prove the retry carries both operations.
  registry.splice("n", "b", 1, 0, "b");
  const retry = registry.prepareServerBarrier(["n"], handoff);
  assert.ok(retry);
  assert.equal(barrierFrames.length, 2);
  assert.equal(barrierFrames[1].updateBytes.length, 2, "retry includes every op since last ack");
  retry.acknowledge();

  registry.splice("n", "b", 2, 0, "c");
  const afterAck = registry.prepareServerBarrier(["n"], handoff);
  assert.ok(afterAck);
  assert.equal(barrierFrames[2].updateBytes.length, 1, "positive ack advances only its captured VV");
});

test("failed barrier handoff does not advance the server checkpoint", async () => {
  const { registry } = makeHarness();
  await registry.acquire("n");
  registry.splice("n", "b", 0, 0, "x");

  const dropped = [];
  const failed = registry.prepareServerBarrier(["n"], (update) => {
    dropped.push(update);
    return false;
  });
  assert.equal(failed, null);
  assert.equal(dropped[0].updateBytes.length, 1);

  const retried = [];
  const retry = registry.prepareServerBarrier(["n"], (update) => {
    retried.push(update);
    return true;
  });
  assert.ok(retry);
  assert.equal(retried[0].updateBytes.length, 1, "dropped bytes remain unacknowledged");
});

test("barrier callers can await every affected doc's completed bootstrap", async () => {
  let finishOpen;
  const doc = makeFakeDoc([]);
  doc.open = (slug) => {
    doc.slug = slug;
    doc.noteId16 = new Uint8Array(16).fill(slug.charCodeAt(0));
    doc.history = BOOTSTRAP_OPS;
    return new Promise((resolve) => {
      finishOpen = resolve;
    });
  };
  const registry = new NoteDocRegistry({
    createDoc: () => doc,
    scheduleFlush: () => null,
    cancelFlush: () => {},
    send: () => true,
  });
  const acquired = registry.acquire("n");
  let ready = false;
  const waiting = registry.waitUntilOpen(["n", "n"]).then(() => {
    ready = true;
  });
  await Promise.resolve();
  assert.equal(ready, false);
  finishOpen();
  await acquired;
  await waiting;
  assert.equal(ready, true);
});

test("barrier preparation fails closed for an affected real note that is not open", async () => {
  const { registry } = makeHarness();
  await assert.rejects(registry.waitUntilOpen(["missing"]), /not open/i);
  assert.equal(registry.prepareServerBarrier(["missing"], () => true), null);
});
