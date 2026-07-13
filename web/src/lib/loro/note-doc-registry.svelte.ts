/**
 * Browser glue for the multi-note Loro doc registry (tesela-baa).
 *
 * Replaces the single "active" NoteDoc (C2.2's active-note-doc singleton):
 * every mounted BlockOutliner acquires its note's doc, so every visible
 * editor — each journal day, drawer tab, tag page — gets the char-splice
 * collab path, not just the focused note. Core logic + lifecycle live in
 * {@link NoteDocRegistry} (doc-registry.ts, unit-tested); this module binds
 * it to the real wasm NoteDoc, rAF coalescing, and the TLR2 binary WS send.
 *
 * Browser-only: `NoteDoc` pulls in `loro-crdt` (wasm), so every entry point
 * here is a no-op under SSR.
 */
import { browser } from "$app/environment";
import { NoteDoc } from "./note-doc";
import { NoteDocRegistry, type RegistryUpdate } from "./doc-registry";
import { encodeTlr2, type LoroDocUpdate } from "./tlr2";
import { ServerBarrierRetryQueue } from "./server-barrier";
import { awaitLoroServerBarrier, sendBinary } from "$lib/ws-client.svelte";
import { propertyMutationBarrier } from "$lib/block-ops-saver";

const barrierRetries = browser
  ? new ServerBarrierRetryQueue<string>({
      initialDelayMs: 250,
      maxDelayMs: 8_000,
      schedule: (cb, delayMs) => setTimeout(cb, delayMs),
      cancelSchedule: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
      run: (slugs) => settleNoteDocsAtServer(slugs),
    })
  : null;

const registry = browser
  ? new NoteDocRegistry<NoteDoc>({
      createDoc: () => new NoteDoc(),
      scheduleFlush: (cb) => requestAnimationFrame(cb),
      cancelFlush: (handle) => cancelAnimationFrame(handle as number),
      send: (u) => sendBinary(encodeTlr2([{ doc: u.doc, updateBytes: u.updateBytes }])),
      requestBarrierRetry: (slugs) => barrierRetries?.enqueue(slugs),
      completeBarrierRetry: (slugs) => barrierRetries?.resolve(slugs),
    })
  : null;

/** Ref-counted open of `slug`'s doc (bootstraps from the server snapshot on
 *  first acquire). Every mounted editor surface for a note holds one ref. */
export function acquireNoteDoc(slug: string): Promise<void> {
  if (!registry || !slug) return Promise.resolve();
  return registry.acquire(slug);
}

/** Ref-counted release; the doc flushes pending ops and closes at zero refs. */
export function releaseNoteDoc(slug: string): void {
  if (!registry || !slug) return;
  registry.release(slug);
}

/** The open NoteDoc for `slug`, or null. The editor binding reads through
 *  this (block containers, live-block mirrors). */
export function getNoteDoc(slug: string | null | undefined): NoteDoc | null {
  return registry?.doc(slug) ?? null;
}

/**
 * Apply a local character splice (UTF-16 index space, CM offsets pass straight
 * through) to block `bid`'s `text_seq` on `slug`'s doc, then schedule that
 * doc's coalesced delta broadcast. Returns false (no-op) when the doc isn't
 * open, the block isn't in it, or the splice fails — the caller keeps its
 * whole-text fallback intact in that case.
 */
export function spliceNoteBlock(
  slug: string | null | undefined,
  bid: string,
  utf16Offset: number,
  utf16DeleteLen: number,
  insert: string,
): boolean {
  if (slug && propertyMutationBarrier.isReserved(slug)) return false;
  return registry?.splice(slug, bid, utf16Offset, utf16DeleteLen, insert) ?? false;
}

/** Feed inbound TLR2 deltas to whichever open docs they target. Returns the
 *  updates that matched no open doc (the caller broad-refreshes for those). */
export function applyInboundToOpenDocs(updates: LoroDocUpdate[]): RegistryUpdate[] {
  if (!registry) return [];
  return registry.applyInbound(updates);
}

/** Force every open doc's pending outbound delta onto the wire (e.g. before
 *  the shell tears down). */
export function flushAllOutbound(): void {
  registry?.flushAll();
}

let serverBarrierQueue: Promise<void> = Promise.resolve();

/**
 * Settle all affected mounted note docs on the server. Calls are serialized
 * connection-wide so two panes cannot acknowledge checkpoints out of order.
 * Every server-backed addressed note participates. A synthetic append
 * destination is omitted only after its caller drains every mounted/unmounted
 * save queue and authoritatively confirms that the note is still absent; this
 * keeps another pane's newly-created doc inside the proof. An absent included
 * registry doc fails closed.
 */
export function settleNoteDocsAtServer(slugs: Iterable<string>): Promise<void> {
  if (!registry) return Promise.resolve();
  const affected = [...new Set(slugs)];
  const run = async () => {
    await registry.waitUntilOpen(affected);
    await awaitLoroServerBarrier((sendCaptured) => {
      const prepared = registry.prepareServerBarrier(affected, (update) =>
        sendCaptured(encodeTlr2([{ doc: update.doc, updateBytes: update.updateBytes }])),
      );
      if (!prepared) throw new Error("Unable to hand affected Loro docs to the captured socket");
      return prepared;
    });
  };
  const completion = serverBarrierQueue.then(run, run);
  serverBarrierQueue = completion.catch(() => {});
  return completion;
}

// ── focused-doc tracking: vim undo/redo route to the note being edited ──────

/** Editor-id-guarded (a late blur can't clobber a fresh focus elsewhere). */
export function setFocusedNoteDoc(editorId: string, slug: string | null | undefined): void {
  registry?.setFocused(editorId, slug);
}

export function clearFocusedNoteDoc(editorId: string): void {
  registry?.clearFocused(editorId);
}

/** True while a Loro undo/redo is applying its inverse ops. The editor binding
 *  skips `by: "local"` text events (its own splices, already in CM) — but
 *  undo's inverse ops are ALSO `by: "local"` and the editor does NOT have them
 *  yet, so the binding must apply them WHILE this flag is set. */
export function isLoroUndoApplying(): boolean {
  return registry?.isUndoApplying() ?? false;
}

/** Undo the last local text edit on the focused note's doc (vim #12). Returns
 *  true if something was undone (the caller stops before structural undo). */
export function undoFocusedDoc(): boolean {
  return registry?.undoFocused() ?? false;
}

/** Redo the last undone text edit on the focused note's doc. */
export function redoFocusedDoc(): boolean {
  return registry?.redoFocused() ?? false;
}

export function canUndoFocusedDoc(): boolean {
  return registry?.canUndoFocused() ?? false;
}

export function canRedoFocusedDoc(): boolean {
  return registry?.canRedoFocused() ?? false;
}
