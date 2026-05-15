/**
 * Prism v4 — `i` peek-popover state.
 *
 * Floating card anchored to the focused pane. Shows quick context about
 * the focused tile WITHOUT opening a dedicated context pane. The kind
 * dropdown lets the user flip between views (backlinks / properties /
 * outline / journey / timeline / graph) — useful for one-off peeks where
 * a full split would be overkill.
 *
 * Only one is open at a time; pressing `i` again closes it.
 */

export type PeekKind =
  | "backlinks"
  | "properties"
  | "outline"
  | "journey"
  | "timeline"
  | "graph";

const KINDS: ReadonlyArray<PeekKind> = [
  "backlinks",
  "properties",
  "outline",
  "journey",
  "timeline",
  "graph",
];

let open = $state(false);
let kind = $state<PeekKind>("backlinks");
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
  return KINDS;
}

export function openPeek(initialKind: PeekKind = "backlinks", paneId?: string) {
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
