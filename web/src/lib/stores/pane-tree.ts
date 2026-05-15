/**
 * Prism v4 pane-tree state — binary-tree splits.
 *
 * Each tab's layout is a recursive split tree (same shape tmux / zellij
 * / Aerospace use):
 *
 *   tab    = { id, name, layout: LayoutNode, focus: paneId }
 *   layout = LeafNode { pane }
 *          | SplitNode { dir: "vertical"|"horizontal", children, sizes }
 *
 * vsplit on the focused leaf splits *just that leaf*, not the row it
 * lives in — so a left sidebar can stay full-height while you split
 * the main editor area. Mutations are pure (state) -> state functions;
 * the reactive Svelte wrapper lives next door.
 */

export type PaneKind = "editor" | "widget" | "context" | "graph" | "dashboard";

export type EditorPane = {
  id: string;
  kind: "editor";
  /** Tile ids stacked Zellij-style; `[ ]` cycles, `⇧S` adds. */
  tiles: string[];
  activeIdx: number;
};

export type WidgetPane = {
  id: string;
  kind: "widget";
  /** Id of the Query-type note that defines this widget. */
  widget: string;
};

export type ContextPane = {
  id: string;
  kind: "context";
  /** When null, follows the per-tab most-recently-focused editor pane. */
  tile: string | null;
};

export type GraphPane = { id: string; kind: "graph" };
export type DashboardPane = { id: string; kind: "dashboard" };

export type Pane = EditorPane | WidgetPane | ContextPane | GraphPane | DashboardPane;

export type SplitDir = "vertical" | "horizontal";

export type LeafNode = {
  kind: "leaf";
  pane: Pane;
};
export type SplitNode = {
  id: string;
  kind: "split";
  /** vertical = children laid out side-by-side (vsplit produces this).
   *  horizontal = children stacked top-to-bottom (hsplit produces this). */
  dir: SplitDir;
  children: LayoutNode[];
  /** One weight per child. Length always === children.length. */
  sizes: number[];
};
export type LayoutNode = LeafNode | SplitNode;

export type Tab = {
  id: string;
  name: string;
  layout: LayoutNode;
  focus: string; // pane id
};

export type PaneTreeState = {
  version: number;
  tabs: Tab[];
  activeTabId: string;
};

/**
 * Bump when the on-disk shape changes. The Phase 6 binary-tree refactor
 * jumps to v2; `deserialize` keeps a v1 (Pane[][]) migration path.
 */
export const STATE_VERSION = 2;
export const STORAGE_KEY = "tesela:prism4:v1";

/** Minimum weight allocated to any pane during a drag. */
export const MIN_PANE_WEIGHT = 0.05;

// ── id minting ──────────────────────────────────────────────────────────────

export function mkPaneId(): string {
  return "p" + Math.random().toString(36).slice(2, 9);
}
export function mkSplitId(): string {
  return "s" + Math.random().toString(36).slice(2, 9);
}
export function mkTabId(): string {
  return "tab-" + Math.random().toString(36).slice(2, 9);
}

// ── factories ───────────────────────────────────────────────────────────────

export const DEFAULT_WIDGET = "recent";

export function makePane(kind: PaneKind): Pane {
  switch (kind) {
    case "editor":    return { id: mkPaneId(), kind, tiles: [], activeIdx: 0 };
    case "widget":    return { id: mkPaneId(), kind, widget: DEFAULT_WIDGET };
    case "context":   return { id: mkPaneId(), kind, tile: null };
    case "graph":     return { id: mkPaneId(), kind };
    case "dashboard": return { id: mkPaneId(), kind };
  }
}

export function makeLeaf(kind: PaneKind = "editor"): LeafNode {
  return { kind: "leaf", pane: makePane(kind) };
}

export function makeSplit(dir: SplitDir, children: LayoutNode[]): SplitNode {
  const sizes = children.map(() => 1 / children.length);
  return { id: mkSplitId(), kind: "split", dir, children, sizes };
}

