/**
 * Prism v4 pane-tree state. Pure data + pure mutation functions. The
 * reactive Svelte wrapper lives next door in `pane-tree.svelte.ts`;
 * this file stays free of runes so it's testable under plain Node.
 *
 * Model (mirrors `Tesela2.zip → proto-prism4.jsx`):
 *
 *   tab    = { id, name, layout: Pane[][], focus: [rowIdx, colIdx] }
 *   pane   = { id, kind, ...kindFields }
 *
 * Every mutation is a pure (state) -> state function — never in-place.
 * The wrapper uses `$state.raw` and assigns the whole tree on each
 * mutation, which matches React reducer semantics and avoids Svelte 5
 * fine-grained reactivity surprises on deeply nested arrays.
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
  /** Id of the Query-type note that defines this widget (e.g. "tasks",
   *  "recent", "projects", or any user-authored Query note). The widget
   *  pane resolves it through `widgetFromNote`. */
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

export type Tab = {
  id: string;
  name: string;
  layout: Pane[][];
  focus: [number, number];
};

export type PaneTreeState = {
  version: number;
  tabs: Tab[];
  activeTabId: string;
};

export const STATE_VERSION = 1;
export const STORAGE_KEY = "tesela:prism4:v1";

// ── id minting ──────────────────────────────────────────────────────────────

export function mkPaneId(): string {
  return "p" + Math.random().toString(36).slice(2, 9);
}

export function mkTabId(): string {
  return "tab-" + Math.random().toString(36).slice(2, 9);
}

// ── factories ───────────────────────────────────────────────────────────────

/** Default Query-note id a fresh widget pane points at. "recent" is one
 * of the system widgets seeded by `ensureSystemWidgets`. */
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

export function makeTab(name: string = "untitled"): Tab {
  return {
    id: mkTabId(),
    name,
    layout: [[makePane("editor")]],
    focus: [0, 0],
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

// ── lookups ─────────────────────────────────────────────────────────────────

export function focusedTab(state: PaneTreeState): Tab | undefined {
  return state.tabs.find((t) => t.id === state.activeTabId);
}

export function focusedPane(state: PaneTreeState): Pane | undefined {
  const t = focusedTab(state);
  if (!t) return undefined;
  return t.layout[t.focus[0]]?.[t.focus[1]];
}

export function paneById(state: PaneTreeState, paneId: string): { tab: Tab; pane: Pane; row: number; col: number } | undefined {
  for (const tab of state.tabs) {
    for (let r = 0; r < tab.layout.length; r++) {
      const row = tab.layout[r];
      for (let c = 0; c < row.length; c++) {
        if (row[c].id === paneId) return { tab, pane: row[c], row: r, col: c };
      }
    }
  }
  return undefined;
}

/** First tile id of the first editor pane in the active tab. */
export function firstEditorTile(state: PaneTreeState): string | undefined {
  const t = focusedTab(state);
  if (!t) return undefined;
  for (const row of t.layout) {
    for (const p of row) {
      if (p.kind === "editor" && p.tiles.length > 0) return p.tiles[p.activeIdx];
    }
  }
  return undefined;
}

// ── internal helpers ────────────────────────────────────────────────────────

/** Short-circuits to the same state reference when fn returns the same
 * tab (no-op detection). That lets the reactive wrapper skip persistence
 * + re-render on idempotent calls. */
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

function withPaneAt(t: Tab, r: number, c: number, fn: (p: Pane) => Pane): Tab {
  const row = t.layout[r];
  if (!row) return t;
  const oldPane = row[c];
  if (!oldPane) return t;
  const nextPane = fn(oldPane);
  if (nextPane === oldPane) return t;
  const layout = t.layout.map((rr, i) =>
    i === r ? rr.map((p, j) => (j === c ? nextPane : p)) : rr,
  );
  return { ...t, layout };
}

// ── focus + split mutations ─────────────────────────────────────────────────

export function focusPane(state: PaneTreeState, row: number, col: number): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const r = Math.max(0, Math.min(t.layout.length - 1, row));
    const rowArr = t.layout[r];
    if (!rowArr || rowArr.length === 0) return t;
    const c = Math.max(0, Math.min(rowArr.length - 1, col));
    if (r === t.focus[0] && c === t.focus[1]) return t;
    return { ...t, focus: [r, c] };
  });
}

