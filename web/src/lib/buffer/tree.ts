/**
 * Prism v5 — pure binary pane-tree algebra over Buffer leaves.
 *
 * Every internal split is binary: two children, one `ratio` (share of the
 * first child). vsplit / hsplit wrap the focused leaf in a new split rather
 * than extending an existing parent — depth grows, no n-ary special cases.
 *
 * The follow-binding source of truth (`lastFocusedPageId`) is mutated only
 * by `focusPane()` and only when the newly focused leaf carries a page
 * buffer. See `state.svelte.ts` for the chokepoint wiring.
 *
 * All operations are pure: input state → output state, structural sharing
 * preserved for unchanged subtrees so reactive consumers can skip re-renders.
 */

import {
  asTabId,
  newLeafId,
  newSplitId,
  newTabId,
  type Buffer,
  type DerivedBinding,
  type Leaf,
  type LeafId,
  type Node,
  type PageId,
  type Reference,
  type SidebarState,
  type Split,
  type SplitId,
  type Tab,
  type TabId,
  type Workspace,
} from "./types.ts";

// ── factories ──────────────────────────────────────────────────────────────

export function makePageBuffer(pageId: PageId): Buffer {
  return { kind: "page", pageId };
}

export function makeDerivedBuffer(
  rendererName: string,
  binding: DerivedBinding,
): Buffer {
  return { kind: "derived", rendererName, binding };
}

export function makeAmbientBuffer(ambientName: string): Buffer {
  return { kind: "ambient", ambientName };
}

export function makeLeaf(buffer: Buffer): Leaf {
  return { type: "leaf", id: newLeafId(), buffer };
}

export function makeSplit(
  dir: "v" | "h",
  ratio: number,
  children: [Node, Node],
): Split {
  return { type: "split", id: newSplitId(), dir, ratio, children };
}

export function makeTab(name: string, rootBuffer: Buffer): Tab {
  const leaf = makeLeaf(rootBuffer);
  return {
    id: newTabId(),
    name,
    layout: leaf,
    lastFocusedLeafId: leaf.id,
    lastFocusedPageId: rootBuffer.kind === "page" ? rootBuffer.pageId : undefined,
  };
}

export const DEFAULT_SIDEBAR: SidebarState = {
  collapsed: false,
  activeSurface: "tree",
};

export function defaultWorkspace(rootBuffer: Buffer): Workspace {
  const t = makeTab("untitled", rootBuffer);
  return {
    _v: 3,
    tabs: [t],
    activeTabId: t.id,
    sidebar: { ...DEFAULT_SIDEBAR },
    peekFirstRendererByPageType: {},
  };
}

// ── traversal ──────────────────────────────────────────────────────────────

export function* leaves(node: Node): Generator<Leaf> {
  if (node.type === "leaf") {
    yield node;
    return;
  }
  yield* leaves(node.children[0]);
  yield* leaves(node.children[1]);
}

export function findLeaf(root: Node, leafId: LeafId): Leaf | undefined {
  for (const l of leaves(root)) if (l.id === leafId) return l;
  return undefined;
}

/** Path of `0|1` child-indices from `root` down to the leaf identified by id. */
export function findPath(
  root: Node,
  leafId: LeafId,
): { node: Leaf; path: number[] } | undefined {
  if (root.type === "leaf") {
    return root.id === leafId ? { node: root, path: [] } : undefined;
  }
  for (let i = 0; i < 2; i++) {
    const hit = findPath(root.children[i], leafId);
    if (hit) return { node: hit.node, path: [i, ...hit.path] };
  }
  return undefined;
}

/** Parent split + index (0|1) of the leaf inside its parent. Undefined if leaf is root. */
export function findParent(
  root: Node,
  leafId: LeafId,
): { parent: Split; index: 0 | 1 } | undefined {
  if (root.type === "leaf") return undefined;
  for (let i = 0; i < 2; i++) {
    const c = root.children[i];
    if (c.type === "leaf" && c.id === leafId) {
      return { parent: root, index: i as 0 | 1 };
    }
    if (c.type === "split") {
      const hit = findParent(c, leafId);
      if (hit) return hit;
    }
  }
  return undefined;
}

function firstLeaf(n: Node): Leaf {
  return n.type === "leaf" ? n : firstLeaf(n.children[0]);
}

// ── internal helpers ───────────────────────────────────────────────────────

