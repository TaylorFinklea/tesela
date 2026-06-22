// Phase 3 — config-UI write-back round-trip.
//
// The config editor (`TagPropertyConfig.svelte`) edits the RAW
// `property_overrides` map, serializes it to single-line FLOW YAML / compact
// JSON via `serializePropertyOverrides` + `updateFrontmatterKey`, and writes it
// to the Tag page markdown. On the server, gray_matter parses that inline
// JSON-as-flow-YAML map back into `metadata.custom.property_overrides` (a nested
// object) via `pod_to_json`. Compact JSON is valid flow YAML, so a faithful
// client-side mirror of that parse is `JSON.parse` of the serialized value.
//
// This test exercises the full loop: build overrides → serialize via the write
// path → parse back the way frontmatter parsing does → resolve via
// `getTagPropertyDefs` → assert the edited Status choices/show/default survived.
//
// See `.docs/ai/phases/2026-06-22-per-type-property-config-spec.md` §3.5, §4 P3.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  buildRegistry,
  buildInheritanceMap,
  getTagPropertyDefs,
  serializePropertyOverrides,
  parsePropertyOverridesRaw,
  normalizeRawOverride,
  updateFrontmatterKey,
  removeFrontmatterKey,
} from "../../src/lib/property-registry.ts";

function note(title, note_type, custom, content = "") {
  return {
    id: title,
    title,
    content,
    body: "",
    metadata: { note_type, custom },
    path: `${title}.md`,
    checksum: "",
  };
}

const statusPage = note("Status", "Property", {
  value_type: "select",
  choices: ["backlog", "todo", "doing", "done"],
});
const rootTag = note("Root Tag", "Tag", { tag_properties: [] });

// Extract the value gray_matter would store in metadata.custom for the
// `property_overrides` frontmatter key, by mirroring its parse of an inline
// JSON-as-flow-YAML map: JSON.parse of the serialized single-line value.
function customFromFrontmatter(content) {
  const m = content.match(/^property_overrides:\s*(.*)$/m);
  if (!m) return undefined;
  return JSON.parse(m[1]);
}

test("round-trip: edited Task Status choices survive serialize → frontmatter → parse → resolve", () => {
  // 1. Build the overrides object the editor would hold after the user replaces
  //    Task's Status choices and sets show/default.
  const map = {
    Status: { choices: ["todo", "doing", "done", "blocked"], show: "on_new", default: "todo" },
  };

  // 2. Serialize via the write path and write into a Tag page's frontmatter.
  const serialized = serializePropertyOverrides(map);
  assert.ok(serialized, "serialized to a non-empty FLOW YAML / JSON value");
  // Must be a single line (one frontmatter line) — no raw newlines.
  assert.ok(!serialized.includes("\n"), "serialized override is single-line");

  const baseContent =
    '---\ntitle: "Task"\ntype: "Tag"\nextends: "Root Tag"\ntag_properties: ["Status"]\ntags: []\n---\n- Task tag page.\n';
  const written = updateFrontmatterKey(baseContent, "property_overrides", serialized);

  // Other frontmatter keys must be untouched.
  assert.ok(written.includes('title: "Task"'));
  assert.ok(written.includes('tag_properties: ["Status"]'));
  assert.ok(written.includes("- Task tag page."));

  // 3. Parse the frontmatter back the way the server does (inline JSON map →
  //    nested object in metadata.custom).
  const parsedCustom = customFromFrontmatter(written);
  const taskTag = note("Task", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Status"],
    property_overrides: parsedCustom,
  });

  // 4. Resolve and assert the edited Status config survived the round-trip.
  const notes = [statusPage, rootTag, taskTag];
  const registry = buildRegistry(notes);
  const inheritance = buildInheritanceMap(notes);
  const defs = getTagPropertyDefs("Task", notes, registry, inheritance);
  const status = defs.find((d) => d.name.toLowerCase() === "status");
  assert.ok(status, "Status resolved for Task");
  assert.deepEqual(status.choices, ["todo", "doing", "done", "blocked"]);
  assert.equal(status.show, "on_new");
  assert.equal(status.default, "todo");
});

test("empty / inherited override map serializes to null → key removed (never bakes inherit)", () => {
  // An all-empty entry must NOT be persisted as an override (spec §3.5 tail).
  assert.equal(serializePropertyOverrides({ Status: {} }), null);
  assert.equal(serializePropertyOverrides({ Status: { choices: [], hide_choices: [] } }), null);
  assert.equal(serializePropertyOverrides({ Status: { default: "  " } }), null);

  // normalizeRawOverride drops inherit-only fields.
  assert.equal(normalizeRawOverride({ choices: [], default: "" }), null);
  assert.deepEqual(normalizeRawOverride({ choices: ["a"], default: "" }), { choices: ["a"] });

  // A removed key leaves the rest of the frontmatter intact.
  const content =
    '---\ntitle: "Task"\ntag_properties: ["Status"]\nproperty_overrides: {"Status":{"choices":["x"]}}\n---\nbody\n';
  const removed = removeFrontmatterKey(content, "property_overrides");
  assert.ok(!removed.includes("property_overrides"));
  assert.ok(removed.includes('title: "Task"'));
  assert.ok(removed.includes('tag_properties: ["Status"]'));
});

test("parsePropertyOverridesRaw distinguishes overridden vs inherited and preserves case", () => {
  const custom = {
    property_overrides: {
      Status: { choices: ["todo", "done"], show: "on_set" },
      Priority: {}, // present but empty → inherited
    },
  };
  const raw = parsePropertyOverridesRaw(custom);
  assert.deepEqual(raw.Status.choices, ["todo", "done"]);
  assert.equal(raw.Status.show, "on_set");
  assert.deepEqual(raw.Priority, {});
  // A property with NO entry is absent (inherited) — distinct from {} (also
  // inherited but explicitly present); both serialize away.
  assert.equal(raw.Deadline, undefined);
});
