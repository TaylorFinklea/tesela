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

// Suppress the layout's `beforeNavigate` drill interceptor when we're
// programmatically navigating to a URL we already constructed correctly.
// SvelteKit reports type === "goto" for both link clicks (via its internal
// link interceptor) and direct goto() calls, so we use this flag to tell
// them apart.
let internalNavInFlight = false;

export function isInternalNavInFlight(): boolean {
  return internalNavInFlight;
}

function programmaticGoto(url: string, opts: Parameters<typeof goto>[1]): void {
  internalNavInFlight = true;
  void goto(url, opts).finally(() => {
    internalNavInFlight = false;
  });
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

  // Buffer-tree short-circuit: when the active route is the Graphite /g
  // shell, open the target directly in the focused buffer instead of
  // rewriting the URL. The buffer tree owns navigation; the URL is just
  // for deep-link entry. Without this, wiki-link clicks set the URL but
  // the chrome doesn't observe it and nothing visibly changes.
  if (u.pathname === "/g" || u.pathname.startsWith("/g/")) {
    // Lazy import to avoid a hard dep cycle: this module is also
    // imported by legacy v9 components that don't know about v5.
    import("$lib/buffer/state.svelte").then(({ openPageInFocused }) => {
      import("$lib/buffer/types").then(({ asPageId }) => {
        openPageInFocused(asPageId(targetNoteId));
      });
    });
    return;
  }

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

  programmaticGoto(newPath, { replaceState: false, noScroll: true });
  // After every drill, focus lands in the right pane.
  setVSplitActiveSide("right");
  // Phase 9.7 — explicitly move DOM focus to the new right pane's cm-editor.
  // Without this, the cm-editor that was focused before the drill keeps DOM
  // focus, so vim chords go to the wrong pane.
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      document.dispatchEvent(
        new CustomEvent("tesela:focus-pane", { detail: { side: "right" } }),
      );
    });
  });
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
  programmaticGoto(`${u.pathname}${qs ? `?${qs}` : ""}`, { replaceState: false, noScroll: true });
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
  programmaticGoto(`/p/${encodeURIComponent(back)}${qs ? `?${qs}` : ""}`, { replaceState: false, noScroll: true });
  // Single-pane is semantically "right" — keeps active-side consistent.
  setVSplitActiveSide("right");
}
