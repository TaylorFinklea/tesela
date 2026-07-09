/**
 * Prism v5 — workspace state migration.
 *
 * Reads the v4 `tesela:prism4:v1` envelope (schema `_v: 2` after the v4
 * binary-tree refactor; or `_v: 1` n-ary matrix for very old state) and
 * produces a v5 Workspace (`_v: 3`).
 *
 * Mapping rules per the spec:
 *   editor    → page buffer (first tile in the v4 stack becomes the pageId;
 *               additional tiles are dropped — v5 has no stacks)
 *   widget    → ambient buffer (name table maps v4 widget id → ambient name)
 *   context   → derived buffer with follow binding (mode tab → renderer name)
 *   graph     → drop (logged in migration report)
 *   dashboard → ambient "workspace-dashboard"
 *
 * v4's n-ary splits are converted to right-leaning binary trees: a split with
 * children [a, b, c, d] and sizes [s0, s1, s2, s3] becomes:
 *   split(a, split(b, split(c, d)))
 * with ratios chosen so each leaf retains its proportion of the original
 * row/column.
 *
 * The migration is **idempotent**: re-running on an already-v3 envelope is
 * a no-op (returns the input unchanged).
 */

import {
  type Buffer,
  type LeafId,
  type Node,
  type PageId,
  type Tab,
  type TabId,
  type Workspace,
  asLeafId,
  asPageId,
  asSplitId,
  asTabId,
  newLeafId,
  newSplitId,
} from "./types.ts";
import { defaultWorkspace, makePageBuffer } from "./tree.ts";

export const V5_KEY = "tesela:prism:state";
export const V4_KEY = "tesela:prism4:v1";

/** A v4 pane (the 5-kind discriminated union). */
type V4Pane =
  | { id: string; kind: "editor"; tiles: string[]; activeIdx: number }
  | { id: string; kind: "widget"; widget: string }
  | { id: string; kind: "context"; tile: string | null }
  | { id: string; kind: "graph" }
  | { id: string; kind: "dashboard" };

type V4LeafNode = { kind: "leaf"; pane: V4Pane };
type V4SplitNode = {
  id: string;
  kind: "split";
  dir: "vertical" | "horizontal";
  children: V4LayoutNode[];
  sizes: number[];
};
type V4LayoutNode = V4LeafNode | V4SplitNode;

type V4Tab = {
  id: string;
  name: string;
  layout: V4LayoutNode;
  focus: string; // pane id (v4)
};

type V4State = {
  version: number;
  tabs: V4Tab[];
  activeTabId: string;
};

/** Widget id → ambient name table. Best-effort. */
const WIDGET_TO_AMBIENT: Record<string, string> = {
  calendar: "calendar",
  tasks: "today-in-progress",
  views: "views",
  inbox: "views",
  pinned: "workspace-dashboard",
  recent: "workspace-dashboard",
  pages: "workspace-dashboard",
  projects: "workspace-dashboard",
  people: "workspace-dashboard",
  dailies: "workspace-dashboard", // dailies is a system widget, not really ambient — but it stays in the dashboard.
};

export type MigrationReport = {
  droppedGraph: number;
  droppedExtraEditorTiles: number;
  convertedContextDerived: number;
  convertedWidgetAmbient: number;
  convertedDashboardAmbient: number;
  unmappedWidgets: string[];
};

const EMPTY_REPORT: MigrationReport = {
  droppedGraph: 0,
  droppedExtraEditorTiles: 0,
  convertedContextDerived: 0,
  convertedWidgetAmbient: 0,
  convertedDashboardAmbient: 0,
  unmappedWidgets: [],
};

