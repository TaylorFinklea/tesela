import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

const buildScript = readFileSync(new URL("../../../scripts/build-desktop.sh", import.meta.url), "utf8");
const releaseScript = readFileSync(new URL("../../../scripts/desktop-release.sh", import.meta.url), "utf8");
const tauriConfig = JSON.parse(
  readFileSync(new URL("../../../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);

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

test("desktop release metadata is catalog-backed and version-aligned", () => {
  assert.equal(tauriConfig.version, "0.1.2");
  assert.match(releaseScript, /changelog\.mjs validate --platform desktop --version/);
  assert.match(releaseScript, /changelog\.mjs render --release/);
  assert.match(releaseScript, /--format markdown/);
  assert.match(releaseScript, /--format plain/);
  assert.doesNotMatch(releaseScript, /DESKTOP_RELEASE_NOTES/);
});

test("desktop release serializes updater JSON and GitHub notes safely", () => {
  assert.match(releaseScript, /changelog\.mjs updater-manifest/);
  assert.match(releaseScript, /--signature-file/);
  assert.match(releaseScript, /--notes-file/);
  assert.match(releaseScript, /gh release create.*--notes-file/);
  assert.doesNotMatch(releaseScript, /cat >"\$MANIFEST_PATH" <<JSON/);
});
