import assert from "node:assert/strict";
import { readdirSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const webRoot = path.resolve(fileURLToPath(new URL("../..", import.meta.url)));
const allowedLegacyAlias = path.join("src", "lib", "legacy", "v4-token-aliases.css");
const thisFile = path.join("tests", "unit", "live-layer-contract.test.mjs");
const scanRoots = ["src", "scripts", path.join("tests", "unit")];
const sourceExtensions = new Set([".svelte", ".ts", ".js", ".mjs", ".css"]);

function* walk(dir) {
  for (const entry of readdirSync(dir)) {
    const full = path.join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      yield* walk(full);
    } else if (st.isFile() && sourceExtensions.has(path.extname(full))) {
      yield full;
    }
  }
}

function scannedFiles() {
  return scanRoots.flatMap((root) => [...walk(path.join(webRoot, root))]);
}

function relative(file) {
  return path.relative(webRoot, file).split(path.sep).join("/");
}

const legacyImportPattern = /(?:\$lib\/v[45]|(?:src\/)?lib\/v[45])/;
const versionedVarPattern = /--v4-[A-Za-z0-9_-]+/;
const manualRootPattern = /\bv4-root\b/;

test("live behavior imports are no longer under lib/v4 or lib/v5", () => {
  const offenders = [];
  for (const file of scannedFiles()) {
    const rel = relative(file);
    if (rel === allowedLegacyAlias || rel === thisFile) continue;
    const content = readFileSync(file, "utf8");
    if (legacyImportPattern.test(content)) offenders.push(rel);
  }
  assert.deepEqual(offenders, []);
});

test("v4 design variables/root scope are quarantined to one legacy alias file", () => {
  const offenders = [];
  for (const file of scannedFiles()) {
    const rel = relative(file);
    if (rel === allowedLegacyAlias || rel === thisFile) continue;
    const content = readFileSync(file, "utf8");
    if (versionedVarPattern.test(content) || manualRootPattern.test(content)) {
      offenders.push(rel);
    }
  }
  assert.deepEqual(offenders, []);
});
