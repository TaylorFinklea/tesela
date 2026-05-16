/**
 * Prism v5 — `⌘I` peek-popover state.
 *
 * Floating card anchored to the focused buffer. Hosts derived renderers
 * via the v5 registry (same Svelte components as a derived-buffer pane),
 * plus a `journey` kind that's Peek-only.
 *
 * Only one Peek is open at a time; pressing `⌘I` (or `K`) again closes it.
 */

/**
 * A peek "kind" maps to either a derived-renderer name from the registry,
 * or one of the Peek-only kinds (currently just `journey`). Adding a new
 * derived renderer makes it automatically available in Peek; the cycle
 * order + first-shown defaults live in workspace state.
 */
export type PeekKind = string;

/**
 * Default cycle order. The user-configurable order will live in workspace
 * state once settings exposes it (Phase 13). For now, this is the order
 * Tab walks through.
 */
export const DEFAULT_PEEK_CYCLE: ReadonlyArray<PeekKind> = [
  "backlinks-of-page",
  "outline-of-page",
  "properties-of-page",
  "tasks-linked-to-page",
  "local-graph-of-page",
  "journey",
];

let open = $state(false);
let kind = $state<PeekKind>("backlinks-of-page");
let anchorPaneId = $state<string | undefined>(undefined);

export function isPeekOpen(): boolean {
  return open;
}

export function getPeekKind(): PeekKind {
  return kind;
}

export function setPeekKind(k: PeekKind) {
  kind = k;
}

export function getPeekAnchorPaneId(): string | undefined {
  return anchorPaneId;
}

export function getPeekKinds(): ReadonlyArray<PeekKind> {
  return DEFAULT_PEEK_CYCLE;
}

/**
 * Cycle to the next/previous peek kind in `DEFAULT_PEEK_CYCLE`, skipping
 * any kind in `hideList`. Returns the new kind.
 */
export function cyclePeek(
  dir: 1 | -1,
  hideList: ReadonlySet<PeekKind> = new Set(),
): PeekKind {
  const visible = DEFAULT_PEEK_CYCLE.filter((k) => !hideList.has(k));
  if (visible.length === 0) return kind;
  const i = visible.indexOf(kind);
  const next = visible[(i + dir + visible.length) % visible.length];
  kind = next;
  return next;
}

export function openPeek(initialKind: PeekKind = "backlinks-of-page", paneId?: string) {
  kind = initialKind;
  anchorPaneId = paneId;
  open = true;
}

export function togglePeek(paneId?: string) {
  if (open) closePeek();
  else openPeek(kind, paneId);
}

export function closePeek() {
  open = false;
}
