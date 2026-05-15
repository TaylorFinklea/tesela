/**
 * Reactive wrapper around `pane-tree.ts`. Uses `$state.raw` because the
 * tree is deeply nested and mutations replace the whole structure
 * immutably (matching the proto's reducer style). `$state.raw` skips
 * the proxy machinery and re-renders consumers when the reference
 * changes — exactly what we want here.
 *
 * Persistence: debounced write to localStorage under
 * `tesela:prism4:v1`. On load we attempt deserialize-and-validate;
 * any failure falls through to a fresh `initialState()`.
 */

import * as pt from "./pane-tree";
import type { Pane, PaneKind, PaneTreeState, Tab } from "./pane-tree";
import { pushJourney } from "./journey.svelte";

const DEBOUNCE_MS = 200;

let state = $state.raw<PaneTreeState>(loadOrInit());
let writeTimer: ReturnType<typeof setTimeout> | null = null;

function loadOrInit(): PaneTreeState {
  if (typeof localStorage === "undefined") return pt.initialState();
  const raw = localStorage.getItem(pt.STORAGE_KEY);
  return pt.deserialize(raw) ?? pt.initialState();
}

function schedulePersist() {
  if (typeof localStorage === "undefined") return;
  if (writeTimer !== null) clearTimeout(writeTimer);
  writeTimer = setTimeout(() => {
    try {
      localStorage.setItem(pt.STORAGE_KEY, pt.serialize(state));
    } catch (e) {
      console.warn("pane-tree: persist failed", e);
    }
    writeTimer = null;
  }, DEBOUNCE_MS);
}

// Per-tab "which editor pane was focused most recently" — a `context`
// pane follows this so it shows the right note even when focus has
// moved onto the context pane itself or a widget pane. Not persisted;
// rebuilds as the user focuses editors. `$state` Map so `getLastEditorPaneId`
// reads are reactive for the context pane's `$derived`.
const lastEditorByTab = $state(new Map<string, string>());

function trackLastEditor() {
  const t = pt.focusedTab(state);
  if (!t) return;
  const p = t.layout[t.focus[0]]?.[t.focus[1]];
  if (p?.kind === "editor") lastEditorByTab.set(t.id, p.id);
}

function commit(next: PaneTreeState) {
  if (next === state) return;
  state = next;
  trackLastEditor();
  schedulePersist();
}

// ── readers ────────────────────────────────────────────────────────────────

export function getState(): PaneTreeState { return state; }
export function getFocusedTab(): Tab | undefined { return pt.focusedTab(state); }
export function getFocusedPane(): Pane | undefined { return pt.focusedPane(state); }
export function getFocusedPaneId(): string | undefined { return pt.focusedPane(state)?.id; }
export function getFirstEditorTile(): string | undefined { return pt.firstEditorTile(state); }
export function getPaneById(id: string) { return pt.paneById(state, id); }
export function getTileLocation(tileId: string) { return pt.findTile(state, tileId); }

/** Id of the editor pane focused most recently in the given tab.
 * `context` panes follow this. Undefined until an editor is focused. */
export function getLastEditorPaneId(tabId: string): string | undefined {
  return lastEditorByTab.get(tabId);
}

/**
 * Resolve the editor pane that "navigation" surfaces (graph, widget,
 * dashboard, context, Station, Peek, Journey) should open a tile in.
 *
 * Order of preference:
 *   1. `preferredPaneId` if it's already an editor pane (e.g. Station's
 *      prior-pane restore — when the user invoked ⌘K from an editor).
 *   2. The most-recently-focused editor pane in the current tab —
 *      mirrors neovim's "open in last buffer" semantics for nav panes.
 *   3. Undefined — caller should fall back to converting the current
 *      pane via `jumpToTile` directly.
 */
export function resolveEditorTarget(preferredPaneId?: string): { row: number; col: number } | undefined {
  if (preferredPaneId) {
    const hit = pt.paneById(state, preferredPaneId);
    if (hit?.pane.kind === "editor") return { row: hit.row, col: hit.col };
  }
  const tab = pt.focusedTab(state);
  if (!tab) return undefined;
  const last = lastEditorByTab.get(tab.id);
  if (!last) return undefined;
  const hit = pt.paneById(state, last);
  if (hit?.pane.kind === "editor") return { row: hit.row, col: hit.col };
  return undefined;
}

