import { test } from "node:test";
import { strict as assert } from "node:assert";

import { findStrongFuzzyMatch } from "../../src/lib/fuzzy.ts";

test("findStrongFuzzyMatch returns the strongest non-exact existing label", () => {
  const match = findStrongFuzzyMatch("Prject", ["Area", "Project", "Priority"]);

  assert.equal(match?.label, "Project");
  assert.ok((match?.score ?? 0) > 0);
});

test("findStrongFuzzyMatch does not prompt for case-insensitive exact matches", () => {
  const match = findStrongFuzzyMatch("project", ["Project", "Priority"]);

  assert.equal(match, null);
});

test("findStrongFuzzyMatch ignores weak short accidental matches", () => {
  const match = findStrongFuzzyMatch("pt", ["Project Task", "Priority"]);

  assert.equal(match, null);
});
