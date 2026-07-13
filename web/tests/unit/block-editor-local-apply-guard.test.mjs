import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const source = readFileSync(new URL("../../src/lib/components/BlockEditor.svelte", import.meta.url), "utf8");

function functionBody(name) {
  const start = source.indexOf(`function ${name}(`);
  assert.notEqual(start, -1, `expected ${name} to exist`);
  const brace = source.indexOf("{", start);
  let depth = 0;
  for (let i = brace; i < source.length; i++) {
    if (source[i] === "{") depth += 1;
    if (source[i] === "}") depth -= 1;
    if (depth === 0) return source.slice(brace + 1, i);
  }
  throw new Error(`could not parse body for ${name}`);
}

test("slash/property programmatic dispatches use the local-apply guard", () => {
  for (const name of ["openDatePickerForProperty", "writePropertyContinuation"]) {
    const body = functionBody(name);
    assert.match(body, /onChange\(/, `${name} should still perform its explicit onChange save`);
    assert.doesNotMatch(body, /\bview\.dispatch\s*\(/, `${name} should not dispatch directly before explicit onChange`);
    assert.match(body, /dispatchWithLocalApplyGuard\s*\(/, `${name} should guard programmatic dispatches`);
  }
});

test("applySlash is deleted", () => {
  assert.equal(source.indexOf("function applySlash("), -1, "applySlash must not exist");
});

test("remote text projection is anchored to the exact subscribed LoroText", () => {
  const reconcile = functionBody("reconcileLoroText");
  const lifecycle = source.slice(
    source.indexOf("// C2.3 reactive subscription lifecycle"),
    source.indexOf("onMount(() => {"),
  );

  assert.match(reconcile, /const canonicalText = container\.toString\(\);/);
  assert.match(reconcile, /planTextReconciliation\(/);
  assert.match(reconcile, /v\.state\.doc\.toString\(\) !== canonicalText/);
  assert.match(reconcile, /onLoroText\?\.\(canonicalText\)/);
  assert.doesNotMatch(reconcile, /Math\.min\(c\.(?:from|to), docLen\)/);
  assert.match(
    lifecycle,
    /container\.subscribe\(\(batch\) => applyRemoteTextEvent\(container, batch\)\)/,
  );
  assert.match(lifecycle, /reconcileLoroText\(container, \[\]\)/);
});
