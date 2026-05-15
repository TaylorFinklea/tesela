/**
 * Reactive wrapper around the binary-tree `pane-tree.ts`. Uses
 * `$state.raw` because the tree is recursive and mutations replace the
 * whole structure immutably. Persistence: debounced write to localStorage
 * under `tesela:prism4:v1`. On load, `deserialize` migrates legacy v1
 * (2D matrix) state into the v2 tree shape on the fly.
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
// pane follows this so it shows the right note even when focus has moved
// onto the context pane or a widget pane. Rebuilds as the user focuses
// editors; not persisted. `$state` Map so reads stay reactive.
const lastEditorByTab = $state(new Map<string, string>());

function trackLastEditor() {
  const t = pt.focusedTab(state);
  if (!t) return;
  const leaf = pt.findLeafByPaneId(t.layout, t.focus);
  if (leaf?.pane.kind === "editor") lastEditorByTab.set(t.id, leaf.pane.id);
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

/** Id of the editor pane focused most recently in the given tab. */
export function getLastEditorPaneId(tabId: string): string | undefined {
  return lastEditorByTab.get(tabId);
}

/**
 * Resolve the editor pane that "navigation" surfaces (graph, widget,
 * dashboard, context, Station, Peek, Journey) should open a tile in.
 *
 *   1. `preferredPaneId` if it's already an editor pane.
 *   2. The most-recently-focused editor in the current tab.
 *   3. Undefined — caller should fall back to converting the current pane.
 */
export function resolveEditorTarget(preferredPaneId?: string): string | undefined {
  if (preferredPaneId) {
    const hit = pt.paneById(state, preferredPaneId);
    if (hit?.pane.kind === "editor") return hit.pane.id;
  }
  const tab = pt.focusedTab(state);
  if (!tab) return undefined;
  const last = lastEditorByTab.get(tab.id);
  if (!last) return undefined;
  const hit = pt.paneById(state, last);
  if (hit?.pane.kind === "editor") return hit.pane.id;
  return undefined;
}

/**
 * Open `tileId` in the resolved editor pane. Falls back to `jumpToTile`
 * on the focused pane when no editor exists in the tab — that path
 * implicitly converts the focused pane to an editor, so the user still
 * lands somewhere.
 */
export function openInEditor(tileId: string, opts?: { preferredPaneId?: string; via?: string }) {
  const target = resolveEditorTarget(opts?.preferredPaneId);
  if (target) {
    focusPane(target);
  }
  jumpToTile(tileId, opts?.via ?? "manual");
}

// ── mutations ──────────────────────────────────────────────────────────────

export function focusPane(paneId: string) { commit(pt.focusPane(state, paneId)); }
export function moveFocus(dir: "left" | "right" | "up" | "down") { commit(pt.moveFocus(state, dir)); }
export function vsplit(kind: PaneKind = "editor") { commit(pt.vsplit(state, kind)); }
export function hsplit(kind: PaneKind = "editor") { commit(pt.hsplit(state, kind)); }
export function closePane() { commit(pt.closePane(state)); }
export function movePane(dir: "left" | "right" | "up" | "down") { commit(pt.movePane(state, dir)); }
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
export function setSplitSizes(splitId: string, sizes: number[]) { commit(pt.setSplitSizes(state, splitId, sizes)); }

export function resetTree() {
  commit(pt.initialState());
}

// ── pane → outliner DOM-element registry ───────────────────────────────────
//
// Imperative lookup table mapping a pane id to the root DOM element of
// the BlockOutliner mounted inside it. Consumers look it up at event-
// handling time — Phase 4's Command Station restores focus to the prior
// pane's outliner this way.

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
