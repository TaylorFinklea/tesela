import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const root = new URL("../../", import.meta.url);

async function source(path) {
  return readFile(new URL(path, root), "utf8");
}

test("fullscreen overlay exposes the release-notes kind and shared opener", async () => {
  const [store, shell] = await Promise.all([
    source("src/lib/stores/fullscreen-overlay.svelte.ts"),
    source("src/lib/components/shell/FullscreenOverlay.svelte"),
  ]);
  assert.match(store, /"release-notes"/);
  assert.match(store, /export function openReleaseNotesOverlay\(/);
  assert.match(shell, /ReleaseNotesOverlay/);
  assert.match(shell, /kind === "release-notes"/);
});

test("release-notes overlay contains latest, history, detail, close, and fallback states", async () => {
  const overlay = await source("src/lib/components/shell/ReleaseNotesOverlay.svelte");
  for (const text of [
    "What’s New",
    "New",
    "Fixed",
    "Important",
    "View older releases",
    "Release notes unavailable",
    "Done",
    "Back",
  ]) {
    assert.match(overlay, new RegExp(text));
  }
  assert.match(overlay, /markCurrentRendered\(\)/);
  assert.match(overlay, /previousFocus/);
  assert.match(overlay, /data-release-notes-detail/);
});

test("Settings General has a dynamic About entry opening the shared surface", async () => {
  const settings = await source("src/routes/settings/general/+page.svelte");
  assert.match(settings, /data-release-notes-entry/);
  assert.match(settings, /releaseVersionLabel/);
  assert.match(settings, /openReleaseNotesOverlay/);
  assert.match(settings, /What’s New/);
});

test("shared command registry exposes whats-new through the same opener", async () => {
  const commands = await source("src/lib/commands/index.ts");
  assert.match(commands, /id: "whats-new"/);
  assert.match(commands, /verb: "whats-new"/);
  assert.match(commands, /label: "What’s New"/);
  assert.match(commands, /run: \(\) => openReleaseNotesOverlay\(\)/);
});

test("Graphite shell performs pure seen-state auto-open after hydration", async () => {
  const shell = await source("src/lib/graphite/shell/GraphiteShell.svelte");
  assert.match(shell, /new ReleaseNotesSeenState/);
  assert.match(shell, /shouldAutoPresent\(\)/);
  assert.match(shell, /openReleaseNotesOverlay\(\)/);
  assert.match(shell, /loadBundledReleaseNotes\(\)/);
});
