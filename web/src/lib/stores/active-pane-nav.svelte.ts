/**
 * Phase 9.5b — column-view navigation entry points.
 *
 * Every navigation in the app goes through `gotoNote`. The drill rule:
 *
 *   Source pane content → new left.   Target → new right.
 *   The non-source pane is dropped.
 *
 * "Source pane" = whatever the active pane currently shows. After a drill,
 * focus shifts to the right pane (the new target).
 *
 * URL spec:
 *   /p/<R>?block=<RB>?back=<L>&backBlock=<LB>
 *
 * `path` + `?block=` is the right (current) pane. `?back=` + `?backBlock=`
 * is the left (back-context) pane. Absent `?back=` means full-screen.
 *
 * Two collapse helpers:
 *   - `collapseSplit()` — drop ?back=, full-screen the current path (right).
 *   - `goBack()` — full-screen the LEFT pane (drop right, navigate to back).
 *
 * `^w q` and Esc both call `goBack()` (the "go to where I came from" model).
 * Kanban-mutex calls `collapseSplit()` (auto-kanban supersedes column-view).
 */
import { goto } from "$app/navigation";
import { page } from "$app/state";
import {
  getVSplitActiveSide,
  setVSplitActiveSide,
} from "$lib/stores/pane-state.svelte";

function noteIdFromPath(pathname: string): string {
  if (!pathname.startsWith("/p/")) return "";
  return decodeURIComponent(pathname.slice(3));
}

/**
 * Drill from the active pane to a new target. Source = active pane's
 * current content; target promotes to the right; source becomes the new
 * left; the non-source pane is dropped.
 *
 * No-op if drilling to the exact same target as the current right.
 */
export function gotoNote(targetNoteId: string, targetBlockId?: string | null): void {
  const u = page.url;
  const onPagePath = u.pathname.startsWith("/p/");
  const splitOpen = !!u.searchParams.get("back");
  const activeSide = getVSplitActiveSide();

  let sourceNoteId: string | undefined;
  let sourceBlockId: string | undefined;
  if (splitOpen && activeSide === "left") {
    sourceNoteId = u.searchParams.get("back") ?? undefined;
    sourceBlockId = u.searchParams.get("backBlock") ?? undefined;
  } else if (onPagePath) {
    sourceNoteId = noteIdFromPath(u.pathname);
    sourceBlockId = u.searchParams.get("block") ?? undefined;
  }

  // No-op when target matches current right exactly.
  const currentR = noteIdFromPath(u.pathname);
  const currentRBlock = u.searchParams.get("block") ?? "";
  if (targetNoteId === currentR && (targetBlockId ?? "") === currentRBlock) return;

  const params = new URLSearchParams();
  if (targetBlockId) params.set("block", targetBlockId);
  if (sourceNoteId) {
    params.set("back", sourceNoteId);
    if (sourceBlockId) params.set("backBlock", sourceBlockId);
  }
  const qs = params.toString();
  const newPath = `/p/${encodeURIComponent(targetNoteId)}${qs ? `?${qs}` : ""}`;

  goto(newPath, { replaceState: false, noScroll: true });
  // After every drill, focus lands in the right pane.
  setVSplitActiveSide("right");
}

/**
 * Drop ?back= and full-screen the current path (right pane). Used by the
 * kanban-mutex when auto-kanban needs to take over the focus region.
 */
export function collapseSplit(): void {
  const u = page.url;
  if (!u.searchParams.get("back")) return;
  const params = new URLSearchParams(u.search);
  params.delete("back");
  params.delete("backBlock");
  const qs = params.toString();
  goto(`${u.pathname}${qs ? `?${qs}` : ""}`, { replaceState: false, noScroll: true });
  setVSplitActiveSide("right");
}

/**
 * Go back: full-screen the LEFT pane (drop the right). Triggered by `^w q`
 * and Esc-when-right-active-and-vim-NORMAL.
 */
export function goBack(): void {
  const u = page.url;
  const back = u.searchParams.get("back");
  if (!back) return;
  const backBlock = u.searchParams.get("backBlock");
  const params = new URLSearchParams();
  if (backBlock) params.set("block", backBlock);
  const qs = params.toString();
  goto(`/p/${encodeURIComponent(back)}${qs ? `?${qs}` : ""}`, { replaceState: false, noScroll: true });
  // Single-pane is semantically "right" — keeps active-side consistent.
  setVSplitActiveSide("right");
}
