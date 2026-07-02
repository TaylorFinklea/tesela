/**
 * Structural checks on the checked-in `src/lib/command-manifest.json` —
 * the file `scripts/generate-command-manifest.mjs` produces from the REAL
 * command registry and the Rust `GET /commands` route
 * (`crates/tesela-server/src/routes/commands.rs`) embeds. Regenerate with
 * `npm run generate:commands` after adding/editing a command; this test
 * only validates shape (no closures, unique ids, required fields) — it
 * can't detect staleness against the live registry without paying Vite's
 * module-load cost (that's what the generator script itself is for).
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
    "surfaces", "keywords", "takes_arg", "arg_prompt",
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
