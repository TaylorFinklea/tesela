import { test } from "node:test";
import assert from "node:assert/strict";
import { formatDateMonthDay } from "../../src/lib/date-format.ts";

test("formatDateMonthDay — bare ISO and bracketed", () => {
  const yr = new Date().getFullYear();
  assert.equal(formatDateMonthDay(`${yr}-05-22`), "May 22");
  assert.equal(formatDateMonthDay(`[[${yr}-05-22]]`), "May 22");
  assert.equal(formatDateMonthDay("2025-12-31"), "Dec 31, 2025");
  assert.equal(formatDateMonthDay(`${yr}-05-22 15:30`), "May 22 3:30p");
  assert.equal(formatDateMonthDay("not-a-date"), "not-a-date");
});
