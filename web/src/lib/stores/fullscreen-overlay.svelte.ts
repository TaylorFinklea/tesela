/**
 * Prism v4 — fullscreen overlays (Phase 5).
 *
 * Today: just the graph (`g` opens it). Future overlays (e.g. zen-mode
 * editor, presentation view) can extend the `OverlayKind` union and
 * the FullscreenOverlay component without growing the keymap.
 */

export type OverlayKind = "graph";

let active = $state<OverlayKind | null>(null);

export function isOverlayOpen(): boolean {
  return active !== null;
}

export function getActiveOverlay(): OverlayKind | null {
  return active;
}

export function openFullscreenGraph() {
  active = "graph";
}

export function closeOverlay() {
  active = null;
}
