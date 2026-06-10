/**
 * Triage flow for the Inbox widget (Phase 9.2). Single-key handlers that fire
 * when the middle column has focus AND the active widget is `inbox`.
 *
 * `t` → set status:: todo
 * `d` → set status:: doing
 * `x` → set status:: done (archive — drops out of inbox query)
 *
 * Implementation (P1.13 structured-first): a triage action is a single
 * CONTAINER property write — `api.setBlockProperty(blockId, key, value)`
 * (`POST /blocks/set-property`), the same endpoint the outliner's
 * `setBlockPropertyStructured` and QueryWidgetView's status cycle use. The
 * server resolves the row's block address (`<note_id>:<line>` or
 * `<note_id>:<bid>`), writes the typed container, and materializes exactly one
 * `key:: value` line. The previous GET → text-splice → whole-note PUT here
 * violated BOTH editing invariants: the base-less PUT re-asserted any block a
 * peer edited in the GET→PUT window (the whole-body clobber the block-ops
 * redesign closed), and the `status::` TEXT line landed in the block's
 * `text_seq`, diverging from — and being silently outvoted by — the container
 * value on any block whose status was previously set structurally.
 * The subsequent WS echo invalidates `["widget", "inbox"]` and the row drops
 * out of the list.
 *
 * The pure body-mutation helpers (`setBlockProperty`, `setBlockText`,
 * `deleteBlock`) live in `block-mutations.ts` so they can be unit-tested
 * without dragging in `$lib/api-client`; re-exported here so callers keep
 * importing `$lib/triage.svelte`.
 */
import { api, ApiError } from "$lib/api-client";

export { setBlockProperty, setBlockText, deleteBlock } from "$lib/block-mutations";

export type TriageAction = "todo" | "doing" | "done";

const ACTIONS: Record<string, TriageAction> = {
  t: "todo",
  d: "doing",
  x: "done",
};

export function triageActionForKey(key: string): TriageAction | null {
  return ACTIONS[key.toLowerCase()] ?? null;
}

/** Container property write shared by the triage actions. Returns true when
 *  the write landed; false when the server couldn't locate the block or note
 *  (404 — a stale row), matching the old "block couldn't be located" contract
 *  the callers' partial-failure accounting relies on. Other failures
 *  (network, 5xx) still throw. */
async function setBlockPropertyContainer(
  blockId: string,
  key: string,
  value: string,
): Promise<boolean> {
  try {
    await api.setBlockProperty(blockId, key, value);
    return true;
  } catch (e) {
    if (e instanceof ApiError && e.status === 404) return false;
    throw e;
  }
}

/**
 * Apply a triage action to the block identified by `blockId` (full
 * `<note_id>:<line>` or `<note_id>:<bid>` address, as carried by inbox /
 * query rows). Returns true if the property write landed; false if the block
 * couldn't be located (e.g. stale row). `_pageId` is retained for call-site
 * compatibility — the block address already encodes the note.
 */
export async function applyTriage(
  _pageId: string,
  blockId: string,
  action: TriageAction,
): Promise<boolean> {
  return setBlockPropertyContainer(blockId, "status", action);
}

/**
 * Attach a block to a project page by setting `project:: <projectId>` — the
 * same container write path as `applyTriage`.
 */
export async function attachToProject(
  _pageId: string,
  blockId: string,
  projectId: string,
): Promise<boolean> {
  return setBlockPropertyContainer(blockId, "project", projectId);
}