export function makeTab(name: string = "untitled"): Tab {
  const leaf = makeLeaf("editor");
  return {
    id: mkTabId(),
    name,
    layout: leaf,
    focus: leaf.pane.id,
  };
}

export function initialState(): PaneTreeState {
  const tab = makeTab();
  return {
    version: STATE_VERSION,
    tabs: [tab],
    activeTabId: tab.id,
  };
}

// ── traversal ──────────────────────────────────────────────────────────────

/** Iterate every leaf in pre-order. */
export function* leaves(node: LayoutNode): Generator<LeafNode> {
  if (node.kind === "leaf") {
    yield node;
  } else {
    for (const c of node.children) yield* leaves(c);
  }
}

/** Walk the tree depth-first and return the path of indices from `root`
 *  to a node matching the predicate, plus the node itself. */
export function findPath(
  root: LayoutNode,
  pred: (n: LayoutNode) => boolean,
): { node: LayoutNode; path: number[] } | undefined {
  if (pred(root)) return { node: root, path: [] };
  if (root.kind === "leaf") return undefined;
  for (let i = 0; i < root.children.length; i++) {
    const hit = findPath(root.children[i], pred);
    if (hit) return { node: hit.node, path: [i, ...hit.path] };
  }
  return undefined;
}

export function findLeafByPaneId(root: LayoutNode, paneId: string): LeafNode | undefined {
  for (const l of leaves(root)) if (l.pane.id === paneId) return l;
  return undefined;
}

/** Returns the immediate parent split of the leaf identified by pane id,
 *  plus the index of the leaf within its parent. Returns undefined when
 *  the leaf is the root (no parent). */
export function findParentOf(
  root: LayoutNode,
  paneId: string,
): { parent: SplitNode; index: number } | undefined {
  if (root.kind === "leaf") return undefined;
  for (let i = 0; i < root.children.length; i++) {
    const c = root.children[i];
    if (c.kind === "leaf" && c.pane.id === paneId) return { parent: root, index: i };
    if (c.kind === "split") {
      const hit = findParentOf(c, paneId);
      if (hit) return hit;
    }
  }
  return undefined;
}

// ── lookups ─────────────────────────────────────────────────────────────────

export function focusedTab(state: PaneTreeState): Tab | undefined {
  return state.tabs.find((t) => t.id === state.activeTabId);
}

export function focusedPane(state: PaneTreeState): Pane | undefined {
  const t = focusedTab(state);
  if (!t) return undefined;
  const leaf = findLeafByPaneId(t.layout, t.focus);
  return leaf?.pane;
}

export function paneById(
  state: PaneTreeState,
  paneId: string,
): { tab: Tab; pane: Pane } | undefined {
  for (const tab of state.tabs) {
    const leaf = findLeafByPaneId(tab.layout, paneId);
    if (leaf) return { tab, pane: leaf.pane };
  }
  return undefined;
}

/** First tile id of the first editor pane in the active tab. */
export function firstEditorTile(state: PaneTreeState): string | undefined {
  const t = focusedTab(state);
  if (!t) return undefined;
  for (const l of leaves(t.layout)) {
    if (l.pane.kind === "editor" && l.pane.tiles.length > 0) {
      return l.pane.tiles[l.pane.activeIdx];
    }
  }
  return undefined;
}

/** Locate an editor pane whose tile stack contains `tileId`, searching
 *  every tab. Returns the first hit's tab id + pane id, or undefined. */
export function findTile(
  state: PaneTreeState,
  tileId: string,
): { tabId: string; paneId: string } | undefined {
  for (const tab of state.tabs) {
    for (const l of leaves(tab.layout)) {
      if (l.pane.kind === "editor" && l.pane.tiles.includes(tileId)) {
        return { tabId: tab.id, paneId: l.pane.id };
      }
    }
  }
  return undefined;
}

// ── internal helpers ───────────────────────────────────────────────────────

