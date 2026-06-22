// Phase 2 — unit tests for the per-type SEED selection + VISIBILITY predicate
// derived from the resolved `show` (on_new / on_set / hidden) on each
// PropertyDefinition. These mirror the logic wired into the editor:
//   - BlockOutliner.autoFillNamesForTag / autoFillTagDefaults — seed ONLY
//     `show === "on_new"` props (with their per-type default).
//   - BlockOutliner.hiddenKeysFor + cm-decorations — hide a key when
//     `show === "hidden"`, OR `show === "on_set"` AND the value is empty.
//     `on_new` and valued `on_set` stay visible.
//
// The `show` values come straight from `getTagPropertyDefs` (the same per-type
// resolver Phase 1 shipped), so this test certifies the Phase 2 routing reads
// the resolver's contract correctly.
//
// See `.docs/ai/phases/2026-06-22-per-type-property-config-spec.md` §3.4,
// decision 2 (LOCKED visibility semantics).

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  buildRegistry,
  buildInheritanceMap,
  getTagPropertyDefs,
} from "../../src/lib/property-registry.ts";

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

// --- Pure predicates mirroring the BlockOutliner wiring -------------------

/** Names to seed on tag-add: ONLY `show === "on_new"`
 *  (mirror of BlockOutliner.autoFillNamesForTag). */
function seedNames(defs) {
  return defs.filter((d) => d.show === "on_new").map((d) => d.name);
}

/** Default-seed entries: `on_new` props with a non-null default
 *  (mirror of BlockOutliner.autoFillTagDefaults's `show`/default gate). */
function defaultSeeds(defs) {
  return defs
    .filter((d) => d.show === "on_new" && d.default != null)
    .map((d) => [d.name, d.default]);
}

/** Visibility predicate: is this property hidden given its (possibly empty)
 *  value? Mirror of `hiddenKeysFor` + cm-decorations `shouldHide` — `hidden`
 *  is always suppressed; `on_set` is suppressed when its value is empty. */
function isHidden(def, value) {
  const isEmpty = !value || value.trim() === "";
  if (def.show === "hidden") return true;
  if (def.show === "on_set" && isEmpty) return true;
  return false;
}

// --- Fixtures ------------------------------------------------------------

const statusPage = note("Status", "Property", {
  value_type: "select",
  choices: ["todo", "doing", "done"],
  default: "todo",
});
const priorityPage = note("Priority", "Property", {
  value_type: "select",
  choices: ["p1", "p2", "p3"],
  // Priority is on_set in the Mixed tag — it carries a default precisely so
  // the default-seed test fails if the show-gate regresses to a default-only
  // gate (an on_set/hidden prop with a default must NOT seed its default).
  default: "p2",
});
const notesPage = note("Notes", "Property", { value_type: "text" });
const secretPage = note("Secret", "Property", { value_type: "text" });

// Mixed-visibility tag: Status on_new (+default), Priority on_set, Secret hidden.
// Notes has no override → derived on_new (Notes.hide_by_default=false).
const mixedTag = note("Mixed", "Tag", {
  extends: "Root Tag",
  tag_properties: ["Status", "Priority", "Secret", "Notes"],
  property_overrides: {
    Status: { show: "on_new", default: "todo" },
    Priority: { show: "on_set" },
    Secret: { show: "hidden" },
  },
});
const rootTag = note("Root Tag", "Tag", { tag_properties: [] });

function resolve(tagName, notes) {
  const registry = buildRegistry(notes);
  const inheritance = buildInheritanceMap(notes);
  return getTagPropertyDefs(tagName, notes, registry, inheritance);
}

const fixtures = [statusPage, priorityPage, notesPage, secretPage, rootTag, mixedTag];

// --- Tests ---------------------------------------------------------------

test("seed selection returns ONLY on_new props", () => {
  const defs = resolve("Mixed", fixtures);
  const seeded = seedNames(defs).map((n) => n.toLowerCase()).sort();
  // Status (explicit on_new) + Notes (derived on_new). NOT Priority (on_set)
  // or Secret (hidden).
  assert.deepEqual(seeded, ["notes", "status"]);
});

test("default-seed applies per-type default only to on_new props", () => {
  const defs = resolve("Mixed", fixtures);
  const seeds = defaultSeeds(defs);
  // Only Status carries a default AND is on_new → exactly one seed.
  assert.deepEqual(seeds, [["Status", "todo"]]);
});

test("visibility predicate hides `hidden` props regardless of value", () => {
  const defs = resolve("Mixed", fixtures);
  const secret = defs.find((d) => d.name.toLowerCase() === "secret");
  assert.ok(secret);
  assert.equal(isHidden(secret, ""), true);
  assert.equal(isHidden(secret, "classified"), true);
});

test("visibility predicate hides empty `on_set` but shows valued `on_set`", () => {
  const defs = resolve("Mixed", fixtures);
  const priority = defs.find((d) => d.name.toLowerCase() === "priority");
  assert.ok(priority);
  assert.equal(priority.show, "on_set");
  assert.equal(isHidden(priority, ""), true); // empty → hidden
  assert.equal(isHidden(priority, "p1"), false); // valued → visible
});

test("visibility predicate never hides `on_new` props (empty or valued)", () => {
  const defs = resolve("Mixed", fixtures);
  const status = defs.find((d) => d.name.toLowerCase() === "status");
  assert.ok(status);
  assert.equal(status.show, "on_new");
  assert.equal(isHidden(status, ""), false);
  assert.equal(isHidden(status, "doing"), false);
});
