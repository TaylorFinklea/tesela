// Unit tests for the PRES presence frame codec (Phase 2 desktop presence).
//
// A presence frame is `b"PRES"` ++ utf8(JSON(PresenceFrame)) — distinct from
// the TLR2 Loro-delta magic so the WS dispatcher (server + web) can route it to
// the remote-cursor store instead of the engine.
import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  encodePresence,
  decodePresence,
  isPresenceFrame,
} from "../../src/lib/loro/presence.ts";

test("presence frame round-trips through encode/decode", () => {
  const f = {
    peer: "p1",
    color: "#ff8800",
    name: "Tab A",
    slug: "2026-06-27",
    bid: "abababab-abab-abab-abab-abababababab",
    offset: 7,
  };
  const bytes = encodePresence(f);
  assert.ok(isPresenceFrame(bytes), "carries the PRES magic");
  assert.deepEqual(decodePresence(bytes), f);
});

test("decodePresence rejects a non-PRES frame (e.g. a TLR2 delta)", () => {
  const tlr2 = new Uint8Array([0x54, 0x4c, 0x52, 0x32, 1, 2, 3]); // "TLR2"…
  assert.equal(decodePresence(tlr2), null);
  assert.equal(isPresenceFrame(tlr2), false);
});

test("decodePresence rejects garbage / malformed JSON after the magic", () => {
  const bad = new Uint8Array([0x50, 0x52, 0x45, 0x53, 0x7b, 0xff]); // PRES + "{"+invalid
  assert.equal(decodePresence(bad), null);
});

test("decodePresence rejects a frame missing required fields", () => {
  const partial = new TextEncoder().encode(JSON.stringify({ peer: "p1" }));
  const frame = new Uint8Array(4 + partial.length);
  frame.set([0x50, 0x52, 0x45, 0x53], 0);
  frame.set(partial, 4);
  assert.equal(decodePresence(frame), null, "needs bid + offset, not just peer");
});
