/**
 * Prism v5 — buffer types and pane-tree shapes.
 *
 * Three buffer kinds in the pane tree's leaf nodes:
 *  - page:    one filesystem-backed page, rendered by a page-type renderer
 *  - derived: pure function of a Reference, host-agnostic renderer
 *  - ambient: workspace-level singleton (calendar, dashboard, ai, …)
 *
 * The discriminated union lives at the leaf's `buffer` field, NOT at the
 * leaf-type level — the pane-tree algebra (split/merge/focus/resize/persist)
 * is identical regardless of kind.
 */

declare const _pageId: unique symbol;
declare const _leafId: unique symbol;
declare const _splitId: unique symbol;
declare const _tabId: unique symbol;

export type PageId = string & { readonly [_pageId]: true };
export type LeafId = string & { readonly [_leafId]: true };
export type SplitId = string & { readonly [_splitId]: true };
export type TabId = string & { readonly [_tabId]: true };

export type Reference =
  | { kind: "page"; path: string }
  | { kind: "tag"; value: string }
  | { kind: "query"; dsl: string };

export type ReferenceKind = Reference["kind"];

export type DerivedBinding =
  | { mode: "follow" }
  | { mode: "pinned"; reference: Reference };

export type Buffer =
  | { kind: "page"; pageId: PageId }
  | { kind: "derived"; rendererName: string; binding: DerivedBinding }
  | { kind: "ambient"; ambientName: string };

export type BufferKind = Buffer["kind"];

export type Leaf = {
  type: "leaf";
  id: LeafId;
  buffer: Buffer;
};

export type Split = {
  type: "split";
  id: SplitId;
  dir: "v" | "h";
  /** Share of the first child. 0..1; second child gets 1 - ratio. */
  ratio: number;
  children: [Node, Node];
};

export type Node = Leaf | Split;

export type Tab = {
  id: TabId;
  name: string;
  layout: Node;
  /** Last focused leaf in this tab (any buffer kind). Restored on tab-switch. */
  lastFocusedLeafId?: LeafId;
  /** Last focused page-buffer pageId. Mutated only by the focusPane chokepoint
   *  when a page buffer gains focus. Drives the derived-buffer Follow binding. */
  lastFocusedPageId?: PageId;
};

export type SidebarSurface = "tree" | "search" | "recent" | "pinned" | "tags";

export type SidebarState = {
  collapsed: boolean;
  activeSurface: SidebarSurface;
};

export type Workspace = {
  /** Schema version. v4 was 2; v5 is 3. */
  _v: 3;
  tabs: Tab[];
  activeTabId: TabId;
  sidebar: SidebarState;
  /** pageType → preferred-first Peek renderer name. Overrides default cycle order. */
  peekFirstRendererByPageType: Record<string, string>;
};

// ── id mints + brand-blind constructors ────────────────────────────────────

function rand(): string {
  return Math.random().toString(36).slice(2, 9);
}

export function newLeafId(): LeafId {
  return ("l" + rand()) as LeafId;
}
export function newSplitId(): SplitId {
  return ("s" + rand()) as SplitId;
}
export function newTabId(): TabId {
  return ("t" + rand()) as TabId;
}

export const asPageId = (s: string): PageId => s as PageId;
export const asLeafId = (s: string): LeafId => s as LeafId;
export const asSplitId = (s: string): SplitId => s as SplitId;
export const asTabId = (s: string): TabId => s as TabId;
