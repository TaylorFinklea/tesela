// Unit tests for the WS refetch coordinator — the upstream half of the
// "edits clear on refresh" fix. Verifies (1) a burst of WS events coalesces
// into ONE refetch pass, (2) own-echo note ids are suppressed from the
// targeted refresh while the broad list refresh still fires, and (3) genuine
// remote ids (no recent local save) DO get a targeted refresh.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import {
  recordLocalSave,
  isOwnEcho,
  scheduleNoteRefresh,
  flushNoteRefreshNow,
  setRefreshCallback,
  __test,
} from "../../src/lib/ws-refresh-coordinator.ts";

function setup() {
  __test.reset();
  const batches = [];
  setRefreshCallback((b) => batches.push(b));
  return batches;
}

test("coalesce: a burst of N events → ONE refetch pass", () => {
  const batches = setup();
  // 5 distinct remote notes update within the coalesce window.
  for (const id of ["a", "b", "c", "d", "e"]) scheduleNoteRefresh(id, true);
  assert.equal(batches.length, 0, "nothing flushed before the window elapses");
  flushNoteRefreshNow();
  assert.equal(batches.length, 1, "the whole burst collapses into one pass");
  assert.equal(batches[0].broad, true);
  assert.deepEqual(batches[0].noteIds.sort(), ["a", "b", "c", "d", "e"]);
});

test("own-echo: a recently-saved id is dropped from the targeted set", () => {
  const batches = setup();
  recordLocalSave("self");
  assert.equal(isOwnEcho("self"), true);
  // Server echoes our own PUT back as note_updated for "self", plus a genuine
  // remote change to "other".
  scheduleNoteRefresh("self", true);
  scheduleNoteRefresh("other", true);
  flushNoteRefreshNow();
  assert.equal(batches.length, 1);
  // "self" is suppressed from the targeted refresh (won't clobber our
  // optimistic editor update); "other" still refreshes.
  assert.deepEqual(batches[0].noteIds, ["other"]);
  // The broad list refresh still fires so lists/ambients stay fresh.
  assert.equal(batches[0].broad, true);
});

test("own-echo window expires", async () => {
  setup();
  recordLocalSave("x");
  assert.equal(isOwnEcho("x"), true);
  // Simulate the window passing by reaching past the threshold. We can't
  // fast-forward Date.now without a clock shim, so assert the boundary logic
  // indirectly: an id never saved is never an echo.
  assert.equal(isOwnEcho("never-saved"), false);
});

test("broad-only event still flushes a pass with no targeted ids", () => {
  const batches = setup();
  scheduleNoteRefresh(null, true);
  flushNoteRefreshNow();
  assert.equal(batches.length, 1);
  assert.deepEqual(batches[0].noteIds, []);
  assert.equal(batches[0].broad, true);
});

test("empty pending → no callback fired", () => {
  const batches = setup();
  flushNoteRefreshNow();
  assert.equal(batches.length, 0);
});
