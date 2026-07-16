/**
 * Structural checks on the checked-in `src/lib/command-manifest.json` —
 * the file `scripts/generate-command-manifest.mjs` produces from the REAL
 * command registry and the Rust `GET /commands` route
 * (`crates/tesela-server/src/routes/commands.rs`) embeds. Regenerate with
 * `npm run generate:commands` after adding/editing a command; this test
 * only validates shape (no closures, unique ids, required fields).
 *
 * For freshness checking (staleness against the live registry), see
 * `scripts/check-manifest-fresh.mjs` which runs in CI and ensures edits to
 * commands must regenerate the JSON.
 */
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";

const manifest = JSON.parse(
  readFileSync(new URL("../../src/lib/command-manifest.json", import.meta.url), "utf8"),
);

test("command-manifest.json is a non-empty array", () => {
  assert.ok(Array.isArray(manifest));
  assert.ok(manifest.length > 0);
});

test("command-manifest.json entries have unique ids", () => {
  const ids = manifest.map((e) => e.id);
  assert.deepEqual(ids, [...new Set(ids)]);
});

test("command-manifest.json entries carry only manifest fields — no run/when closures", () => {
  const allowed = new Set([
    "id", "verb", "label", "glyph", "category", "shortcut", "chord",
    "surfaces", "keywords", "takes_arg", "arg_prompt", "description",
  ]);
  for (const entry of manifest) {
    assert.equal("run" in entry, false, `${entry.id}: manifest entry must not carry run`);
    assert.equal("when" in entry, false, `${entry.id}: manifest entry must not carry when`);
    for (const key of Object.keys(entry)) {
      assert.ok(allowed.has(key), `${entry.id}: unexpected field "${key}"`);
    }
    assert.equal(typeof entry.id, "string");
    assert.equal(typeof entry.label, "string");
    assert.equal(typeof entry.glyph, "string");
    assert.equal(typeof entry.category, "string");
    assert.ok(Array.isArray(entry.surfaces));
    assert.ok(Array.isArray(entry.keywords));
    assert.equal(typeof entry.takes_arg, "boolean");
  }
});

test("ambient views command is labeled Views, not Inbox", () => {
  const entry = manifest.find((e) => e.id === "views");
  assert.ok(entry, "expected a views ambient command entry");
  assert.equal(manifest.some((e) => e.id === "inbox"), false, "inbox command id should not remain primary");
  assert.equal(entry.label, "Open Views");
  assert.ok(entry.keywords.includes("views"), "Views command should be searchable as views");
  assert.equal(entry.keywords.includes("inbox"), false, "Inbox should not be visible command search copy");
});

test("manifest exposes Move block subtree on the free a m chord", () => {
  const entry = manifest.find((item) => item.id === "move-block-subtree");
  assert.ok(entry);
  assert.equal(entry.label, "Move block subtree");
  assert.deepEqual(entry.chord, ["a", "m"]);
  assert.deepEqual(entry.surfaces, ["leader", "palette"]);
  assert.equal(manifest.filter((item) => item.chord?.join(" ") === "a m").length, 1);
});

test("manifest exposes every rail action through named commands", () => {
  const railCommands = [
    ["rail-focus", ["r", "f"]],
    ["rail-quick-capture", ["r", "c"]],
    ["rail-add-widget", ["r", "a"]],
  ];

  for (const [id, chord] of railCommands) {
    const entry = manifest.find((item) => item.id === id);
    assert.ok(entry, `expected ${id} in the command manifest`);
    assert.deepEqual(entry.chord, chord);
    assert.ok(entry.surfaces.includes("leader"));
  }

  const jump = manifest.find((item) => item.id === "jump");
  assert.ok(jump, "rail page rows reuse the named jump command");
  assert.equal(jump.takes_arg, true);

  const railFavorite = manifest.find((item) => item.id === "rail-toggle-favorite");
  assert.ok(railFavorite, "rail favorite buttons have a named argument-taking command");
  assert.equal(railFavorite.takes_arg, true);
  assert.match(railFavorite.arg_prompt, /page/i);

  const focusedFavorite = manifest.find((item) => item.id === "toggle-favorite");
  assert.ok(focusedFavorite, "the existing focused-page favorite command remains available");
  assert.equal(focusedFavorite.takes_arg, false);
});