function paneToBuffer(
  pane: V4Pane,
  report: MigrationReport,
): Buffer | null {
  switch (pane.kind) {
    case "editor": {
      if (pane.tiles.length === 0) {
        // Empty editor pane — seed with a placeholder; the runtime will treat
        // this as "needs a page" and either show empty-state or seed daily.
        return null;
      }
      const active = pane.tiles[pane.activeIdx] ?? pane.tiles[0];
      if (pane.tiles.length > 1) {
        report.droppedExtraEditorTiles += pane.tiles.length - 1;
      }
      return makePageBuffer(asPageId(active));
    }
    case "widget": {
      const ambient = WIDGET_TO_AMBIENT[pane.widget];
      if (!ambient) {
        report.unmappedWidgets.push(pane.widget);
        // Fall through to workspace-dashboard for unknown widgets so users
        // don't lose the pane wholesale.
        report.convertedWidgetAmbient += 1;
        return { kind: "ambient", ambientName: "workspace-dashboard" };
      }
      report.convertedWidgetAmbient += 1;
      return { kind: "ambient", ambientName: ambient };
    }
    case "context": {
      report.convertedContextDerived += 1;
      // v4 context panes were sub-tabbed (backlinks/outline/properties/etc.)
      // without persistence of which tab was active. Default to backlinks.
      return {
        kind: "derived",
        rendererName: "backlinks-of-page",
        binding: { mode: "follow" },
      };
    }
    case "graph": {
      report.droppedGraph += 1;
      return null;
    }
    case "dashboard": {
      report.convertedDashboardAmbient += 1;
      return { kind: "ambient", ambientName: "workspace-dashboard" };
    }
  }
}

/**
 * Right-lean: turn an n-ary v4 split with `k` children into k-1 nested binary
 * splits. Ratio at each level preserves the leftmost child's share relative
 * to the remaining tail.
 */
function fold(
  buffers: (Node | null)[],
  sizes: number[],
  dir: "v" | "h",
): Node | null {
  // Drop nulls (dropped panes). Re-normalize.
  const pairs = buffers
    .map((b, i) => ({ b, s: sizes[i] ?? 1 }))
    .filter((p) => p.b !== null) as { b: Node; s: number }[];
  if (pairs.length === 0) return null;
  if (pairs.length === 1) return pairs[0].b;
  // Right-lean: split(head, rest)
  const head = pairs[0];
  const tail = pairs.slice(1);
  const tailTotal = tail.reduce((sum, p) => sum + p.s, 0);
  const total = head.s + tailTotal;
  const ratio = total > 0 ? head.s / total : 0.5;
  const tailNode = fold(
    tail.map((p) => p.b),
    tail.map((p) => p.s),
    dir,
  )!;
  return {
    type: "split",
    id: newSplitId(),
    dir,
    ratio,
    children: [head.b, tailNode],
  };
}

function v4NodeToV5(
  v4: V4LayoutNode,
  v4FocusPaneId: string,
  newFocusLeafId: { current: LeafId | undefined; pageId: PageId | undefined },
  report: MigrationReport,
): Node | null {
  if (v4.kind === "leaf") {
    const buffer = paneToBuffer(v4.pane, report);
    if (!buffer) return null;
    const id = newLeafId();
    const leaf: Node = { type: "leaf", id, buffer };
    if (v4.pane.id === v4FocusPaneId) {
      newFocusLeafId.current = id;
      if (buffer.kind === "page") newFocusLeafId.pageId = buffer.pageId;
    }
    return leaf;
  }
  const childNodes = v4.children.map((c) =>
    v4NodeToV5(c, v4FocusPaneId, newFocusLeafId, report),
  );
  const dir: "v" | "h" = v4.dir === "vertical" ? "v" : "h";
  return fold(childNodes, v4.sizes, dir);
}

/** Default daily-note page buffer, used when v4 state had no surviving leaves. */
function fallbackTab(): Tab {
  const id = newLeafId();
  return {
    id: asTabId("t" + Math.random().toString(36).slice(2, 9)),
    name: "untitled",
    layout: {
      type: "leaf",
      id,
      // pageId is empty here; the +page.svelte seed flow fills it on mount.
      buffer: makePageBuffer(asPageId("")),
    },
    lastFocusedLeafId: id,
  };
}