function replaceTab(state: PaneTreeState, tabId: string, fn: (t: Tab) => Tab): PaneTreeState {
  let changed = false;
  const tabs = state.tabs.map((t) => {
    if (t.id !== tabId) return t;
    const next = fn(t);
    if (next !== t) changed = true;
    return next;
  });
  return changed ? { ...state, tabs } : state;
}

/** Immutable tree update — runs `fn` on every node matching `pred` (DFS).
 *  Returns the (possibly unchanged) root. */
function updateNode(
  root: LayoutNode,
  pred: (n: LayoutNode) => boolean,
  fn: (n: LayoutNode) => LayoutNode,
): LayoutNode {
  if (pred(root)) return fn(root);
  if (root.kind === "leaf") return root;
  let changed = false;
  const next = root.children.map((c) => {
    const updated = updateNode(c, pred, fn);
    if (updated !== c) changed = true;
    return updated;
  });
  return changed ? { ...root, children: next } : root;
}

/** Collapse a split with only one child into that child. Bottom-up so a
 *  chain of single-child splits all collapse in one pass. */
function collapse(root: LayoutNode): LayoutNode {
  if (root.kind === "leaf") return root;
  let changed = false;
  const cs = root.children.map((c) => {
    const u = collapse(c);
    if (u !== c) changed = true;
    return u;
  });
  if (cs.length === 1) return cs[0];
  return changed ? { ...root, children: cs } : root;
}

function findFirstLeaf(n: LayoutNode): LeafNode {
  if (n.kind === "leaf") return n;
  return findFirstLeaf(n.children[0]);
}

/** Spatial-neighbor lookup. Walks up from the focused leaf to find the
 *  nearest ancestor whose split direction matches the requested motion,
 *  then descends into the sibling on that side. Returns undefined when
 *  no neighbor exists in the requested direction. */
function neighborLeaf(
  root: LayoutNode,
  paneId: string,
  dir: "left" | "right" | "up" | "down",
): LeafNode | undefined {
  const hit = findPath(root, (n) => n.kind === "leaf" && n.pane.id === paneId);
  if (!hit) return undefined;
  const path = hit.path;
  const axis: SplitDir = dir === "left" || dir === "right" ? "vertical" : "horizontal";
  const stride = dir === "left" || dir === "up" ? -1 : 1;
  // Walk from root down to the focused leaf, remembering each ancestor split.
  let node: LayoutNode = root;
  const ancestors: SplitNode[] = [];
  for (const idx of path) {
    if (node.kind !== "split") break;
    ancestors.push(node);
    node = node.children[idx];
  }
  for (let i = ancestors.length - 1; i >= 0; i--) {
    const a = ancestors[i];
    if (a.dir !== axis) continue;
    const idxInA = path[i];
    const targetIdx = idxInA + stride;
    if (targetIdx < 0 || targetIdx >= a.children.length) continue;
    return descendToLeaf(a.children[targetIdx], dir);
  }
  return undefined;
}

/** Reach into a subtree, preferring the edge child closest to the motion's
 *  origin. (Coming from the right via `left` → land on the rightmost leaf.) */
function descendToLeaf(
  node: LayoutNode,
  fromDir: "left" | "right" | "up" | "down",
): LeafNode {
  if (node.kind === "leaf") return node;
  const axis: SplitDir = fromDir === "left" || fromDir === "right" ? "vertical" : "horizontal";
  if (node.dir === axis) {
    const idx = fromDir === "left" || fromDir === "up" ? node.children.length - 1 : 0;
    return descendToLeaf(node.children[idx], fromDir);
  }
  return descendToLeaf(node.children[0], fromDir);
}

// ── focus + split mutations ────────────────────────────────────────────────

export function focusPane(state: PaneTreeState, paneId: string): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    if (t.focus === paneId) return t;
    if (!findLeafByPaneId(t.layout, paneId)) return t;
    return { ...t, focus: paneId };
  });
}

