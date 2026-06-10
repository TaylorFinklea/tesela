/**
 * Pure mapping from a Loro TextDiff (quill delta: retain / insert / delete
 * runs, UTF-16 index space) to a CodeMirror change array.
 *
 * CM6 interprets EVERY `from`/`to` in a multi-change dispatch relative to the
 * ORIGINAL (pre-transaction) document — the same convention
 * `applyLocalSplicesToLoro` and `promoteOrDemoteTag` document on the write
 * side. So the position counter here walks ORIGINAL-doc coordinates:
 *
 *   - `retain n`  → advance by n (n original chars are kept untouched);
 *   - `delete n`  → emit {from: pos, to: pos + n} AND advance by n (a delete
 *                   consumes n original chars);
 *   - `insert s`  → emit {from: pos, to: pos} and do NOT advance (an insert
 *                   consumes zero original chars).
 *
 * Getting this inverted (advancing on insert, not on delete — the pre-fix
 * code) misplaces every run after the first non-retain in a multi-run delta:
 * a peer's Alt-Enter tag demote ("see #foo bar" → delete 4..9 + insert@12)
 * would land its insert INSIDE the delete range and corrupt — or throw out
 * of — the receiving editor. Kept Svelte/CM-free so the coordinate contract
 * is unit-tested directly (web/tests/unit/loro-text-delta.test.mjs).
 *
 * NOTE: a delta is relative to ONE document state. When a LoroEventBatch
 * carries multiple text events, each event's delta is relative to the doc
 * AFTER the previous event applied — callers must dispatch one change set
 * per event, not concatenate them.
 */

/** One run of a Loro TextDiff quill delta (structural subset of loro-crdt's
 *  `TextDiff["diff"]` entries — attributes are ignored). */
export type TextDeltaOp = { retain?: number; insert?: string; delete?: number };

/** A CM6 change spec in original-doc coordinates. */
export type TextChangeSpec = { from: number; to: number; insert: string };

export function deltaToChanges(delta: readonly TextDeltaOp[]): TextChangeSpec[] {
  const changes: TextChangeSpec[] = [];
  let pos = 0;
  for (const op of delta) {
    if (typeof op.retain === "number") {
      pos += op.retain;
    } else if (typeof op.insert === "string") {
      changes.push({ from: pos, to: pos, insert: op.insert });
      // No advance: an insert consumes zero original-doc length.
    } else if (typeof op.delete === "number") {
      changes.push({ from: pos, to: pos + op.delete, insert: "" });
      // Advance: a delete consumes original-doc length.
      pos += op.delete;
    }
  }
  return changes;
}