/** Move focus by delta in 2D. Clamps to grid edges. */
export function moveFocus(state: PaneTreeState, dRow: number, dCol: number): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const [r, c] = t.focus;
    const nr = Math.max(0, Math.min(t.layout.length - 1, r + dRow));
    const rowArr = t.layout[nr];
    if (!rowArr || rowArr.length === 0) return t;
    const nc = Math.max(0, Math.min(rowArr.length - 1, c + dCol));
    if (nr === r && nc === c) return t;
    return { ...t, focus: [nr, nc] };
  });
}

export function vsplit(state: PaneTreeState, kind: PaneKind = "editor"): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const [r, c] = t.focus;
    const np = makePane(kind);
    const layout = t.layout.map((row, i) =>
      i === r ? [...row.slice(0, c + 1), np, ...row.slice(c + 1)] : row,
    );
    return { ...t, layout, focus: [r, c + 1] };
  });
}

export function hsplit(state: PaneTreeState, kind: PaneKind = "editor"): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const [r] = t.focus;
    const np = makePane(kind);
    const layout = [...t.layout.slice(0, r + 1), [np], ...t.layout.slice(r + 1)];
    return { ...t, layout, focus: [r + 1, 0] };
  });
}

/** Closes the focused pane. The very last pane in the only tab cannot close. */
export function closePane(state: PaneTreeState): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    if (t.layout.length === 1 && t.layout[0].length === 1) return t;
    const [r, c] = t.focus;
    const newRow = t.layout[r].filter((_, i) => i !== c);
    const layout =
      newRow.length > 0
        ? t.layout.map((rr, i) => (i === r ? newRow : rr))
        : t.layout.filter((_, i) => i !== r);
    const nr = Math.min(r, layout.length - 1);
    const nc = Math.min(c, layout[nr].length - 1);
    return { ...t, layout, focus: [nr, nc] };
  });
}

// ── tile + stack mutations (editor panes) ───────────────────────────────────

/**
 * Replace the focused pane's active tile with `tileId`. If the focused
 * pane is not an editor, convert it to one. If the editor pane has an
 * empty stack, seed it with `tileId`.
 */
export function jumpToTile(state: PaneTreeState, tileId: string): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const [r, c] = t.focus;
    const pane = t.layout[r]?.[c];
    if (!pane) return t;
    return withPaneAt(t, r, c, (p) => {
      if (p.kind !== "editor") {
        return { id: p.id, kind: "editor", tiles: [tileId], activeIdx: 0 };
      }
      if (p.tiles.length === 0) return { ...p, tiles: [tileId], activeIdx: 0 };
      return {
        ...p,
        tiles: p.tiles.map((tid, k) => (k === p.activeIdx ? tileId : tid)),
      };
    });
  });
}

/**
 * Push `tileId` onto the focused editor pane's stack. If already
 * present, focus its index instead of duplicating. No-op if focused
 * pane isn't an editor.
 */
export function stackAdd(state: PaneTreeState, tileId: string): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const [r, c] = t.focus;
    const pane = t.layout[r]?.[c];
    if (!pane || pane.kind !== "editor") return t;
    return withPaneAt(t, r, c, (p) => {
      if (p.kind !== "editor") return p;
      const existing = p.tiles.indexOf(tileId);
      if (existing >= 0) return { ...p, activeIdx: existing };
      return { ...p, tiles: [...p.tiles, tileId], activeIdx: p.tiles.length };
    });
  });
}

/** Cycle the focused editor pane's active tile by `dir` (+1 / -1). */
export function stackNext(state: PaneTreeState, dir: 1 | -1): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const [r, c] = t.focus;
    const pane = t.layout[r]?.[c];
    if (!pane || pane.kind !== "editor") return t;
    const n = pane.tiles.length;
    if (n <= 1) return t;
    return withPaneAt(t, r, c, (p) => {
      if (p.kind !== "editor") return p;
      const ni = ((p.activeIdx + dir) % n + n) % n;
      return { ...p, activeIdx: ni };
    });
  });
}