export function moveFocus(
  state: PaneTreeState,
  dir: "left" | "right" | "up" | "down",
): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const target = neighborLeaf(t.layout, t.focus, dir);
    if (!target || target.pane.id === t.focus) return t;
    return { ...t, focus: target.pane.id };
  });
}

/** Insert a new leaf next to the focused leaf along `dir`. If the
 *  parent split is already aligned with `dir`, the new leaf joins as a
 *  sibling; otherwise the focused leaf is wrapped in a new split. */
function insertSplit(state: PaneTreeState, dir: SplitDir, kind: PaneKind): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const focused = t.focus;
    const newLeaf = makeLeaf(kind);
    const parent = findParentOf(t.layout, focused);
    // Case 1: focused leaf is the root (no parent). Wrap in a split.
    if (!parent) {
      if (t.layout.kind !== "leaf" || t.layout.pane.id !== focused) return t;
      const split = makeSplit(dir, [t.layout, newLeaf]);
      return { ...t, layout: split, focus: newLeaf.pane.id };
    }
    const parentSplit = parent.parent;
    // Case 2: parent split already aligned with `dir`. Insert sibling after.
    if (parentSplit.dir === dir) {
      const layout = updateNode(t.layout, (n) => n.kind === "split" && n.id === parentSplit.id, (n) => {
        if (n.kind !== "split") return n;
        const sizes = n.sizes.slice();
        const half = sizes[parent.index] / 2;
        sizes[parent.index] = half;
        sizes.splice(parent.index + 1, 0, half);
        const children = [
          ...n.children.slice(0, parent.index + 1),
          newLeaf,
          ...n.children.slice(parent.index + 1),
        ];
        return { ...n, children, sizes };
      });
      return { ...t, layout, focus: newLeaf.pane.id };
    }
    // Case 3: parent dir doesn't match. Wrap the focused leaf in a new
    // split that does, replacing it inside the parent.
    const layout = updateNode(t.layout, (n) => n.kind === "split" && n.id === parentSplit.id, (n) => {
      if (n.kind !== "split") return n;
      const child = n.children[parent.index];
      const newSplit = makeSplit(dir, [child, newLeaf]);
      const children = n.children.slice();
      children[parent.index] = newSplit;
      return { ...n, children };
    });
    return { ...t, layout, focus: newLeaf.pane.id };
  });
}

export function vsplit(state: PaneTreeState, kind: PaneKind = "editor"): PaneTreeState {
  return insertSplit(state, "vertical", kind);
}

export function hsplit(state: PaneTreeState, kind: PaneKind = "editor"): PaneTreeState {
  return insertSplit(state, "horizontal", kind);
}

/** Closes the focused pane. The very last pane in the only tab cannot
 *  close — refusing leaves the user with something to focus. */
export function closePane(state: PaneTreeState): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    if (t.layout.kind === "leaf") return t;
    const parent = findParentOf(t.layout, t.focus);
    if (!parent) return t;
    const parentSplit = parent.parent;
    // Pick a new focus pane: prefer the leaf at the index-1 position in
    // the parent split, else index+1. Use the first leaf of whichever
    // child we land on.
    const siblingIdx = parent.index > 0 ? parent.index - 1 : parent.index + 1;
    const sibling = parentSplit.children[siblingIdx];
    const nextFocus = findFirstLeaf(sibling).pane.id;
    // Remove the focused leaf + its weight; roll the weight into a
    // remaining neighbor so the split's total weight is preserved.
    const layout = updateNode(t.layout, (n) => n.kind === "split" && n.id === parentSplit.id, (n) => {
      if (n.kind !== "split") return n;
      const closedW = n.sizes[parent.index];
      const into = Math.max(0, parent.index - 1);
      const children = n.children.filter((_, i) => i !== parent.index);
      const sizes = n.sizes.filter((_, i) => i !== parent.index);
      if (sizes.length > 0) {
        const intoFinal = into >= sizes.length ? sizes.length - 1 : into;
        sizes[intoFinal] += closedW;
      }
      return { ...n, children, sizes };
    });
    const collapsed = collapse(layout);
    return { ...t, layout: collapsed, focus: nextFocus };
  });
}

