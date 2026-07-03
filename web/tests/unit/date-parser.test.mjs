// Unit tests for the date-parser recurrence helpers.
// Covers the full grammar recognised by tesela-core::recurrence::parse —
// BYDAY day-sets, "weekends", and trailing " until YYYY-MM-DD" / " count N"
// end clauses — plus a regression guard for the original simple forms.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { parseRecurrenceInput, parseDateAndRecurrenceInput } from "../../src/lib/date-parser.ts";

test("parseRecurrenceInput — existing forms still parse", () => {
  assert.equal(parseRecurrenceInput("daily"), "daily");
  assert.equal(parseRecurrenceInput("every 2 weeks"), "every 2 weeks");
  assert.equal(parseRecurrenceInput("weekdays"), "weekdays");
  assert.equal(parseRecurrenceInput("garbage"), null);
});

test("parseRecurrenceInput — weekends", () => {
  assert.equal(parseRecurrenceInput("weekends"), "weekends");
});

test("parseRecurrenceInput — BYDAY day-sets", () => {
  assert.equal(parseRecurrenceInput("every mon, wed, fri"), "every mon, wed, fri");
  assert.equal(parseRecurrenceInput("every monday"), "every mon");
  assert.equal(parseRecurrenceInput("every fri, mon"), "every mon, fri");
  assert.equal(parseRecurrenceInput("every mon, blarg"), null);
});

test("parseRecurrenceInput — until / count end clauses", () => {
  assert.equal(parseRecurrenceInput("weekly until 2026-12-31"), "weekly until 2026-12-31");
  assert.equal(parseRecurrenceInput("every mon, fri count 12"), "every mon, fri count 12");
  assert.equal(parseRecurrenceInput("daily count 0"), null);
  assert.equal(parseRecurrenceInput("daily until not-a-date"), null);
  // Overflow calendar date — must reject, not roll forward.
  assert.equal(parseRecurrenceInput("daily until 2026-02-30"), null);
});

test("parseDateAndRecurrenceInput extracts an extended recurrence tail", () => {
  const r = parseDateAndRecurrenceInput("friday every mon, wed, fri count 8");
  assert.equal(r.recurrence, "every mon, wed, fri count 8");
});

test("parseDateAndRecurrenceInput — field keyword", () => {
  const fixed = new Date(2026, 4, 22); // Fri May 22 2026
  assert.equal(parseDateAndRecurrenceInput("deadline friday", fixed)?.field, "deadline");
  assert.equal(parseDateAndRecurrenceInput("scheduled tomorrow", fixed)?.field, "scheduled");
  assert.equal(parseDateAndRecurrenceInput("due may 1", fixed)?.field, "deadline");
  assert.equal(parseDateAndRecurrenceInput("tomorrow", fixed)?.field, null);
  const r = parseDateAndRecurrenceInput("deadline every day", fixed);
  assert.equal(r?.field, "deadline");
  assert.equal(r?.recurrence, "daily");
});

test("parseDateAndRecurrenceInput — keyword-less bare recurrence anchors to today", () => {
  const fixed = new Date(2026, 4, 22); // Fri May 22 2026
  // `every monday` typed alone: recurrence + today anchor, field null
  // (downstream routes a null field to the bareDateField default).
  const a = parseDateAndRecurrenceInput("every monday", fixed);
  assert.equal(a?.recurrence, "every mon");
  assert.equal(a?.field, null);
  assert.equal(a?.date, "2026-05-22");
  assert.equal(parseDateAndRecurrenceInput("weekdays", fixed)?.recurrence, "weekdays");
  assert.equal(parseDateAndRecurrenceInput("every 3 days", fixed)?.recurrence, "every 3 days");
});

test("parseDateAndRecurrenceInput rejects trailing recurrence with an unparseable prefix (tesela-fr1)", () => {
  // A trailing recurrence tail extracted off a longer phrase must NOT make
  // the whole phrase a "bare recurrence" match when the leading prose isn't
  // itself a date — otherwise the caller (detectTaskTokens/longestDateFrom)
  // treats the entire span, prose included, as consumed.
  assert.equal(parseDateAndRecurrenceInput("Call the doctor every sun"), null);
});