/** Immutably swap a single node identified by `pred` for `fn(node)`. */
function updateNode(
  root: Node,
  pred: (n: Node) => boolean,
  fn: (n: Node) => Node,
): Node {
  if (pred(root)) return fn(root);
  if (root.type === "leaf") return root;
  const c0 = updateNode(root.children[0], pred, fn);
  const c1 = updateNode(root.children[1], pred, fn);
  if (c0 === root.children[0] && c1 === root.children[1]) return root;
  return { ...root, children: [c0, c1] };
}

/** Collapse splits whose one child has been removed by a sibling-removal op. */
function collapse(root: Node): Node {
  if (root.type === "leaf") return root;
  const c0 = collapse(root.children[0]);
  const c1 = collapse(root.children[1]);
  if (c0 === root.children[0] && c1 === root.children[1]) return root;
  return { ...root, children: [c0, c1] };
}

/** Spatial neighbor of `leafId` in the given direction, or undefined. */
function neighborLeaf(
  root: Node,
  leafId: LeafId,
  dir: "left" | "right" | "up" | "down",
): Leaf | undefined {
  const hit = findPath(root, leafId);
  if (!hit) return undefined;
  const axis: "v" | "h" = dir === "left" || dir === "right" ? "v" : "h";
  const stride = dir === "left" || dir === "up" ? -1 : 1;
  let node: Node = root;
  const ancestors: Split[] = [];
  for (const idx of hit.path) {
    if (node.type !== "split") break;
    ancestors.push(node);
    node = node.children[idx];
  }
  for (let i = ancestors.length - 1; i >= 0; i--) {
    const a = ancestors[i];
    if (a.dir !== axis) continue;
    const idxInA = hit.path[i];
    const target = idxInA + stride;
    if (target < 0 || target > 1) continue;
    return descendToLeaf(a.children[target], dir);
  }
  return undefined;
}

/** Reach into a subtree, preferring the edge child closest to motion's origin. */
function descendToLeaf(
  node: Node,
  fromDir: "left" | "right" | "up" | "down",
): Leaf {
  if (node.type === "leaf") return node;
  const axis: "v" | "h" =
    fromDir === "left" || fromDir === "right" ? "v" : "h";
  if (node.dir === axis) {
    const idx = fromDir === "left" || fromDir === "up" ? 1 : 0;
    return descendToLeaf(node.children[idx], fromDir);
  }
  return descendToLeaf(node.children[0], fromDir);
}

// ── split / close / move ───────────────────────────────────────────────────

/**
 * Split the focused leaf into a new binary split. The focused leaf becomes
 * child[0], the new leaf becomes child[1]. Returns updated layout + new
 * leaf id (caller decides whether to focus it).
 */
export function splitFocused(
  layout: Node,
  focusedLeafId: LeafId,
  dir: "v" | "h",
  newBuffer: Buffer,
  ratio = 0.5,
): { layout: Node; newLeafId: LeafId } {
  const focused = findLeaf(layout, focusedLeafId);
  if (!focused) return { layout, newLeafId: focusedLeafId };
  const newLeaf = makeLeaf(newBuffer);
  const next = updateNode(
    layout,
    (n) => n.type === "leaf" && n.id === focusedLeafId,
    () => makeSplit(dir, ratio, [focused, newLeaf]),
  );
  return { layout: next, newLeafId: newLeaf.id };
}

/**
 * Close the focused leaf. The other child of its parent split is promoted
 * into the parent's slot. If the focused leaf is the root, refuses (caller
 * must guard).
 */
export function closeFocused(
  layout: Node,
  focusedLeafId: LeafId,
): { layout: Node; nextFocusId: LeafId } | undefined {
  if (layout.type === "leaf") return undefined; // root leaf, can't close
  const parent = findParent(layout, focusedLeafId);
  if (!parent) return undefined;
  const survivorIdx = parent.index === 0 ? 1 : 0;
  const survivor = parent.parent.children[survivorIdx];
  const nextFocus = firstLeaf(survivor);
  const next = updateNode(
    layout,
    (n) => n.type === "split" && n.id === parent.parent.id,
    () => survivor,
  );
  return { layout: collapse(next), nextFocusId: nextFocus.id };
}

/**
 * Aerospace-style move: detach focused leaf, reinsert at the dir-most edge
 * of the tab so it becomes a top-level row/column on that side.
 */