/** Detach the focused leaf and reinsert at the dir-most edge of the
 *  tab so it becomes a top-level row/column on that side. */
export function movePane(
  state: PaneTreeState,
  dir: "left" | "right" | "up" | "down",
): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    if (t.layout.kind === "leaf") return t;
    const focusedLeaf = findLeafByPaneId(t.layout, t.focus);
    if (!focusedLeaf) return t;
    const parent = findParentOf(t.layout, t.focus);
    if (!parent) return t;
    const parentSplit = parent.parent;
    // Detach from current parent (mirror of closePane's weight-merge).
    const without = updateNode(t.layout, (n) => n.kind === "split" && n.id === parentSplit.id, (n) => {
      if (n.kind !== "split") return n;
      const closedW = n.sizes[parent.index];
      const into = Math.max(0, parent.index - 1);
      const children = n.children.filter((_, i) => i !== parent.index);
      const sizes = n.sizes.filter((_, i) => i !== parent.index);
      if (sizes.length > 0) {
        const intoFinal = into >= sizes.length ? sizes.length - 1 : into;
        sizes[intoFinal] += closedW;
      }
      return { ...n, children, sizes };
    });
    const collapsed = collapse(without);
    const axis: SplitDir = dir === "left" || dir === "right" ? "vertical" : "horizontal";
    const appendAtEnd = dir === "right" || dir === "down";
    let nextLayout: LayoutNode;
    if (collapsed.kind === "split" && collapsed.dir === axis) {
      const newWeight = 1 / (collapsed.children.length + 1);
      const children = appendAtEnd
        ? [...collapsed.children, focusedLeaf]
        : [focusedLeaf, ...collapsed.children];
      const sizes = appendAtEnd
        ? [...collapsed.sizes, newWeight]
        : [newWeight, ...collapsed.sizes];
      nextLayout = { ...collapsed, children, sizes };
    } else {
      const children = appendAtEnd ? [collapsed, focusedLeaf] : [focusedLeaf, collapsed];
      nextLayout = makeSplit(axis, children);
    }
    return { ...t, layout: nextLayout, focus: focusedLeaf.pane.id };
  });
}

// ── tile + stack mutations (editor panes) ──────────────────────────────────

function withFocusedLeaf(t: Tab, fn: (pane: Pane) => Pane): Tab {
  const leaf = findLeafByPaneId(t.layout, t.focus);
  if (!leaf) return t;
  const next = fn(leaf.pane);
  if (next === leaf.pane) return t;
  return {
    ...t,
    layout: updateNode(
      t.layout,
      (n) => n.kind === "leaf" && n.pane.id === t.focus,
      () => ({ kind: "leaf", pane: next }),
    ),
  };
}

export function jumpToTile(state: PaneTreeState, tileId: string): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) =>
    withFocusedLeaf(t, (p) => {
      if (p.kind !== "editor") {
        return { id: p.id, kind: "editor", tiles: [tileId], activeIdx: 0 };
      }
      if (p.tiles.length === 0) return { ...p, tiles: [tileId], activeIdx: 0 };
      return {
        ...p,
        tiles: p.tiles.map((tid, k) => (k === p.activeIdx ? tileId : tid)),
      };
    }),
  );
}

export function stackAdd(state: PaneTreeState, tileId: string): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) =>
    withFocusedLeaf(t, (p) => {
      if (p.kind !== "editor") return p;
      const existing = p.tiles.indexOf(tileId);
      if (existing >= 0) return { ...p, activeIdx: existing };
      return { ...p, tiles: [...p.tiles, tileId], activeIdx: p.tiles.length };
    }),
  );
}

