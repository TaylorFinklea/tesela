// Unit tests for the recurrence formatter.
// Covers human-readable rendering of `recurring::` values.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { formatRecurrence } from "../../src/lib/recurrence-format.ts";

test("formatRecurrence — simple + every-N", () => {
  assert.equal(formatRecurrence("daily"), "Daily");
  assert.equal(formatRecurrence("every 2 weeks"), "Every 2 weeks");
  assert.equal(formatRecurrence("weekdays"), "Weekdays");
  assert.equal(formatRecurrence("weekends"), "Weekends");
});
test("formatRecurrence — BYDAY", () => {
  assert.equal(formatRecurrence("every mon, wed, fri"), "Mon, Wed, Fri");
});
test("formatRecurrence — end clauses", () => {
  assert.equal(formatRecurrence("weekly until 2026-12-31"), "Weekly until Dec 31, 2026");
  assert.equal(formatRecurrence("daily count 10"), "Daily, 10×");
});
test("formatRecurrence — unrecognized passes through", () => {
  assert.equal(formatRecurrence("blarg"), "blarg");
});
