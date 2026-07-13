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

export type TextReconciliationPlan =
  | { kind: "unchanged"; text: string }
  | { kind: "incremental"; events: TextChangeSpec[][]; text: string }
  | { kind: "canonical"; text: string };

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

function validLength(value: number): boolean {
  return Number.isSafeInteger(value) && value >= 0;
}

function validatedDeltaToChanges(
  delta: readonly TextDeltaOp[],
  docLength: number,
): TextChangeSpec[] | null {
  const changes: TextChangeSpec[] = [];
  let pos = 0;

  for (const op of delta) {
    const hasRetain = typeof op.retain === "number";
    const hasInsert = typeof op.insert === "string";
    const hasDelete = typeof op.delete === "number";
    if (Number(hasRetain) + Number(hasInsert) + Number(hasDelete) !== 1) return null;

    if (hasRetain) {
      if (!validLength(op.retain!) || pos + op.retain! > docLength) return null;
      pos += op.retain!;
    } else if (hasInsert) {
      if (op.insert!.length > 0) changes.push({ from: pos, to: pos, insert: op.insert! });
    } else {
      if (!validLength(op.delete!) || pos + op.delete! > docLength) return null;
      if (op.delete! > 0) changes.push({ from: pos, to: pos + op.delete!, insert: "" });
      pos += op.delete!;
    }
  }

  return changes;
}

function applyChanges(doc: string, changes: readonly TextChangeSpec[]): string | null {
  let cursor = 0;
  let result = "";
  for (const change of changes) {
    if (
      !validLength(change.from)
      || !validLength(change.to)
      || change.from < cursor
      || change.to < change.from
      || change.to > doc.length
    ) {
      return null;
    }
    result += doc.slice(cursor, change.from) + change.insert;
    cursor = change.to;
  }
  return result + doc.slice(cursor);
}

/**
 * Choose the safe way to project Loro text events into a CodeMirror document.
 * Event deltas are only an optimization: they are accepted when every event is
 * valid against the view state produced by the previous event AND that full
 * projection exactly equals the subscribed LoroText's canonical value.
 * Otherwise callers must replace the view with `text` wholesale.
 */
export function planTextReconciliation(
  currentText: string,
  eventDeltas: readonly (readonly TextDeltaOp[])[],
  canonicalText: string,
): TextReconciliationPlan {
  let projected = currentText;
  const events: TextChangeSpec[][] = [];
  let hasChanges = false;

  for (const delta of eventDeltas) {
    const changes = validatedDeltaToChanges(delta, projected.length);
    if (!changes) return { kind: "canonical", text: canonicalText };
    const next = applyChanges(projected, changes);
    if (next === null) return { kind: "canonical", text: canonicalText };
    projected = next;
    events.push(changes);
    if (changes.length > 0) hasChanges = true;
  }

  if (projected !== canonicalText) return { kind: "canonical", text: canonicalText };
  if (!hasChanges) return { kind: "unchanged", text: canonicalText };
  return { kind: "incremental", events, text: canonicalText };
}
