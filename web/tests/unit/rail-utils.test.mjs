import assert from "node:assert/strict";
import test from "node:test";

const { agendaQueryKey, agendaRange, splitRailTasks, railTaskLabel } = await import(
  "../../src/lib/graphite/rail-utils.ts"
);

test("agendaRange matches the Agenda lookback and forward window", () => {
  const range = agendaRange(new Date(2026, 6, 10, 12), 60);

  assert.deepEqual(range, { from: "2026-04-11", to: "2026-09-08" });
});

test("agendaQueryKey uses the shared Agenda cache shape", () => {
  assert.deepEqual(agendaQueryKey("2026-04-11", "2026-09-08"), [
    "agenda",
    { from: "2026-04-11", to: "2026-09-08", includeDone: false },
  ]);
});

test("splitRailTasks keeps open tasks and separates doing from next", () => {
  const rows = [
    { kind: "task", status: "doing", text: "Ship the rail" },
    { kind: "task", status: "todo", text: "Write tests" },
    { kind: "task", status: "done", text: "Old task" },
    { kind: "event", status: null, text: "A meeting" },
  ];

  const buckets = splitRailTasks(rows);

  assert.deepEqual(buckets.doing.map((row) => row.text), ["Ship the rail"]);
  assert.deepEqual(buckets.next.map((row) => row.text), ["Write tests"]);
  assert.equal(buckets.total, 2);
});

test("railTaskLabel gives empty task rows a useful fallback", () => {
  assert.equal(railTaskLabel({ text: "Call Taylor" }), "Call Taylor");
  assert.equal(railTaskLabel({ text: "" }), "(untitled task)");
});
