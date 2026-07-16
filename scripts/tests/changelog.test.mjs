import assert from "node:assert/strict";
import test from "node:test";

import {
  buildUpdaterManifest,
  platformHistory,
  renderRelease,
  validateCatalog,
} from "../changelog-lib.mjs";

function validCatalog() {
  return {
    schemaVersion: 1,
    current: {
      web: "2026-07-15.web-desktop",
      desktop: "2026-07-15.web-desktop",
      ios: "2026-07-15.ios",
    },
    releases: [
      {
        id: "2026-07-15.ios",
        publishedAt: "2026-07-15T18:00:00Z",
        title: "Native polish",
        summary: "A clearer iPhone experience.",
        platforms: ["ios"],
        versions: { ios: { marketing: "1.1", build: "80" } },
        new: ["A new native view."],
        fixed: [],
        important: [],
      },
      {
        id: "2026-07-15.web-desktop",
        publishedAt: "2026-07-15T17:00:00Z",
        title: "Sharper daily work",
        summary: "Move work between days with confidence.",
        platforms: ["web", "desktop"],
        versions: { desktop: "0.1.2" },
        new: ["Move a whole subtree."],
        fixed: ["Quotes like \"this\", Unicode ✓, and\nnewlines stay intact."],
        important: [],
      },
      {
        id: "2026-07-02.web-desktop",
        publishedAt: "2026-07-02T12:00:00Z",
        title: "Find and shape",
        summary: "Search and saved views go deeper.",
        platforms: ["web", "desktop"],
        versions: { desktop: "0.1.1" },
        new: ["Search note content."],
        fixed: [],
        important: ["Desktop updates are signed."],
      },
      {
        id: "2026-06-04.web-desktop",
        publishedAt: "2026-06-04T12:00:00Z",
        title: "Graphite workspace",
        summary: "Daily notes, agenda, views, and library share one shell.",
        platforms: ["web", "desktop"],
        versions: { desktop: "0.1.0" },
        new: ["A keyboard-first workspace."],
        fixed: [],
        important: [],
      },
      {
        id: "2026-05-30.ios",
        publishedAt: "2026-05-30T12:00:00Z",
        title: "Earlier iOS release",
        summary: "An older native milestone.",
        platforms: ["ios"],
        versions: { ios: { marketing: "1.0", build: "10" } },
        new: ["Native daily notes."],
        fixed: [],
        important: [],
      },
    ],
  };
}

function copy(value) {
  return structuredClone(value);
}

function assertInvalid(name, mutate, pattern) {
  test(name, () => {
    const catalog = validCatalog();
    mutate(catalog);
    assert.throws(() => validateCatalog(catalog), pattern);
  });
}

test("validates a schema-1 catalog and exact platform artifacts", () => {
  const catalog = validCatalog();
  assert.equal(validateCatalog(catalog), catalog);
  assert.equal(
    validateCatalog(catalog, { platform: "desktop", version: "0.1.2" }),
    catalog,
  );
  assert.equal(
    validateCatalog(catalog, { platform: "ios", version: "1.1", build: "80" }),
    catalog,
  );
  assert.equal(validateCatalog(catalog, { platform: "web" }), catalog);
});

