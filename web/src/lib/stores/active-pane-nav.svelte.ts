/**
 * Phase 9.5 — single navigation helper that respects the active vsplit side.
 *
 * Three call sites use this: rail clicks (`Rail.svelte`), ⌘K palette
 * navigation (`CommandPalette.svelte`), and wiki-link clicks inside cm6
 * (`cm-decorations.ts` / `BlockEditor.svelte`'s click handler).
 *
 * Behavior:
 *   - When the vertical split is closed OR the active side is "left", navigate
 *     normally — replace the path. This matches pre-9.5 behavior.
 *   - When vsplit is open and active side is "right", keep the path (left
 *     pane's note) and update only `?right=<id>&rightBlock=<id?>` so the right
 *     pane navigates independently.
 */
import { goto } from "$app/navigation";
import { page } from "$app/state";
import {
  isVSplitOpen,
  getVSplitActiveSide,
} from "$lib/stores/pane-state.svelte";

/**
 * Navigate to a note. When the right pane is the active side of an open
 * vsplit, the right pane is updated; otherwise the left pane (path) is.
 */
export function gotoNote(noteId: string, blockId?: string | null): void {
  const targetRight =
    isVSplitOpen() && getVSplitActiveSide() === "right" && page.url.pathname.startsWith("/p/");

  if (targetRight) {
    const params = new URLSearchParams(page.url.search);
    params.set("right", noteId);
    if (blockId) params.set("rightBlock", blockId);
    else params.delete("rightBlock");
    goto(`${page.url.pathname}?${params.toString()}`, { replaceState: false, noScroll: true });
    return;
  }

  // Default: navigate the left pane (path).
  const path = `/p/${encodeURIComponent(noteId)}`;
  if (blockId) {
    const params = new URLSearchParams();
    params.set("block", blockId);
    goto(`${path}?${params.toString()}`, { replaceState: false, noScroll: true });
  } else {
    goto(path);
  }
}
