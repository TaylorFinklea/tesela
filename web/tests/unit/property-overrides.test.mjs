// Unit tests for the per-type property override merge in the TS registry
// (`property-registry.ts`). This is the mirror of the Rust resolver
// (`get_resolved_tag_def` / `apply_override` in `sqlite.rs`); the merged
// result MUST equal what the Rust resolver returns for the same input, so the
// core assertions use the SAME shared vector the Rust test uses:
//   Task    Status == [todo, doing, done, blocked] + show on_new
//   Project Status == [planned, active, shipped]
//
// See `.docs/ai/phases/2026-06-22-per-type-property-config-spec.md` §3.2–3.5.

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  buildRegistry,
  buildInheritanceMap,
  getTagPropertyDefs,
} from "../../src/lib/property-registry.ts";

// Minimal Note builder matching the shape the registry reads
// (title, metadata.note_type, metadata.custom).
function note(title, note_type, custom) {
  return {
    id: title,
    title,
    content: "",
    body: "",
    metadata: { note_type, custom },
    path: `${title}.md`,
    checksum: "",
  };
}

// The global Status Property page — choices are the GLOBAL fallback that each
// type's `property_overrides.Status.choices` REPLACEs.
const statusPage = note("Status", "Property", {
  value_type: "select",
  choices: ["backlog", "todo", "doing", "done"],
});
const priorityPage = note("Priority", "Property", {
  value_type: "select",
  choices: ["p1", "p2", "p3"],
});

// Built-in tag pages, mirroring server/lib.rs:217-218 (override = inline FLOW
// YAML; here pre-parsed as the registry receives it via NoteMetadata.custom).
const rootTag = note("Root Tag", "Tag", { tag_properties: [] });
const taskTag = note("Task", "Tag", {
  extends: "Root Tag",
  tag_properties: ["Status", "Priority"],
  property_overrides: {
    Status: { choices: ["todo", "doing", "done", "blocked"], show: "on_new", default: "todo" },
    Priority: { show: "on_set" },
  },
});
const projectTag = note("Project", "Tag", {
  extends: "Root Tag",
  tag_properties: ["Status"],
  property_overrides: {
    Status: { choices: ["planned", "active", "shipped"] },
  },
});

function resolve(tagName, notes) {
  const registry = buildRegistry(notes);
  const inheritance = buildInheritanceMap(notes);
  return getTagPropertyDefs(tagName, notes, registry, inheritance);
}

function byName(defs, name) {
  return defs.find((d) => d.name.toLowerCase() === name.toLowerCase());
}

test("Task Status == [todo, doing, done, blocked] + show on_new (REPLACE global)", () => {
  const defs = resolve("Task", [statusPage, priorityPage, rootTag, taskTag, projectTag]);
  const status = byName(defs, "Status");
  assert.ok(status, "Status resolved for Task");
  assert.deepEqual(status.choices, ["todo", "doing", "done", "blocked"]);
  assert.equal(status.show, "on_new");
  assert.equal(status.default, "todo");
});

test("Project Status == [planned, active, shipped] (REPLACE; show derived on_new)", () => {
  const defs = resolve("Project", [statusPage, priorityPage, rootTag, taskTag, projectTag]);
  const status = byName(defs, "Status");
  assert.ok(status, "Status resolved for Project");
  assert.deepEqual(status.choices, ["planned", "active", "shipped"]);
  // No `show` override + Status.hide_by_default=false → derived on_new.
  assert.equal(status.show, "on_new");
});

test("Task Priority show on_set (explicit override, no choices REPLACE keeps global)", () => {
  const defs = resolve("Task", [statusPage, priorityPage, rootTag, taskTag, projectTag]);
  const priority = byName(defs, "Priority");
  assert.ok(priority, "Priority resolved for Task");
  assert.equal(priority.show, "on_set");
  assert.deepEqual(priority.choices, ["p1", "p2", "p3"]);
});