export function stackNext(state: PaneTreeState, dir: 1 | -1): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) =>
    withFocusedLeaf(t, (p) => {
      if (p.kind !== "editor") return p;
      const n = p.tiles.length;
      if (n <= 1) return p;
      const ni = ((p.activeIdx + dir) % n + n) % n;
      return { ...p, activeIdx: ni };
    }),
  );
}

export function stackClose(state: PaneTreeState, idx: number): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) =>
    withFocusedLeaf(t, (p) => {
      if (p.kind !== "editor") return p;
      if (idx < 0 || idx >= p.tiles.length) return p;
      const tiles = p.tiles.filter((_, i) => i !== idx);
      const activeIdx =
        tiles.length === 0 ? 0 : idx <= p.activeIdx ? Math.max(0, p.activeIdx - 1) : p.activeIdx;
      return { ...p, tiles, activeIdx };
    }),
  );
}

// ── pane-kind swap ─────────────────────────────────────────────────────────

export function swapKind(state: PaneTreeState, paneId: string, newKind: PaneKind): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const leaf = findLeafByPaneId(t.layout, paneId);
    if (!leaf || leaf.pane.kind === newKind) return t;
    const replacement: Pane = { ...makePane(newKind), id: leaf.pane.id };
    return {
      ...t,
      layout: updateNode(
        t.layout,
        (n) => n.kind === "leaf" && n.pane.id === paneId,
        () => ({ kind: "leaf", pane: replacement }),
      ),
    };
  });
}

export function setPaneWidget(state: PaneTreeState, paneId: string, widgetId: string): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const leaf = findLeafByPaneId(t.layout, paneId);
    if (!leaf || leaf.pane.kind !== "widget" || leaf.pane.widget === widgetId) return t;
    const replacement: WidgetPane = { ...leaf.pane, widget: widgetId };
    return {
      ...t,
      layout: updateNode(
        t.layout,
        (n) => n.kind === "leaf" && n.pane.id === paneId,
        () => ({ kind: "leaf", pane: replacement }),
      ),
    };
  });
}

// ── size mutations (drag handlers) ─────────────────────────────────────────

/** Replace the sizes array on a specific split node. Used by the drag
 *  handler each pointermove. Validates length matches children — returns
 *  the same state reference when the update is a no-op (so reactive
 *  consumers skip a re-render). */
export function setSplitSizes(state: PaneTreeState, splitId: string, sizes: number[]): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const layout = updateNode(
      t.layout,
      (n) => n.kind === "split" && n.id === splitId,
      (n) => {
        if (n.kind !== "split") return n;
        if (sizes.length !== n.children.length) return n;
        return { ...n, sizes: sizes.slice() };
      },
    );
    return layout === t.layout ? t : { ...t, layout };
  });
}

// ── tab mutations ──────────────────────────────────────────────────────────

export function newTab(state: PaneTreeState, name: string = "new"): PaneTreeState {
  const t = makeTab(name);
  return { ...state, tabs: [...state.tabs, t], activeTabId: t.id };
}

export function closeTab(state: PaneTreeState, tabId: string): PaneTreeState {
  if (state.tabs.length <= 1) return state;
  const remaining = state.tabs.filter((t) => t.id !== tabId);
  const activeTabId = tabId === state.activeTabId ? remaining[0].id : state.activeTabId;
  return { ...state, tabs: remaining, activeTabId };
}

export function switchTab(state: PaneTreeState, tabId: string): PaneTreeState {
  if (!state.tabs.some((t) => t.id === tabId)) return state;
  if (tabId === state.activeTabId) return state;
  return { ...state, activeTabId: tabId };
}

export function switchTabByIndex(state: PaneTreeState, index: number): PaneTreeState {
  const tab = state.tabs[index];
  if (!tab) return state;
  return switchTab(state, tab.id);
}

export function renameTab(state: PaneTreeState, tabId: string, name: string): PaneTreeState {
  return {
    ...state,
    tabs: state.tabs.map((t) => (t.id === tabId ? { ...t, name } : t)),
  };
}

