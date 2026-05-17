/**
 * Pure prune predicate, separated from the api-touching scratch-prune
 * module so tests can run under Node's strip-only TS mode without
 * dragging in `$lib/...` imports.
 */

import type { Note } from "../types/Note.ts";

/** Decide whether a given note should be pruned. Pure — accepts the note
 *  and the threshold cutoff (a Date). Notes without a `modified_at`
 *  field fall back to `created_at`; if neither exists, they're skipped
 *  (we can't tell their age). */
export function shouldPrune(note: Note, cutoff: Date): boolean {
  if (note.metadata.note_type !== "scratch") return false;
  const stamp = note.modified_at ?? note.created_at;
  if (!stamp) return false;
  const t = Date.parse(stamp);
  if (Number.isNaN(t)) return false;
  return t < cutoff.getTime();
}