test("no-override tag: choices identical to global, show derived on_new", () => {
  const personTag = note("Person", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Status"],
  });
  const defs = resolve("Person", [statusPage, rootTag, personTag]);
  const status = byName(defs, "Status");
  assert.deepEqual(status.choices, ["backlog", "todo", "doing", "done"]);
  assert.equal(status.show, "on_new");
  assert.equal(status.default, null);
});

test("hide_by_default=true derives show hidden (no override)", () => {
  const hiddenProp = note("Secret", "Property", {
    value_type: "text",
    hide_by_default: true,
  });
  const tag = note("Vault", "Tag", { extends: "Root Tag", tag_properties: ["Secret"] });
  const defs = resolve("Vault", [hiddenProp, rootTag, tag]);
  assert.equal(byName(defs, "Secret").show, "hidden");
});

test("REPLACE then SUBTRACT — choices override + hide_choices subtracts from replaced list", () => {
  const tag = note("Triage", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Status"],
    property_overrides: {
      Status: { choices: ["todo", "doing", "done", "blocked"], hide_choices: ["blocked"] },
    },
  });
  const defs = resolve("Triage", [statusPage, rootTag, tag]);
  // REPLACE [todo,doing,done,blocked] then SUBTRACT [blocked].
  assert.deepEqual(byName(defs, "Status").choices, ["todo", "doing", "done"]);
});

test("legacy hidden_{Prop} subtracts (TS-only fold, after REPLACE)", () => {
  const tag = note("Triage2", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Status"],
    property_overrides: {
      Status: { choices: ["todo", "doing", "done", "blocked"] },
    },
    hidden_Status: ["done"],
  });
  const defs = resolve("Triage2", [statusPage, rootTag, tag]);
  assert.deepEqual(byName(defs, "Status").choices, ["todo", "doing", "blocked"]);
});

test("child-wins first-insert: child override beats parent override", () => {
  const parent = note("Base", "Tag", {
    tag_properties: ["Status"],
    property_overrides: { Status: { choices: ["a", "b"], default: "a", show: "hidden" } },
  });
  const child = note("Derived", "Tag", {
    extends: "Base",
    tag_properties: [],
    property_overrides: { Status: { choices: ["x", "y"], default: "x", show: "on_set" } },
  });
  const defs = resolve("Derived", [statusPage, parent, child]);
  const status = byName(defs, "Status");
  // Child wins choices/default/show; Status is in membership via the parent.
  assert.deepEqual(status.choices, ["x", "y"]);
  assert.equal(status.default, "x");
  assert.equal(status.show, "on_set");
});

test("legacy hidden_{Prop} subtract is additive across the chain (child + parent accumulate)", () => {
  // The ADDITIVE subtract path is the legacy `hidden_{Prop}` fold (the TS-only
  // mirror of Rust's hidden_pairs loop): both child and parent `hidden_Status`
  // keys accumulate into the override's hide_choices. Here the CHILD owns the
  // choices REPLACE (so first-insert-wins keeps it) and both rows' hidden_
  // keys subtract from that replaced list — exactly Rust's additive subtract.
  const parent = note("PBase", "Tag", {
    tag_properties: ["Status"],
    hidden_Status: ["d"],
  });
  const child = note("PDerived", "Tag", {
    extends: "PBase",
    tag_properties: ["Status"],
    property_overrides: { Status: { choices: ["a", "b", "c", "d"] } },
    hidden_Status: ["a"],
  });
  const defs = resolve("PDerived", [statusPage, parent, child]);
  // REPLACE [a,b,c,d] (child override) then SUBTRACT {a (child hidden_),
  // d (parent hidden_)} → [b, c].
  assert.deepEqual(byName(defs, "Status").choices, ["b", "c"]);
});

