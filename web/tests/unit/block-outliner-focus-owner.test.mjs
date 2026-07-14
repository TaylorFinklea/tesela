import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

async function loadFocusOwnerCheck() {
  const mod = await import("../../src/lib/editor/outliner-focus-owner.ts").catch(() => ({}));
  assert.equal(
    typeof mod.outlinerOwnsDocumentFocus,
    "function",
    "expected outlinerOwnsDocumentFocus to exist",
  );
  return mod.outlinerOwnsDocumentFocus;
}

const source = readFileSync(
  new URL("../../src/lib/components/BlockOutliner.svelte", import.meta.url),
  "utf8",
);

function sourceBetween(startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  const end = source.indexOf(endMarker, start);
  assert.notEqual(start, -1, `expected ${startMarker}`);
  assert.notEqual(end, -1, `expected ${endMarker}`);
  return source.slice(start, end);
}

test("an outliner owns focus only when its root contains the active element", async () => {
  const outlinerOwnsDocumentFocus = await loadFocusOwnerCheck();
  const inside = {};
  const outside = {};
  const root = { contains: (node) => node === inside };

  assert.equal(outlinerOwnsDocumentFocus(root, { activeElement: inside }), true);
  assert.equal(outlinerOwnsDocumentFocus(root, { activeElement: outside }), false);
  assert.equal(outlinerOwnsDocumentFocus(root, { activeElement: null }), false);
  assert.equal(outlinerOwnsDocumentFocus(null, { activeElement: inside }), false);
  assert.equal(outlinerOwnsDocumentFocus(root), false, "SSR has no document owner");
});

test("reactive focused-block publication is owner-gated but direct focus still publishes", () => {
  assert.match(
    source,
    /import \{ outlinerOwnsDocumentFocus \} from "\$lib\/editor\/outliner-focus-owner";/,
  );

  const reactivePublication = sourceBetween(
    "// Notify parent when a block GAINS focus",
    "function buildFullContent",
  );
  assert.match(
    reactivePublication,
    /if \(!outlinerOwnsDocumentFocus\(rootEl\)\) return;[\s\S]*onfocusedblockchange\?\.\(visibleBlocks\[focusedIndex\] \?\? null\)/,
  );

  const directFocus = sourceBetween("onfocus={() => {", "onchange={(text)");
  assert.match(directFocus, /focusedIndex = vi;[\s\S]*onfocusedblockchange\?\.\(block\)/);
  assert.doesNotMatch(directFocus, /outlinerOwnsDocumentFocus/);
});
