/**
 * Outliner-level undo / redo.
 *
 * Snapshot-based stack — each structural mutation in BlockOutliner pushes a
 * snapshot of (blocks, focusedIndex, collapsedBlocks) BEFORE applying the
 * change. Undo pops the latest snapshot, swaps state, and pushes the
 * post-change state to the redo stack. Any new mutation invalidates the
 * redo chain, matching vim's intuition.
 *
 * Intra-block text edits (typing) live on cm-editor's own history; the vim
 * `u` / `Ctrl+R` mappings in BlockEditor fall through to cm-editor's undo
 * when this stack is empty.
 *
 * Per-page lifecycle: BlockOutliner instantiates one of these. On `noteId`
 * change OR external (WebSocket) body reparse, the stack is cleared because
 * stale snapshots reference block IDs that may no longer exist.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";

export type OutlinerSnapshot = {
  blocks: ParsedBlock[];
  focusedIndex: number | null;
  collapsedBlocks: Set<string>;
};

const MAX_HISTORY = 100;

function cloneSnapshot(s: OutlinerSnapshot): OutlinerSnapshot {
  return {
    blocks: s.blocks.map((b) => ({ ...b })),
    focusedIndex: s.focusedIndex,
    collapsedBlocks: new Set(s.collapsedBlocks),
  };
}

export class OutlinerHistory {
  undoStack = $state<OutlinerSnapshot[]>([]);
  redoStack = $state<OutlinerSnapshot[]>([]);

  /** Capture a pre-mutation snapshot. Called at the top of every structural
   *  mutation function. Drops the redo chain — once you take a new path, you
   *  can't redo back to the old one. */
  push(s: OutlinerSnapshot): void {
    this.undoStack.push(cloneSnapshot(s));
    if (this.undoStack.length > MAX_HISTORY) this.undoStack.shift();
    this.redoStack.length = 0;
  }

  /** Pop the last undo snapshot. The caller passes the CURRENT state so we
   *  can stash it on the redo stack before swapping. Returns null when
   *  empty so the caller can fall through (e.g. to cm-editor history). */
  popUndo(current: OutlinerSnapshot): OutlinerSnapshot | null {
    if (this.undoStack.length === 0) return null;
    this.redoStack.push(cloneSnapshot(current));
    return this.undoStack.pop()!;
  }

  popRedo(current: OutlinerSnapshot): OutlinerSnapshot | null {
    if (this.redoStack.length === 0) return null;
    this.undoStack.push(cloneSnapshot(current));
    return this.redoStack.pop()!;
  }

  clear(): void {
    this.undoStack.length = 0;
    this.redoStack.length = 0;
  }
}
