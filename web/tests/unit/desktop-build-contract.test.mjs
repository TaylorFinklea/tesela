import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const buildScript = readFileSync(new URL("../../../scripts/build-desktop.sh", import.meta.url), "utf8");
const releaseScript = readFileSync(new URL("../../../scripts/desktop-release.sh", import.meta.url), "utf8");
const installScript = readFileSync(new URL("../../../scripts/install-desktop.sh", import.meta.url), "utf8");
const tauriConfig = JSON.parse(
  readFileSync(new URL("../../../src-tauri/tauri.conf.json", import.meta.url), "utf8"),
);

test("desktop bundles the web UI as a runtime resource", () => {
  assert.equal(tauriConfig.bundle.resources["../web/build"], "web");
});

test("desktop artifact guard accepts only bundles with a web index", (t) => {
  const root = mkdtempSync(path.join(tmpdir(), "tesela-desktop-bundle-"));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const validApp = path.join(root, "Valid.app");
  const missingApp = path.join(root, "Missing.app");
  mkdirSync(path.join(validApp, "Contents", "Resources", "web"), { recursive: true });
  mkdirSync(missingApp);
  writeFileSync(
    path.join(validApp, "Contents", "Resources", "web", "index.html"),
    "<!doctype html>",
  );

  const command = 'source scripts/lib/desktop-bundle.sh; assert_desktop_web_bundle "$1"';
  const valid = spawnSync("bash", ["-c", command, "desktop-bundle-test", validApp], {
    cwd: new URL("../../..", import.meta.url),
    encoding: "utf8",
  });
  const missing = spawnSync("bash", ["-c", command, "desktop-bundle-test", missingApp], {
    cwd: new URL("../../..", import.meta.url),
    encoding: "utf8",
  });

  assert.equal(valid.status, 0, valid.stderr);
  assert.notEqual(missing.status, 0);
  assert.match(missing.stderr, /Contents\/Resources\/web\/index\.html/);
});

test("every desktop artifact flow checks the bundled web UI", () => {
  assert.match(buildScript, /assert_desktop_web_bundle "\$APP_BUNDLE"/);
  assert.match(releaseScript, /assert_desktop_web_bundle "\$APP_BUNDLE"/);
  assert.match(installScript, /assert_desktop_web_bundle "\$SRC"/);
});

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