test("child hidden_{Prop} that creates the entry first BLOCKS a parent's choices REPLACE (first-insert-wins)", () => {
  // Faithful mirror of Rust `build_overrides`: within each row the
  // property_overrides loop runs, then the hidden fold `or_default`s. A child
  // row whose ONLY contribution is hidden_Status creates the entry (choices
  // null) BEFORE the parent's property_overrides.Status is seen, so
  // first-insert-wins discards the parent's choices REPLACE — only the global
  // list (minus subtracted) survives. This is the intentional TS/Rust
  // asymmetry's edge (Rust's hidden_pairs is a no-op shim so it can't arise
  // there), and it follows directly from mirroring the Rust structure.
  const parent = note("WBase", "Tag", {
    tag_properties: ["Status"],
    property_overrides: { Status: { choices: ["a", "b", "c", "d"] } },
    hidden_Status: ["d"],
  });
  const child = note("WDerived", "Tag", {
    extends: "WBase",
    tag_properties: [],
    hidden_Status: ["a"],
  });
  const defs = resolve("WDerived", [statusPage, parent, child]);
  // Entry created by child hidden_ (choices null) → parent choices NOT merged.
  // Global [backlog,todo,doing,done] minus {a (none present), d (none present)}.
  assert.deepEqual(byName(defs, "Status").choices, ["backlog", "todo", "doing", "done"]);
});

test("whole-override first-insert-wins: child's choices:null discards parent's choices override", () => {
  // Mirror of Rust `or_insert_with`: the child inserts its WHOLE override
  // (choices absent → null) first, so the parent's `{choices:[a,b,c,d]}` is
  // never merged. Only the legacy hidden_ fold is additive.
  const parent = note("WBase", "Tag", {
    tag_properties: ["Status"],
    property_overrides: { Status: { choices: ["a", "b", "c", "d"] } },
  });
  const child = note("WDerived", "Tag", {
    extends: "WBase",
    tag_properties: [],
    property_overrides: { Status: { hide_choices: ["backlog"] } },
  });
  const defs = resolve("WDerived", [statusPage, parent, child]);
  // child override wins entirely (choices:null → keep GLOBAL list), then
  // subtract its own hide_choices ["backlog"].
  assert.deepEqual(byName(defs, "Status").choices, ["todo", "doing", "done"]);
});

test("§3.5b — override for a property NOT in membership is ignored", () => {
  const tag = note("Lonely", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Status"],
    // Priority override exists but Priority is not in tag_properties → ignored.
    property_overrides: {
      Status: { choices: ["todo"] },
      Priority: { choices: ["zzz"] },
    },
  });
  const defs = resolve("Lonely", [statusPage, priorityPage, rootTag, tag]);
  assert.equal(byName(defs, "Priority"), undefined);
  assert.deepEqual(byName(defs, "Status").choices, ["todo"]);
});

test("§3.5c — override for a property with NO global Property page applies to text stub", () => {
  const tag = note("Stubby", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Phase"], // no Phase Property page exists
    property_overrides: {
      Phase: { choices: ["alpha", "beta"], default: "alpha", show: "on_set" },
    },
  });
  const defs = resolve("Stubby", [rootTag, tag]);
  const phase = byName(defs, "Phase");
  assert.ok(phase, "stub Phase def created");
  assert.equal(phase.value_type, "text");
  assert.deepEqual(phase.choices, ["alpha", "beta"]);
  assert.equal(phase.default, "alpha");
  assert.equal(phase.show, "on_set");
});

test("case-insensitive override keys (property_overrides.STATUS matches Status)", () => {
  const tag = note("CaseTag", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Status"],
    property_overrides: { STATUS: { choices: ["x"] } },
  });
  const defs = resolve("CaseTag", [statusPage, rootTag, tag]);
  assert.deepEqual(byName(defs, "Status").choices, ["x"]);
});

test("malformed (non-object) override is ignored, not an error", () => {
  const tag = note("Bad", "Tag", {
    extends: "Root Tag",
    tag_properties: ["Status"],
    property_overrides: { Status: "garbage" },
  });
  const defs = resolve("Bad", [statusPage, rootTag, tag]);
  // Empty override → global choices preserved, show derived on_new.
  assert.deepEqual(byName(defs, "Status").choices, ["backlog", "todo", "doing", "done"]);
  assert.equal(byName(defs, "Status").show, "on_new");
});
