/**
 * Prism v5 — reactive Svelte 5 wrapper around the buffer tree.
 *
 * Holds workspace state as `$state.raw` (the tree is recursive; whole-state
 * replacement on each mutation matches the immutable algebra). Persistence
 * is debounced; loads on init via `migration.loadFromLocalStorage`.
 *
 * The `focusPane` function is the single chokepoint for the follow-binding
 * rule: it always updates `lastFocusedLeafId`, and updates
 * `lastFocusedPageId` ONLY when the newly focused buffer is a page.
 */

import {
  type Buffer,
  type DerivedBinding,
  type LeafId,
  type SplitId,
  type Tab,
  type TabId,
  type Workspace,
} from "./types.ts";
import {
  activeTab as activeTabOf,
  addTab,
  closeFocused,
  defaultWorkspace,
  findLeaf,
  makeAmbientBuffer,
  makeDerivedBuffer,
  makePageBuffer,
  makeTab,
  movePaneToEdge,
  nextFocusedLeaf,
  removeTab,
  renameTab as renameTabPure,
  replaceLeafBuffer,
  replaceTab,
  setSplitRatio,
  splitFocused,
  switchActiveTab,
} from "./tree.ts";
import {
  asPageId,
  type PageId,
} from "./types.ts";
import { loadFromLocalStorage, saveToLocalStorage } from "./migration.ts";
import { touchRecent } from "../state/shared.svelte.ts";

const DEBOUNCE_MS = 200;

function initialWorkspace(): Workspace {
  const loaded = loadFromLocalStorage();
  if (loaded) return loaded.workspace;
  return defaultWorkspace(makePageBuffer(asPageId("")));
}

let workspace = $state.raw<Workspace>(initialWorkspace());
let writeTimer: ReturnType<typeof setTimeout> | null = null;

function schedulePersist() {
  if (writeTimer !== null) clearTimeout(writeTimer);
  writeTimer = setTimeout(() => {
    saveToLocalStorage(workspace);
    writeTimer = null;
  }, DEBOUNCE_MS);
}

function commit(next: Workspace) {
  if (next === workspace) return;
  workspace = next;
  schedulePersist();
}

// ── readers ────────────────────────────────────────────────────────────────

export function getWorkspace(): Workspace {
  return workspace;
}

export function getActiveTab(): Tab | undefined {
  return activeTabOf(workspace);
}

export function getFocusedLeafId(): LeafId | undefined {
  return activeTabOf(workspace)?.lastFocusedLeafId;
}

export function getFocusedBuffer(): Buffer | undefined {
  const t = activeTabOf(workspace);
  if (!t?.lastFocusedLeafId) return undefined;
  const l = findLeaf(t.layout, t.lastFocusedLeafId);
  return l?.buffer;
}

export function getLastFocusedPageId(): PageId | undefined {
  return activeTabOf(workspace)?.lastFocusedPageId;
}

// ── focus chokepoint ───────────────────────────────────────────────────────

/**
 * Focus a leaf in the active tab. THIS is the only function that mutates
 * `lastFocusedPageId` — and it only does so when the focused leaf's buffer
 * is a page. Derived and ambient leaves are transparent to the follow
 * binding.
 */
export function focusLeaf(leafId: LeafId) {
  commit(focusLeafIn(workspace, workspace.activeTabId, leafId));
}

function focusLeafIn(ws: Workspace, tabId: TabId, leafId: LeafId): Workspace {
  const t = ws.tabs.find((x) => x.id === tabId);
  if (!t) return ws;
  const leaf = findLeaf(t.layout, leafId);
  if (!leaf) return ws;
  // Empty pageId means the page buffer hasn't been seeded yet — it's
  // still a placeholder. Don't pollute the follow source with it; keep
  // the prior real page so derived followers stay useful.
  const nextLastPage =
    leaf.buffer.kind === "page" && leaf.buffer.pageId
      ? leaf.buffer.pageId
      : t.lastFocusedPageId;
  const next: Tab = {
    ...t,
    lastFocusedLeafId: leafId,
    lastFocusedPageId: nextLastPage,
  };
  // Side-effect: page focus updates the recent LRU. Lives in the chokepoint
  // alongside the follow-source mutation; both are gated by the same
  // page-kind + non-empty check.
  if (leaf.buffer.kind === "page" && leaf.buffer.pageId) {
    touchRecent(leaf.buffer.pageId);
  }
  return replaceTab(ws, next);
}

// ── split / close / move ───────────────────────────────────────────────────

export function vsplit(buffer: Buffer) {
  splitWith("v", buffer);
}

export function hsplit(buffer: Buffer) {
  splitWith("h", buffer);
}

function splitWith(dir: "v" | "h", buffer: Buffer) {
  const t = activeTabOf(workspace);
  if (!t || !t.lastFocusedLeafId) return;
  const { layout, newLeafId } = splitFocused(
    t.layout,
    t.lastFocusedLeafId,
    dir,
    buffer,
  );
  const intermediate = replaceTab(workspace, { ...t, layout });
  commit(focusLeafIn(intermediate, t.id, newLeafId));
}

