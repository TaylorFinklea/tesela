// Phase 4 — per-choice color parse + serialize.
//
// A select / multi-select Property page can carry a `choice_colors` map
// (back-compat sibling key, like `value_chord_keys`): `{ done: "#7CB342",
// blocked: "#E8697F" }`. `parsePropertyPage` reads it (keys lowercased, empty
// values dropped) so DisplayChip can tint the value chip; the config UI
// serializes it back via `serializeChoiceColors` + `updateFrontmatterKey`.
//
// Absent map == today's behavior (empty `choice_colors: {}`), so the parse
// must default to `{}` and the serialize must collapse an empty map to `null`
// (caller removes the key rather than writing `choice_colors: {}`).
//
// See `.docs/ai/phases/2026-06-22-per-type-property-config-spec.md` §4 Phase 4.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  parsePropertyPage,
  serializeChoiceColors,
  updateFrontmatterKey,
  removeFrontmatterKey,
} from "../../src/lib/property-registry.ts";

function propNote(title, custom) {
  return {
    id: title,
    title,
    content: "",
    body: "",
    metadata: { note_type: "Property", custom },
    path: `${title}.md`,
    checksum: "",
  };
}

test("parsePropertyPage reads choice_colors (lowercased keys, trimmed values)", () => {
  const def = parsePropertyPage(
    propNote("Status", {
      value_type: "select",
      choices: ["todo", "doing", "done", "blocked"],
      choice_colors: { Done: "#7CB342", blocked: " #E8697F ", todo: "" },
    }),
  );
  assert.ok(def);
  // Key lowercased so a value lookup matches regardless of stored case.
  assert.equal(def.choice_colors.done, "#7CB342");
  // Value trimmed.
  assert.equal(def.choice_colors.blocked, "#E8697F");
  // Empty value dropped entirely (not stored as "").
  assert.equal(def.choice_colors.todo, undefined);
});

test("parsePropertyPage defaults choice_colors to {} when absent or malformed", () => {
  const absent = parsePropertyPage(propNote("Status", { value_type: "select", choices: ["a"] }));
  assert.deepEqual(absent.choice_colors, {});
  // A malformed (array) value is ignored, not crashed.
  const bad = parsePropertyPage(
    propNote("Status", { value_type: "select", choices: ["a"], choice_colors: ["x"] }),
  );
  assert.deepEqual(bad.choice_colors, {});
});

test("serializeChoiceColors → single-line FLOW YAML / JSON; empty → null", () => {
  const s = serializeChoiceColors({ done: "#7CB342", blocked: "#E8697F" });
  assert.ok(s);
  assert.ok(!s.includes("\n"), "single line");
  // Compact JSON is valid flow YAML; mirror the server's parse with JSON.parse.
  assert.deepEqual(JSON.parse(s), { done: "#7CB342", blocked: "#E8697F" });

  // Empty / whitespace-only values are dropped.
  assert.deepEqual(JSON.parse(serializeChoiceColors({ done: "#7CB342", x: "  " })), {
    done: "#7CB342",
  });

  // Wholly empty map → null (caller removes the key).
  assert.equal(serializeChoiceColors({}), null);
  assert.equal(serializeChoiceColors({ x: "", y: "   " }), null);
});

test("round-trip: serialize → write to Property page frontmatter → re-parse", () => {
  const map = { done: "#7CB342", blocked: "#E8697F" };
  const serialized = serializeChoiceColors(map);
  const base =
    '---\ntitle: "Status"\ntype: "Property"\nvalue_type: "select"\nchoices: ["todo", "done", "blocked"]\ntags: []\n---\n- Status property.\n';
  const written = updateFrontmatterKey(base, "choice_colors", serialized);

  // Other keys untouched.
  assert.ok(written.includes('value_type: "select"'));
  assert.ok(written.includes("- Status property."));

  // Mirror the server parse (inline JSON map → nested object) and re-parse via
  // parsePropertyPage.
  const m = written.match(/^choice_colors:\s*(.*)$/m);
  assert.ok(m);
  const def = parsePropertyPage(
    propNote("Status", { value_type: "select", choices: ["todo", "done", "blocked"], choice_colors: JSON.parse(m[1]) }),
  );
  assert.equal(def.choice_colors.done, "#7CB342");
  assert.equal(def.choice_colors.blocked, "#E8697F");

  // Clearing the last color removes the key entirely (no `choice_colors: {}`).
  assert.equal(serializeChoiceColors({}), null);
  const removed = removeFrontmatterKey(written, "choice_colors");
  assert.ok(!removed.includes("choice_colors"));
  assert.ok(removed.includes('value_type: "select"'));
});
