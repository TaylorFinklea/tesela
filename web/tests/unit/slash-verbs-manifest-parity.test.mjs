/**
 * tesela-cmdd.5 — the web slash menu's base-verb SET traces to the checked-in
 * command manifest (`src/lib/command-manifest.json`, tesela-cmdd.2's ONE
 * extraction point), the same way `BlockEditor.getSlashTree` builds its
 * `verbLeaves` from `commandRegistry.availableOn('slash', baseCtx)
 * .filter(cmd => cmd.slashKey)` — this file can't drive that Svelte-bound
 * live registry lookup directly (no Vite here, see
 * `scripts/generate-command-manifest.mjs`'s docstring), so it locks the
 * invariant against the manifest snapshot instead: any manifest command
 * added/removed from the `editor` category's `slash` surface shows up here,
 * so an iOS/web divergence can't land as a silent, forgotten edit (mirrors
 * `ManifestSlashVerbsTests.swift` on iOS).
 */
import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";

const manifest = JSON.parse(
  readFileSync(new URL("../../src/lib/command-manifest.json", import.meta.url), "utf8"),
);

// `editor.property` is invoked from the `/p` submenu leaf (no `slashKey` on
// web), and `editor.widget` is leader-only — both are `editor`+`slash` in
// the manifest's coarse surfaces list but are NOT top-level slash verbs.
const NOT_TOP_LEVEL_SLASH_VERBS = new Set(["editor.property", "editor.widget"]);

function canonicalBaseVerbIds() {
  return manifest
    .filter((e) => e.category === "editor" && e.surfaces.includes("slash"))
    .map((e) => e.id)
    .filter((id) => !NOT_TOP_LEVEL_SLASH_VERBS.has(id));
}

test("the manifest's canonical slash base-verb set is exactly the 8 insertion verbs", () => {
  // Mirrors slash-tree.test.mjs's "8 insertion verbs" fixture — this test
  // asserts the CHECKED-IN MANIFEST (not just the hand-authored test
  // fixture) agrees, so a manifest edit that adds/removes an editor/slash
  // command is caught here even if slash-tree.test.mjs's fixture is never
  // touched.
  assert.deepEqual(
    canonicalBaseVerbIds().sort(),
    [
      "editor.collection",
      "editor.date",
      "editor.heading",
      "editor.link",
      "editor.query",
      "editor.tag",
      "editor.task",
      "editor.template",
    ].sort(),
  );
});

test("editor.property and editor.widget are structurally excluded, not silently missing", () => {
  const ids = canonicalBaseVerbIds();
  assert.ok(!ids.includes("editor.property"));
  assert.ok(!ids.includes("editor.widget"));
  // Confirm they DO exist in the manifest under 'editor' — this is an
  // intentional carve-out, not evidence the commands were removed.
  const editorIds = manifest.filter((e) => e.category === "editor").map((e) => e.id);
  assert.ok(editorIds.includes("editor.property"));
  assert.ok(editorIds.includes("editor.widget"));
});
