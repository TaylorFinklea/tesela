// Prism v5 scratch-prune predicate tests.

import { test } from "node:test";
import { strict as assert } from "node:assert";

import { shouldPrune } from "../../src/lib/state/scratch-prune-pure.ts";

function makeNote(opts) {
  return {
    id: opts.id ?? "scratch/x",
    title: opts.title ?? "x",
    content: "",
    body: "",
    metadata: {
      title: opts.title ?? "x",
      tags: [],
      aliases: [],
      note_type: opts.note_type ?? null,
      custom: {},
      created: null,
      modified: null,
    },
    path: "",
    checksum: "",
    created_at: opts.created_at ?? null,
    modified_at: opts.modified_at ?? null,
    attachments: [],
  };
}

test("shouldPrune: skips non-scratch notes", () => {
  const n = makeNote({ note_type: "note", modified_at: "2020-01-01T00:00:00Z" });
  assert.equal(shouldPrune(n, new Date()), false);
});

test("shouldPrune: skips when no timestamps", () => {
  const n = makeNote({ note_type: "scratch" });
  assert.equal(shouldPrune(n, new Date()), false);
});

test("shouldPrune: deletes scratch older than cutoff (via modified_at)", () => {
  const n = makeNote({
    note_type: "scratch",
    modified_at: "2020-01-01T00:00:00Z",
  });
  const cutoff = new Date("2024-01-01T00:00:00Z");
  assert.equal(shouldPrune(n, cutoff), true);
});

test("shouldPrune: keeps fresh scratch", () => {
  const n = makeNote({
    note_type: "scratch",
    modified_at: "2026-05-16T00:00:00Z",
  });
  const cutoff = new Date("2026-05-10T00:00:00Z");
  assert.equal(shouldPrune(n, cutoff), false);
});

test("shouldPrune: falls back to created_at when modified_at missing", () => {
  const n = makeNote({
    note_type: "scratch",
    created_at: "2020-01-01T00:00:00Z",
  });
  const cutoff = new Date("2024-01-01T00:00:00Z");
  assert.equal(shouldPrune(n, cutoff), true);
});

test("shouldPrune: garbage timestamp is not pruned", () => {
  const n = makeNote({ note_type: "scratch", modified_at: "not-a-date" });
  assert.equal(shouldPrune(n, new Date()), false);
});
