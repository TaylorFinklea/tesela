/**
 * Prism v4 — fullscreen overlays.
 *
 * Today: graph (`g`) and settings (`⚙` / `:settings-<slug>`). Future
 * overlays (zen-mode editor, presentation view) can extend the
 * `OverlayKind` union without growing the keymap.
 */

export type OverlayKind = "graph" | "settings";

export type SettingsSlug =
  | "general"
  | "mosaic"
  | "data"
  | "sync"
  | "devices"
  | "voice";

let active = $state<OverlayKind | null>(null);
let settingsSlug = $state<SettingsSlug>("general");

export function isOverlayOpen(): boolean {
  return active !== null;
}

export function getActiveOverlay(): OverlayKind | null {
  return active;
}

export function getSettingsSlug(): SettingsSlug {
  return settingsSlug;
}

export function setSettingsSlug(slug: SettingsSlug) {
  settingsSlug = slug;
}

export function openFullscreenGraph() {
  active = "graph";
}

export function openSettingsOverlay(slug: SettingsSlug = "general") {
  settingsSlug = slug;
  active = "settings";
}

export function closeOverlay() {
  active = null;
}
