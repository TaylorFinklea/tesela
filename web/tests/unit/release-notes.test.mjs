import assert from "node:assert/strict";
import test from "node:test";

import catalogJson from "../../../release-notes/releases.json" with { type: "json" };
import {
  ReleaseNotesSeenState,
  loadBundledReleaseNotes,
  parseReleaseCatalog,
  platformReleaseHistory,
  releaseVersionLabel,
  resolveReleasePlatform,
  safeReleaseCatalog,
  shouldPresentCurrent,
} from "../../src/lib/release-notes.ts";

function catalog() {
  return structuredClone(catalogJson);
}

class MemoryStorage {
  values = new Map();
  readsFail = false;
  writesFail = false;

  getItem(key) {
    if (this.readsFail) throw new Error("read denied");
    return this.values.get(key) ?? null;
  }

  setItem(key, value) {
    if (this.writesFail) throw new Error("write denied");
    this.values.set(key, value);
  }
}

test("parses the bundled schema-1 catalog", () => {
  const parsed = parseReleaseCatalog(catalog());
  assert.equal(parsed.current.desktop, "2026-07-17.desktop-0.1.3");
  assert.equal(loadBundledReleaseNotes()?.releases.length, 7);
});

test("strict parsing rejects unsupported schema and safe loading fails soft", () => {
  const invalid = catalog();
  invalid.schemaVersion = 2;
  assert.throws(() => parseReleaseCatalog(invalid), /schemaVersion/);
  const messages = [];
  assert.equal(safeReleaseCatalog(invalid, (message) => messages.push(message)), null);
  assert.equal(messages.length, 1);
  assert.match(messages[0], /release notes unavailable/i);
});

test("strict parsing rejects invalid current pointers and release shape", () => {
  const missing = catalog();
  missing.current.web = "missing";
  assert.throws(() => parseReleaseCatalog(missing), /current\.web/);

  const empty = catalog();
  empty.releases[0].new = [];
  empty.releases[0].fixed = [];
  empty.releases[0].important = [];
  assert.throws(() => parseReleaseCatalog(empty), /change item/);
});

test("hosted web is the default and only the explicit Tauri bridge selects desktop", () => {
  assert.equal(resolveReleasePlatform(), "web");
  assert.equal(resolveReleasePlatform({ __TESELA_PLATFORM__: "desktop" }), "desktop");
  assert.equal(resolveReleasePlatform({ __TESELA_PLATFORM__: "ios" }), "web");
});

test("platform history starts at current and contains only applicable older releases", () => {
  const parsed = parseReleaseCatalog(catalog());
  assert.deepEqual(
    platformReleaseHistory(parsed, "desktop").map((release) => release.id),
    [
      "2026-07-17.desktop-0.1.3",
      "2026-07-15.desktop-0.1.2",
      "2026-07-02.desktop-0.1.1",
      "2026-06-04.desktop-0.1.0",
    ],
  );
  assert.deepEqual(
    platformReleaseHistory(parsed, "ios").map((release) => release.id),
    [
      "2026-07-15.ios-1.1-80",
      "2026-07-14.ios-1.1-79",
      "2026-07-08.ios-1.1-75",
    ],
  );
});

test("seen-state decisions cover missing, unknown, older, current, and downgrade", () => {
  const parsed = parseReleaseCatalog(catalog());
  assert.equal(shouldPresentCurrent(parsed, "desktop", null), true);
  assert.equal(shouldPresentCurrent(parsed, "desktop", "unknown"), true);
  assert.equal(shouldPresentCurrent(parsed, "desktop", "2026-07-02.desktop-0.1.1"), true);
  assert.equal(shouldPresentCurrent(parsed, "desktop", "2026-07-17.desktop-0.1.3"), false);

  const downgraded = catalog();
  downgraded.current.desktop = "2026-07-02.desktop-0.1.1";
  const parsedDowngrade = parseReleaseCatalog(downgraded);
  assert.equal(
    shouldPresentCurrent(parsedDowngrade, "desktop", "2026-07-17.desktop-0.1.3"),
    false,
  );
});

test("storage read failure treats the release as unknown", () => {
  const storage = new MemoryStorage();
  storage.readsFail = true;
  const state = new ReleaseNotesSeenState(
    parseReleaseCatalog(catalog()),
    "web",
    storage,
    new Set(),
  );
  assert.equal(state.shouldAutoPresent(), true);
});

test("rendering current persists the platform key", () => {
  const storage = new MemoryStorage();
  const state = new ReleaseNotesSeenState(
    parseReleaseCatalog(catalog()),
    "desktop",
    storage,
    new Set(),
  );
  state.markCurrentRendered();
  assert.equal(
    storage.getItem("tesela:releaseNotes:lastSeen:desktop"),
    "2026-07-17.desktop-0.1.3",
  );
  assert.equal(state.shouldAutoPresent(), false);
});

test("storage write failure still suppresses reopening during the session", () => {
  const storage = new MemoryStorage();
  storage.writesFail = true;
  const sessionSeen = new Set();
  const state = new ReleaseNotesSeenState(
    parseReleaseCatalog(catalog()),
    "web",
    storage,
    sessionSeen,
  );
  assert.equal(state.shouldAutoPresent(), true);
  state.markCurrentRendered();
  assert.equal(state.shouldAutoPresent(), false);

  const recreated = new ReleaseNotesSeenState(
    parseReleaseCatalog(catalog()),
    "web",
    storage,
    sessionSeen,
  );
  assert.equal(recreated.shouldAutoPresent(), false);
});

test("manual browsing does not alter seen state until current detail renders", () => {
  const storage = new MemoryStorage();
  const state = new ReleaseNotesSeenState(
    parseReleaseCatalog(catalog()),
    "web",
    storage,
    new Set(),
  );
  platformReleaseHistory(parseReleaseCatalog(catalog()), "web")[1];
  assert.equal(storage.values.size, 0);
  assert.equal(state.shouldAutoPresent(), true);
});

test("version labels are platform-aware", () => {
  const parsed = parseReleaseCatalog(catalog());
  assert.equal(releaseVersionLabel(platformReleaseHistory(parsed, "desktop")[0], "desktop"), "Tesela 0.1.3");
  assert.equal(releaseVersionLabel(platformReleaseHistory(parsed, "ios")[0], "ios"), "Tesela 1.1 (80)");
  assert.equal(releaseVersionLabel(platformReleaseHistory(parsed, "web")[0], "web"), "Tesela Web");
});
