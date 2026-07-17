import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import test from "node:test";
import { fileURLToPath } from "node:url";
import path from "node:path";

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../../..");

function help(script) {
  return spawnSync("bash", [script, "--help"], {
    cwd: repo,
    encoding: "utf8",
  });
}

test("desktop release help points operators through Bitwarden", () => {
  const result = help("scripts/desktop-release.sh");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /bws-project run -- scripts\/desktop-release\.sh/);
});

test("iOS release help points operators through Bitwarden", () => {
  const result = help("scripts/ios-testflight.sh");
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /bws-project run -- scripts\/ios-testflight\.sh/);
});

test("release scripts have no persistent Apple-key or updater-Keychain fallback", () => {
  const scripts = [
    "scripts/build-desktop.sh",
    "scripts/desktop-release.sh",
    "scripts/ios-testflight.sh",
  ].map((file) => readFileSync(path.join(repo, file), "utf8")).join("\n");

  assert.doesNotMatch(scripts, /\.appstoreconnect\/AuthKey_/);
  assert.doesNotMatch(scripts, /tesela-desktop-updater-key/);
  assert.doesNotMatch(scripts, /security find-generic-password/);
});

test("desktop release inputs cannot bypass the Bitwarden namespaced entries", () => {
  const desktopRelease = readFileSync(
    path.join(repo, "scripts/desktop-release.sh"),
    "utf8",
  );
  assert.match(
    desktopRelease,
    /require_bws_secret TESELA_TAURI_SIGNING_PRIVATE_KEY\n/,
  );
  assert.match(
    desktopRelease,
    /require_bws_secret TESELA_TAURI_SIGNING_PRIVATE_KEY_PASSWORD\n/,
  );

  const desktopBuild = readFileSync(
    path.join(repo, "scripts/build-desktop.sh"),
    "utf8",
  );
  const ambientUnset = desktopBuild.indexOf(
    "unset TAURI_SIGNING_PRIVATE_KEY TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
  );
  const bwsMapping = desktopBuild.indexOf(
    'if [[ -n "${TESELA_TAURI_SIGNING_PRIVATE_KEY:-}" ]]',
  );
  assert.ok(ambientUnset >= 0, "build script must discard ambient Tauri credentials");
  assert.ok(
    ambientUnset < bwsMapping,
    "ambient Tauri credentials must be discarded before the BWS mapping",
  );
});

test("desktop updater public key is a valid UTF-8 minisign public key box", () => {
  const config = JSON.parse(
    readFileSync(path.join(repo, "src-tauri/tauri.conf.json"), "utf8"),
  );
  const decoded = Buffer.from(config.plugins.updater.pubkey, "base64");
  assert.equal(decoded.toString("base64"), config.plugins.updater.pubkey);
  const publicKeyBox = new TextDecoder("utf-8", { fatal: true }).decode(decoded);
  const lines = publicKeyBox.trimEnd().split("\n");

  assert.equal(lines.length, 2);
  assert.match(lines[0], /^untrusted comment: minisign public key: [0-9A-F]{16}$/);
  assert.match(lines[1], /^RW[A-Za-z0-9+/]{54}$/);
});