export function movePaneToEdge(
  layout: Node,
  focusedLeafId: LeafId,
  dir: "left" | "right" | "up" | "down",
): Node {
  if (layout.type === "leaf") return layout;
  const focused = findLeaf(layout, focusedLeafId);
  if (!focused) return layout;
  const parent = findParent(layout, focusedLeafId);
  if (!parent) return layout;
  const survivorIdx = parent.index === 0 ? 1 : 0;
  const survivor = parent.parent.children[survivorIdx];
  const without = updateNode(
    layout,
    (n) => n.type === "split" && n.id === parent.parent.id,
    () => survivor,
  );
  const collapsed = collapse(without);
  const axis: "v" | "h" = dir === "left" || dir === "right" ? "v" : "h";
  const appendAtEnd = dir === "right" || dir === "down";
  const children: [Node, Node] = appendAtEnd
    ? [collapsed, focused]
    : [focused, collapsed];
  const ratio = appendAtEnd ? 0.7 : 0.3;
  return makeSplit(axis, ratio, children);
}

export function setSplitRatio(
  layout: Node,
  splitId: SplitId,
  ratio: number,
): Node {
  return updateNode(
    layout,
    (n) => n.type === "split" && n.id === splitId,
    (n) => (n.type === "split" ? { ...n, ratio } : n),
  );
}

// ── focus motion ───────────────────────────────────────────────────────────

export function nextFocusedLeaf(
  layout: Node,
  focusedLeafId: LeafId,
  dir: "left" | "right" | "up" | "down",
): LeafId | undefined {
  const n = neighborLeaf(layout, focusedLeafId, dir);
  return n?.id;
}

// ── buffer mutation in the focused leaf ────────────────────────────────────

/** Replace the buffer at a specific leaf. Returns updated layout. */
export function replaceLeafBuffer(
  layout: Node,
  leafId: LeafId,
  buffer: Buffer,
): Node {
  return updateNode(
    layout,
    (n) => n.type === "leaf" && n.id === leafId,
    (n) => (n.type === "leaf" ? { ...n, buffer } : n),
  );
}

// ── tab operations ─────────────────────────────────────────────────────────

export function findTab(ws: Workspace, tabId: TabId): Tab | undefined {
  return ws.tabs.find((t) => t.id === tabId);
}

export function activeTab(ws: Workspace): Tab | undefined {
  return findTab(ws, ws.activeTabId);
}

export function replaceTab(ws: Workspace, next: Tab): Workspace {
  let changed = false;
  const tabs = ws.tabs.map((t) => {
    if (t.id !== next.id) return t;
    if (t === next) return t;
    changed = true;
    return next;
  });
  return changed ? { ...ws, tabs } : ws;
}

export function addTab(ws: Workspace, tab: Tab, focusIt = true): Workspace {
  return {
    ...ws,
    tabs: [...ws.tabs, tab],
    activeTabId: focusIt ? tab.id : ws.activeTabId,
  };
}

export function removeTab(ws: Workspace, tabId: TabId): Workspace {
  if (ws.tabs.length <= 1) return ws; // refuse to close the only tab
  const remaining = ws.tabs.filter((t) => t.id !== tabId);
  const activeTabId =
    tabId === ws.activeTabId ? remaining[0].id : ws.activeTabId;
  return { ...ws, tabs: remaining, activeTabId };
}

export function switchActiveTab(ws: Workspace, tabId: TabId): Workspace {
  if (tabId === ws.activeTabId) return ws;
  if (!ws.tabs.some((t) => t.id === tabId)) return ws;
  return { ...ws, activeTabId: tabId };
}

export function renameTab(
  ws: Workspace,
  tabId: TabId,
  name: string,
): Workspace {
  return {
    ...ws,
    tabs: ws.tabs.map((t) => (t.id === tabId ? { ...t, name } : t)),
  };
}

export function moveTab(ws: Workspace, from: number, to: number): Workspace {
  if (from === to) return ws;
  if (from < 0 || from >= ws.tabs.length) return ws;
  if (to < 0 || to >= ws.tabs.length) return ws;
  const tabs = ws.tabs.slice();
  const [moved] = tabs.splice(from, 1);
  tabs.splice(to, 0, moved);
  return { ...ws, tabs };
}

// ── cross-tab lookups ──────────────────────────────────────────────────────

/** Find the first tab + leaf showing the given pageId, if any. */
export function findPageBufferLocation(
  ws: Workspace,
  pageId: PageId,
): { tabId: TabId; leafId: LeafId } | undefined {
  for (const tab of ws.tabs) {
    for (const l of leaves(tab.layout)) {
      if (l.buffer.kind === "page" && l.buffer.pageId === pageId) {
        return { tabId: tab.id, leafId: l.id };
      }
    }
  }
  return undefined;
}

// Re-export id constructors for callers (so they don't need to import from types.ts).
export { newLeafId, newSplitId, newTabId, asTabId };

/**
 * Build a Reference for the most common case (a page path).
 */
export function pageRef(path: string): Reference {
  return { kind: "page", path };
}