assertInvalid("rejects an unknown schema", (c) => { c.schemaVersion = 2; }, /schemaVersion/);
assertInvalid("rejects malformed top-level keys", (c) => { c.surprise = true; }, /top-level.*surprise/i);
assertInvalid("rejects duplicate release ids", (c) => { c.releases[1].id = c.releases[0].id; }, /duplicate.*id/i);
assertInvalid("rejects invalid timestamps", (c) => { c.releases[0].publishedAt = "yesterday"; }, /publishedAt/);
assertInvalid("rejects non-descending timestamps", (c) => {
  c.releases[1].publishedAt = "2026-07-16T00:00:00Z";
}, /newest-first/);
assertInvalid("rejects invalid platforms", (c) => { c.releases[0].platforms = ["watchos"]; }, /platforms/);
assertInvalid("rejects duplicate platforms", (c) => { c.releases[0].platforms = ["ios", "ios"]; }, /duplicate.*platform/i);
assertInvalid("rejects missing current pointers", (c) => { delete c.current.web; }, /current\.web/);
assertInvalid("rejects unknown current targets", (c) => { c.current.web = "missing"; }, /current\.web.*missing/);
assertInvalid("rejects current targets for the wrong platform", (c) => {
  c.current.web = "2026-07-15.ios";
}, /current\.web.*platform/);
assertInvalid("rejects desktop releases without a version", (c) => {
  delete c.releases[1].versions.desktop;
}, /versions\.desktop/);
assertInvalid("rejects iOS releases without a version pair", (c) => {
  delete c.releases[0].versions.ios.build;
}, /versions\.ios\.build/);
assertInvalid("rejects blank release copy", (c) => { c.releases[0].title = "  "; }, /title/);
assertInvalid("rejects blank section items", (c) => { c.releases[0].new = [" "]; }, /new\[0\]/);
assertInvalid("rejects releases with no change items", (c) => { c.releases[0].new = []; }, /change item/);

test("rejects platform artifact mismatches and incomplete artifact arguments", () => {
  const catalog = validCatalog();
  assert.throws(
    () => validateCatalog(catalog, { platform: "desktop", version: "9.9.9" }),
    /desktop version.*0\.1\.2.*9\.9\.9/i,
  );
  assert.throws(
    () => validateCatalog(catalog, { platform: "ios", version: "1.1", build: "79" }),
    /iOS build.*80.*79/i,
  );
  assert.throws(
    () => validateCatalog(catalog, { platform: "ios", version: "1.1" }),
    /--build/,
  );
  assert.throws(
    () => validateCatalog(catalog, { platform: "web", version: "1" }),
    /web.*version/i,
  );
});

test("platform history starts at current and excludes newer other-platform releases", () => {
  const catalog = validCatalog();
  assert.deepEqual(
    platformHistory(catalog, "desktop").map((release) => release.id),
    [
      "2026-07-15.web-desktop",
      "2026-07-02.web-desktop",
      "2026-06-04.web-desktop",
    ],
  );
  assert.deepEqual(
    platformHistory(catalog, "ios").map((release) => release.id),
    ["2026-07-15.ios", "2026-05-30.ios"],
  );
});

test("renders stable Markdown and plain notes while omitting empty groups", () => {
  const release = validCatalog().releases[1];
  assert.equal(
    renderRelease(release, "markdown"),
    [
      "# Sharper daily work",
      "",
      "Move work between days with confidence.",
      "",
      "## New",
      "",
      "- Move a whole subtree.",
      "",
      "## Fixed",
      "",
      "- Quotes like \"this\", Unicode ✓, and\n  newlines stay intact.",
      "",
    ].join("\n"),
  );
  assert.equal(
    renderRelease(release, "plain"),
    [
      "Sharper daily work",
      "",
      "Move work between days with confidence.",
      "",
      "NEW",
      "• Move a whole subtree.",
      "",
      "FIXED",
      "• Quotes like \"this\", Unicode ✓, and\n  newlines stay intact.",
      "",
    ].join("\n"),
  );
  assert.doesNotMatch(renderRelease(release, "plain"), /IMPORTANT/);
  assert.throws(() => renderRelease(release, "html"), /format/);
});

test("builds parseable updater JSON without interpolating release-note text", () => {
  const notes = renderRelease(validCatalog().releases[1], "plain");
  const json = buildUpdaterManifest({
    version: "0.1.2",
    notes,
    pubDate: "2026-07-15T18:00:00Z",
    target: "darwin-aarch64",
    signature: "sig\nwith newline",
    url: "https://example.test/Tesela.app.tar.gz",
  });
  const parsed = JSON.parse(json);
  assert.equal(parsed.notes, notes);
  assert.equal(parsed.platforms["darwin-aarch64"].signature, "sig\nwith newline");
  assert.equal(`${json.endsWith("\n")}`, "true");
});

test("validation does not mutate the input", () => {
  const catalog = validCatalog();
  const before = copy(catalog);
  validateCatalog(catalog);
  assert.deepEqual(catalog, before);
});