function v4TabToV5(v4: V4Tab, report: MigrationReport): Tab {
  const newFocus = {
    current: undefined as LeafId | undefined,
    pageId: undefined as PageId | undefined,
  };
  let layout = v4NodeToV5(v4.layout, v4.focus, newFocus, report);
  if (!layout) {
    // Whole tab was made of graph/empty-editor panes that got dropped.
    return fallbackTab();
  }
  if (!newFocus.current) {
    // Original focus pane was dropped. Focus the first surviving leaf.
    const firstLeaf = collectFirstLeaf(layout);
    newFocus.current = firstLeaf?.id;
    if (firstLeaf?.buffer.kind === "page") {
      newFocus.pageId = firstLeaf.buffer.pageId;
    }
  }
  return {
    id: asTabId(v4.id),
    name: v4.name,
    layout,
    lastFocusedLeafId: newFocus.current,
    lastFocusedPageId: newFocus.pageId,
  };
}

function collectFirstLeaf(n: Node): { id: LeafId; buffer: Buffer } | null {
  if (n.type === "leaf") return { id: n.id, buffer: n.buffer };
  return collectFirstLeaf(n.children[0]) ?? collectFirstLeaf(n.children[1]);
}

/**
 * Convert a v4 state envelope (schema 2) into a v5 Workspace (schema 3).
 *
 * If the input is already v3, return unchanged.
 */
export function migrate(
  rawState: unknown,
): { workspace: Workspace; report: MigrationReport } {
  const report: MigrationReport = { ...EMPTY_REPORT, unmappedWidgets: [] };
  if (!rawState || typeof rawState !== "object") {
    return { workspace: defaultWorkspace(makePageBuffer(asPageId(""))), report };
  }
  const obj = rawState as Record<string, unknown>;
  // Already v3?
  if (obj._v === 3) {
    return { workspace: obj as Workspace, report };
  }
  // v4 binary tree (schema 2) or v4 matrix (schema 1).
  // We only handle the binary-tree shape here; matrix → tree was already
  // converted by v4's own migration before being persisted.
  if (typeof obj.version !== "number") {
    return {
      workspace: defaultWorkspace(makePageBuffer(asPageId(""))),
      report,
    };
  }
  const v4 = obj as unknown as V4State;
  const tabs = v4.tabs.map((t) => v4TabToV5(t, report));
  const activeTabId = tabs.some((t) => t.id === (v4.activeTabId as TabId))
    ? (v4.activeTabId as TabId)
    : tabs[0].id;
  return {
    workspace: {
      _v: 3,
      tabs,
      activeTabId,
      sidebar: { collapsed: false, activeSurface: "tree" },
      peekFirstRendererByPageType: {},
    },
    report,
  };
}

/**
 * Load workspace state from localStorage. Reads v5 key first; on miss,
 * reads v4 key and migrates. On any decode failure, returns a fresh
 * workspace with an empty-page-buffer tab.
 *
 * Idempotent + safe to call any time (returns `null` if `localStorage`
 * isn't available, so callers can fall back to `defaultWorkspace`).
 */
export function loadFromLocalStorage(): {
  workspace: Workspace;
  report: MigrationReport;
  migratedFrom: "v3" | "v2" | "none";
} | null {
  if (typeof localStorage === "undefined") return null;
  const v5raw = localStorage.getItem(V5_KEY);
  if (v5raw) {
    try {
      const parsed = JSON.parse(v5raw);
      const { workspace, report } = migrate(parsed);
      return { workspace, report, migratedFrom: "v3" };
    } catch {
      // fall through; try v4 key, else default
    }
  }
  const v4raw = localStorage.getItem(V4_KEY);
  if (v4raw) {
    try {
      const parsed = JSON.parse(v4raw);
      const { workspace, report } = migrate(parsed);
      // One-shot rename: write the v5 envelope and delete the v4 key.
      localStorage.setItem(V5_KEY, JSON.stringify(workspace));
      localStorage.removeItem(V4_KEY);
      return { workspace, report, migratedFrom: "v2" };
    } catch {
      // fall through
    }
  }
  return {
    workspace: defaultWorkspace(makePageBuffer(asPageId(""))),
    report: { ...EMPTY_REPORT, unmappedWidgets: [] },
    migratedFrom: "none",
  };
}

export function saveToLocalStorage(ws: Workspace): void {
  if (typeof localStorage === "undefined") return;
  try {
    localStorage.setItem(V5_KEY, JSON.stringify(ws));
  } catch (e) {
    console.warn("workspace persist failed", e);
  }
}

// Re-export to keep the call sites tidy.
export { asLeafId, asSplitId };