/** Remove a tile from the focused editor pane's stack by index. */
export function stackClose(state: PaneTreeState, idx: number): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    const [r, c] = t.focus;
    const pane = t.layout[r]?.[c];
    if (!pane || pane.kind !== "editor") return t;
    return withPaneAt(t, r, c, (p) => {
      if (p.kind !== "editor") return p;
      if (idx < 0 || idx >= p.tiles.length) return p;
      const tiles = p.tiles.filter((_, i) => i !== idx);
      // If we just closed the last tile, leave empty (editor with no
      // active tile is a valid state — the next jumpToTile populates it).
      const activeIdx =
        tiles.length === 0
          ? 0
          : idx <= p.activeIdx
            ? Math.max(0, p.activeIdx - 1)
            : p.activeIdx;
      return { ...p, tiles, activeIdx };
    });
  });
}

// ── pane-kind swap ──────────────────────────────────────────────────────────

/** Swap a pane's kind, preserving its id and (where possible) ignoring
 * the prior kind's fields. */
export function swapKind(state: PaneTreeState, paneId: string, newKind: PaneKind): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    let changed = false;
    const layout = t.layout.map((row) =>
      row.map((p) => {
        if (p.id === paneId && p.kind !== newKind) {
          changed = true;
          return { ...makePane(newKind), id: p.id };
        }
        return p;
      }),
    );
    return changed ? { ...t, layout } : t;
  });
}

/** Point a widget pane at a different Query note. No-op if the pane
 * isn't a widget pane or already shows that widget. */
export function setPaneWidget(state: PaneTreeState, paneId: string, widgetId: string): PaneTreeState {
  return replaceTab(state, state.activeTabId, (t) => {
    let changed = false;
    const layout = t.layout.map((row) =>
      row.map((p) => {
        if (p.id === paneId && p.kind === "widget" && p.widget !== widgetId) {
          changed = true;
          return { ...p, widget: widgetId };
        }
        return p;
      }),
    );
    return changed ? { ...t, layout } : t;
  });
}

// ── tab mutations ───────────────────────────────────────────────────────────

export function newTab(state: PaneTreeState, name: string = "new"): PaneTreeState {
  const t = makeTab(name);
  return { ...state, tabs: [...state.tabs, t], activeTabId: t.id };
}

export function closeTab(state: PaneTreeState, tabId: string): PaneTreeState {
  if (state.tabs.length <= 1) return state;
  const remaining = state.tabs.filter((t) => t.id !== tabId);
  const activeTabId =
    tabId === state.activeTabId ? remaining[0].id : state.activeTabId;
  return { ...state, tabs: remaining, activeTabId };
}

export function switchTab(state: PaneTreeState, tabId: string): PaneTreeState {
  if (!state.tabs.some((t) => t.id === tabId)) return state;
  if (tabId === state.activeTabId) return state;
  return { ...state, activeTabId: tabId };
}

/** Switch to the Nth tab (0-indexed). Out-of-range no-ops. */
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

/** Move tab `from` index to `to` index. Out-of-range or identical no-ops. */
export function moveTab(state: PaneTreeState, from: number, to: number): PaneTreeState {
  if (from === to) return state;
  if (from < 0 || from >= state.tabs.length) return state;
  if (to < 0 || to >= state.tabs.length) return state;
  const tabs = state.tabs.slice();
  const [moved] = tabs.splice(from, 1);
  tabs.splice(to, 0, moved);
  return { ...state, tabs };
}

// ── serialization ───────────────────────────────────────────────────────────

export function serialize(state: PaneTreeState): string {
  return JSON.stringify(state);
}

/** Returns null on any decode/version mismatch. Caller falls back to
 * `initialState()`. */
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
  if (obj.version !== STATE_VERSION) return null;
  if (!Array.isArray(obj.tabs) || obj.tabs.length === 0) return null;
  if (typeof obj.activeTabId !== "string") return null;
  // Light shape check; deeper validation can land if real-world bugs surface.
  if (!obj.tabs.some((t: any) => t && t.id === obj.activeTabId)) return null;
  return obj as PaneTreeState;
}
