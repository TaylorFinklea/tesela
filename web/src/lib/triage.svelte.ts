/**
 * Triage flow for the Inbox widget (Phase 9.2). Single-key handlers that fire
 * when the middle column has focus AND the active widget is `inbox`.
 *
 * `t` → set status:: todo
 * `d` → set status:: doing
 * `x` → set status:: done (archive — drops out of inbox query)
 *
 * Implementation: fetches the focused row's containing note, edits the
 * referenced block's body to insert/replace `status:: <value>`, PUTs back. The
 * subsequent WS echo invalidates `["widget", "inbox"]` and the row drops out
 * of the list.
 *
 * The pure body-mutation helpers (`setBlockProperty`, `setBlockText`,
 * `deleteBlock`) live in `block-mutations.ts` so they can be unit-tested
 * without dragging in `$lib/api-client`; re-exported here so callers keep
 * importing `$lib/triage.svelte`.
 */
import { api } from "$lib/api-client";
import { setBlockProperty } from "$lib/block-mutations";

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

/**
 * Apply a triage action to the block identified by `blockId` inside the note
 * `pageId`. Returns true if the PUT was issued; false if the block couldn't
 * be located (e.g. stale row).
 */
export async function applyTriage(
  pageId: string,
  blockId: string,
  action: TriageAction,
): Promise<boolean> {
  const note = await api.getNote(pageId);
  const updated = setBlockStatus(note.content, blockId, action);
  if (updated === note.content) return false;
  await api.updateNote(pageId, updated);
  return true;
}

/**
 * Insert (or replace) a `status:: <value>` continuation line on the block.
 * Convenience wrapper around `setBlockProperty`.
 */
export function setBlockStatus(
  content: string,
  blockId: string,
  action: TriageAction,
): string {
  return setBlockProperty(content, blockId, "status", action);
}

/**
 * Attach a block to a project page by setting `project:: <projectId>`.
 * Wraps `setBlockProperty` + the API PUT.
 */
export async function attachToProject(
  pageId: string,
  blockId: string,
  projectId: string,
): Promise<boolean> {
  const note = await api.getNote(pageId);
  const updated = setBlockProperty(note.content, blockId, "project", projectId);
  if (updated === note.content) return false;
  await api.updateNote(pageId, updated);
  return true;
}