export function moveTab(state: PaneTreeState, from: number, to: number): PaneTreeState {
  if (from === to) return state;
  if (from < 0 || from >= state.tabs.length) return state;
  if (to < 0 || to >= state.tabs.length) return state;
  const tabs = state.tabs.slice();
  const [moved] = tabs.splice(from, 1);
  tabs.splice(to, 0, moved);
  return { ...state, tabs };
}

// ── serialization ──────────────────────────────────────────────────────────

export function serialize(state: PaneTreeState): string {
  return JSON.stringify(state);
}

/**
 * Returns null on any decode failure. Migrates legacy v1 state (the
 * `Pane[][]` matrix with `rowSizes` / `colSizes`) into the v2 binary
 * tree on the fly so existing localStorage envelopes keep working.
 */
export function deserialize(raw: string | null | undefined): PaneTreeState | null {
  if (!raw) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object") return null;
  const obj = parsed as Record<string, unknown>;
  if (!Array.isArray(obj.tabs) || obj.tabs.length === 0) return null;
  if (typeof obj.activeTabId !== "string") return null;
  if (!obj.tabs.some((t: any) => t && t.id === obj.activeTabId)) return null;

  if (obj.version === STATE_VERSION) {
    const tabs = (obj.tabs as any[]).map((t) => normalizeV2Tab(t));
    if (tabs.some((t) => t === null)) return null;
    return { ...(obj as PaneTreeState), tabs: tabs as Tab[] };
  }
  if (obj.version === 1) {
    return migrateV1((obj as unknown) as LegacyV1State);
  }
  return null;
}

function normalizeV2Tab(t: any): Tab | null {
  if (!t || typeof t.id !== "string" || typeof t.name !== "string") return null;
  if (!t.layout || typeof t.focus !== "string") return null;
  if (!findLeafByPaneId(t.layout as LayoutNode, t.focus)) {
    const first = findFirstLeaf(t.layout as LayoutNode);
    return { ...(t as Tab), focus: first.pane.id };
  }
  return t as Tab;
}

type LegacyV1State = {
  version: number;
  activeTabId: string;
  tabs: {
    id: string;
    name: string;
    layout: Pane[][];
    focus: [number, number];
    rowSizes?: number[];
    colSizes?: number[][];
  }[];
};

function migrateV1(s: LegacyV1State): PaneTreeState | null {
  try {
    const tabs: Tab[] = s.tabs.map((t) => {
      const rows = t.layout;
      const rowSizes =
        Array.isArray(t.rowSizes) && t.rowSizes.length === rows.length
          ? t.rowSizes
          : rows.map(() => 1);
      const colSizes =
        Array.isArray(t.colSizes) && t.colSizes.length === rows.length
          ? t.colSizes
          : rows.map((r) => r.map(() => 1));
      const rowNodes: LayoutNode[] = rows.map((row, r) => {
        const ls: LeafNode[] = row.map((pane) => ({ kind: "leaf", pane }));
        if (ls.length === 1) return ls[0];
        return {
          id: mkSplitId(),
          kind: "split",
          dir: "vertical",
          children: ls,
          sizes: colSizes[r].slice(),
        };
      });
      const layout: LayoutNode =
        rowNodes.length === 1
          ? rowNodes[0]
          : {
              id: mkSplitId(),
              kind: "split",
              dir: "horizontal",
              children: rowNodes,
              sizes: rowSizes.slice(),
            };
      const [fr, fc] = t.focus;
      const focusPaneObj = rows[fr]?.[fc] ?? rows[0]?.[0] ?? findFirstLeaf(layout).pane;
      return {
        id: t.id,
        name: t.name,
        layout,
        focus: focusPaneObj.id,
      };
    });
    return {
      version: STATE_VERSION,
      tabs,
      activeTabId: s.activeTabId,
    };
  } catch {
    return null;
  }
}