export function closeFocusedLeaf() {
  const t = activeTabOf(workspace);
  if (!t || !t.lastFocusedLeafId) return;
  const r = closeFocused(t.layout, t.lastFocusedLeafId);
  if (!r) return;
  const intermediate = replaceTab(workspace, { ...t, layout: r.layout });
  commit(focusLeafIn(intermediate, t.id, r.nextFocusId));
}

export function moveFocus(dir: "left" | "right" | "up" | "down") {
  const t = activeTabOf(workspace);
  if (!t || !t.lastFocusedLeafId) return;
  const next = nextFocusedLeaf(t.layout, t.lastFocusedLeafId, dir);
  if (next && next !== t.lastFocusedLeafId) focusLeaf(next);
}

export function movePane(dir: "left" | "right" | "up" | "down") {
  const t = activeTabOf(workspace);
  if (!t || !t.lastFocusedLeafId) return;
  const layout = movePaneToEdge(t.layout, t.lastFocusedLeafId, dir);
  if (layout === t.layout) return;
  commit(replaceTab(workspace, { ...t, layout }));
}

export function setRatio(splitId: SplitId, ratio: number) {
  const t = activeTabOf(workspace);
  if (!t) return;
  const layout = setSplitRatio(t.layout, splitId, ratio);
  if (layout === t.layout) return;
  commit(replaceTab(workspace, { ...t, layout }));
}

// ── open verbs ─────────────────────────────────────────────────────────────

/** Replace the focused leaf's buffer with a page. */
export function openPageInFocused(pageId: PageId) {
  const t = activeTabOf(workspace);
  if (!t || !t.lastFocusedLeafId) return;
  openPageInLeaf(t.lastFocusedLeafId, pageId);
}

/** Replace a specific leaf's buffer with a page. Used by the daily seed
 *  flow that needs to target the empty page leaf, not whatever happens to
 *  be focused. */
export function openPageInLeaf(leafId: LeafId, pageId: PageId) {
  const t = activeTabOf(workspace);
  if (!t) return;
  const layout = replaceLeafBuffer(
    t.layout,
    leafId,
    makePageBuffer(pageId),
  );
  const intermediate = replaceTab(workspace, { ...t, layout });
  // If the targeted leaf is currently focused, re-run focusLeafIn to
  // update the follow-binding chokepoint with the new pageId.
  if (t.lastFocusedLeafId === leafId) {
    commit(focusLeafIn(intermediate, t.id, leafId));
  } else {
    commit(intermediate);
  }
}

/** Replace the focused leaf's buffer with a derived buffer. */
export function openDerivedInFocused(
  rendererName: string,
  binding: DerivedBinding,
) {
  const t = activeTabOf(workspace);
  if (!t || !t.lastFocusedLeafId) return;
  const layout = replaceLeafBuffer(
    t.layout,
    t.lastFocusedLeafId,
    makeDerivedBuffer(rendererName, binding),
  );
  commit(replaceTab(workspace, { ...t, layout }));
}

/** Replace the focused leaf's buffer with an ambient buffer. */
export function openAmbientInFocused(ambientName: string) {
  const t = activeTabOf(workspace);
  if (!t || !t.lastFocusedLeafId) return;
  const layout = replaceLeafBuffer(
    t.layout,
    t.lastFocusedLeafId,
    makeAmbientBuffer(ambientName),
  );
  commit(replaceTab(workspace, { ...t, layout }));
}

// ── tabs ───────────────────────────────────────────────────────────────────

export function newTab(name: string = "untitled") {
  const tab = makeTab(name, makePageBuffer(asPageId("")));
  commit(addTab(workspace, tab, true));
}

export function closeTab(tabId: TabId) {
  commit(removeTab(workspace, tabId));
}

export function switchTab(tabId: TabId) {
  commit(switchActiveTab(workspace, tabId));
}

export function switchTabByIndex(index: number) {
  const tab = workspace.tabs[index];
  if (tab) commit(switchActiveTab(workspace, tab.id));
}

export function rename(tabId: TabId, name: string) {
  commit(renameTabPure(workspace, tabId, name));
}

// ── sidebar ────────────────────────────────────────────────────────────────

export function setSidebarCollapsed(collapsed: boolean) {
  if (workspace.sidebar.collapsed === collapsed) return;
  commit({ ...workspace, sidebar: { ...workspace.sidebar, collapsed } });
}

export function setSidebarSurface(surface: Workspace["sidebar"]["activeSurface"]) {
  if (workspace.sidebar.activeSurface === surface) return;
  commit({ ...workspace, sidebar: { ...workspace.sidebar, activeSurface: surface } });
}

// ── reset (testing / dev) ──────────────────────────────────────────────────

export function resetWorkspace() {
  commit(defaultWorkspace(makePageBuffer(asPageId(""))));
}
