import { test } from "node:test";
import assert from "node:assert/strict";
import { prevDate, dailyWalkDates, filterDisplayableDailies } from "../../src/lib/journal-dates.ts";

// The contract under test: JournalView's date walk must be bounded for EVERY
// relation between today and the on-disk window. The in-component
// predecessor (`while (true)` exiting only on `cursor === oldest`) never
// terminated when `oldest` was future-dated — the cursor only steps
// backward — hard-hanging the tab during render while allocating a synthetic
// Note per iteration.

test("prevDate steps back one day, across month and year boundaries", () => {
  assert.equal(prevDate("2026-06-10"), "2026-06-09");
  assert.equal(prevDate("2026-06-01"), "2026-05-31");
  assert.equal(prevDate("2026-01-01"), "2025-12-31");
  assert.equal(prevDate("2024-03-01"), "2024-02-29"); // leap day
});

test("normal case: today → oldest, gap-free descending", () => {
  const walk = dailyWalkDates("2026-06-10", "2026-06-08", "2026-06-05");
  assert.deepEqual(walk, [
    "2026-06-10",
    "2026-06-09",
    "2026-06-08",
    "2026-06-07",
    "2026-06-06",
    "2026-06-05",
  ]);
});

test("single on-disk daily == today", () => {
  assert.deepEqual(dailyWalkDates("2026-06-10", "2026-06-10", "2026-06-10"), ["2026-06-10"]);
});

test("EVERY daily future-dated: terminates and still includes today (the hang case)", () => {
  // A fresh mosaic whose only daily synced from a TZ-ahead peer: oldest is
  // tomorrow. The old loop walked backward from today forever.
  const walk = dailyWalkDates("2026-06-10", "2026-06-11", "2026-06-11");
  assert.deepEqual(walk, ["2026-06-11", "2026-06-10"]);
});

test("future newest + past oldest: the future daily renders instead of being dropped", () => {
  // Peer across the dateline created "tomorrow" while normal history exists.
  // The old walk started at today, so the future daily was silently dropped
  // by the dead post-loop guard.
  const walk = dailyWalkDates("2026-06-10", "2026-06-12", "2026-06-09");
  assert.deepEqual(walk, [
    "2026-06-12",
    "2026-06-11",
    "2026-06-10",
    "2026-06-09",
  ]);
});

test("far-future oldest stays bounded", () => {
  const walk = dailyWalkDates("2026-06-10", "2026-07-10", "2026-07-01");
  // start=2026-07-10 (newest), end=2026-06-10 (today): 31 days inclusive.
  assert.equal(walk.length, 31);
  assert.equal(walk[0], "2026-07-10");
  assert.equal(walk[walk.length - 1], "2026-06-10");
});

test("walk always contains today and every date in [oldest, newest]", () => {
  const cases = [
    ["2026-06-10", "2026-06-08", "2026-06-05"],
    ["2026-06-10", "2026-06-11", "2026-06-11"],
    ["2026-06-10", "2026-06-12", "2026-06-09"],
    ["2026-06-10", "2026-06-10", "2026-06-01"],
  ];
  for (const [today, newest, oldest] of cases) {
    const walk = dailyWalkDates(today, newest, oldest);
    assert.ok(walk.includes(today), `today missing for ${newest}/${oldest}`);
    assert.ok(walk.includes(newest), `newest missing for ${newest}/${oldest}`);
    assert.ok(walk.includes(oldest), `oldest missing for ${newest}/${oldest}`);
    // Strictly descending, gap-free.
    for (let i = 1; i < walk.length; i++) {
      assert.equal(walk[i], prevDate(walk[i - 1]));
    }
  }
});

test("blank future placeholder dailies are hidden from the default journal feed", () => {
  const notes = [
    { title: "2026-06-13", body: "- <!-- bid:019ebc57-e8a1-7fa1-9836-ff6c34dcbf07 -->" },
    { title: "2026-06-12", body: "- Today" },
    { title: "2026-06-11", body: "- Yesterday" },
  ];

  assert.deepEqual(
    filterDisplayableDailies("2026-06-12", notes).map((n) => n.title),
    ["2026-06-12", "2026-06-11"],
  );
});

test("contentful or explicitly anchored future dailies remain displayable", () => {
  const notes = [
    { title: "2026-06-14", body: "- Future task" },
    { title: "2026-06-13", body: "- <!-- bid:019ebc57-e8a1-7fa1-9836-ff6c34dcbf07 -->" },
    { title: "2026-06-12", body: "- Today" },
  ];

  assert.deepEqual(
    filterDisplayableDailies("2026-06-12", notes).map((n) => n.title),
    ["2026-06-14", "2026-06-12"],
  );
  assert.deepEqual(
    filterDisplayableDailies("2026-06-12", notes, "2026-06-13").map((n) => n.title),
    ["2026-06-14", "2026-06-13", "2026-06-12"],
  );
});
