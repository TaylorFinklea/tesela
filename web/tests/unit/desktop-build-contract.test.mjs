import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const buildScript = readFileSync(new URL("../../../scripts/build-desktop.sh", import.meta.url), "utf8");

test("local builds remove stale signed updater artifacts when signing is disabled", () => {
  const fallbackStart = buildScript.indexOf("updater signing key unavailable");
  const buildStart = buildScript.indexOf("cargo tauri build", fallbackStart);
  assert.notEqual(fallbackStart, -1);
  assert.notEqual(buildStart, -1);

  const fallback = buildScript.slice(fallbackStart, buildStart);
  assert.match(fallback, /Tesela\.app\.tar\.gz/);
  assert.match(fallback, /\.sig/);
  assert.match(fallback, /rm -f/);
});