/**
 * Open `tileId` in the resolved editor pane. Used by every navigation
 * surface: Station note rows, Dashboard widget rows, PeekPopover
 * backlinks / outline / journey, GraphCanvas node picks, Journey
 * breadcrumb chips, the ⌘[ / ⌘] history walk.
 *
 * Wiki-link / drill-in clicks from inside an editor continue to use
 * `jumpToTile` directly so they stay in their current buffer — that's
 * native in-buffer navigation, not "open from a nav surface."
 */
export function openInEditor(tileId: string, opts?: { preferredPaneId?: string; via?: string }) {
  const target = resolveEditorTarget(opts?.preferredPaneId);
  if (target) {
    focusPane(target.row, target.col);
  }
  jumpToTile(tileId, opts?.via ?? "manual");
}

// ── mutations ──────────────────────────────────────────────────────────────

export function focusPane(row: number, col: number) { commit(pt.focusPane(state, row, col)); }
export function moveFocus(dRow: number, dCol: number) { commit(pt.moveFocus(state, dRow, dCol)); }
export function vsplit(kind: PaneKind = "editor") { commit(pt.vsplit(state, kind)); }
export function hsplit(kind: PaneKind = "editor") { commit(pt.hsplit(state, kind)); }
export function closePane() { commit(pt.closePane(state)); }
export function jumpToTile(tileId: string, via: string = "manual") {
  commit(pt.jumpToTile(state, tileId));
  pushJourney(tileId, via);
}
export function stackAdd(tileId: string) {
  commit(pt.stackAdd(state, tileId));
  pushJourney(tileId, "stack");
}
export function stackNext(dir: 1 | -1) { commit(pt.stackNext(state, dir)); }
export function stackClose(idx: number) { commit(pt.stackClose(state, idx)); }
export function swapKind(paneId: string, kind: PaneKind) { commit(pt.swapKind(state, paneId, kind)); }
export function setPaneWidget(paneId: string, widgetId: string) { commit(pt.setPaneWidget(state, paneId, widgetId)); }
export function newTab(name?: string) { commit(pt.newTab(state, name)); }
export function closeTab(tabId: string) { commit(pt.closeTab(state, tabId)); }
export function switchTab(tabId: string) { commit(pt.switchTab(state, tabId)); }
export function switchTabByIndex(index: number) { commit(pt.switchTabByIndex(state, index)); }
export function renameTab(tabId: string, name: string) { commit(pt.renameTab(state, tabId, name)); }
export function moveTab(from: number, to: number) { commit(pt.moveTab(state, from, to)); }
export function setColSizes(r: number, sizes: number[]) { commit(pt.setColSizes(state, r, sizes)); }
export function setRowSizes(sizes: number[]) { commit(pt.setRowSizes(state, sizes)); }

/**
 * Reset to a fresh initialState. Useful for tests + the "wipe layout"
 * power-user command. Persistence fires on the next debounce tick.
 */
export function resetTree() {
  commit(pt.initialState());
}

// ── pane → outliner DOM-element registry ────────────────────────────────────
//
// Imperative lookup table mapping a pane id to the root DOM element of
// the BlockOutliner mounted inside it. Not reactive — consumers look it
// up at event-handling time. This is the primitive that lets later
// phases route document-level events to the right pane: Phase 4's
// Command Station restores focus to the prior pane's outliner, and
// Phase 5's leader tree dispatches `tesela:*` events carrying a paneId.
// Phase 1.5 only populates it; nothing reads it yet.

const outlinerEls = new Map<string, HTMLElement>();

export function registerPaneOutliner(paneId: string, el: HTMLElement) {
  outlinerEls.set(paneId, el);
}

export function unregisterPaneOutliner(paneId: string) {
  outlinerEls.delete(paneId);
}

export function getPaneOutliner(paneId: string): HTMLElement | undefined {
  return outlinerEls.get(paneId);
}
