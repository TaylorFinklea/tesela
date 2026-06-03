/**
 * The single "active" NoteDoc for the web peer (C2.2).
 *
 * A process-wide singleton holding ONE {@link NoteDoc} for the note the user is
 * currently focused on. Two callers drive it from different layers, so it lives
 * here rather than in either component:
 *
 *   - the v4/v5 layout opens it for the focused page's slug whenever that slug
 *     changes (browser-only `$effect`), and
 *   - the root `+layout.svelte` WS `onBinaryDelta` handler feeds inbound TLR2
 *     deltas into it.
 *
 * For C2.2 this only MAINTAINS the doc + proves convergence; it is NOT wired to
 * the editor (that's C2.3). Keeping exactly one doc — closed/reopened as the
 * focus slug changes — is what prevents doc/subscription leaks while navigating.
 *
 * Browser-only: `NoteDoc` pulls in `loro-crdt` (wasm), so every entry point
 * here is a no-op under SSR.
 */
import { browser } from "$app/environment";
import { NoteDoc } from "./note-doc";
import { encodeTlr2, type LoroDocUpdate } from "./tlr2";
import { sendBinary } from "$lib/ws-client.svelte";
import type { VersionVector } from "loro-crdt";

let active: NoteDoc | null = null;
/** Slug the singleton is currently open on, to dedupe redundant opens. */
let activeSlug: string | null = null;
/** Per-doc cursor: the doc version vector at the last outbound delta send.
 *  `exportSince(lastSentVV)` ships only ops newer than this — the next splice
 *  doesn't re-send the whole history. Reset to null on every (re)open so a new
 *  note starts from a clean cursor. */
let lastSentVV: VersionVector | null = null;
/** Coalesce many tiny splices in one frame (a burst of keystrokes) into a
 *  single export+send on the next animation frame. */
let flushHandle: ReturnType<typeof requestAnimationFrame> | null = null;

function ensure(): NoteDoc {
  if (!active) active = new NoteDoc();
  return active;
}

/**
 * Open (or re-open) the active doc on `slug`. Idempotent for the same slug.
 * Pass null/empty (no focused page) to close the active doc. Browser-only.
 * Returns the promise so callers can await the bootstrap if they need to;
 * navigation drivers can fire-and-forget.
 */
export function openActiveNoteDoc(slug: string | null): Promise<void> {
  if (!browser) return Promise.resolve();
  if (!slug) {
    if (active) {
      active.close();
      activeSlug = null;
    }
    resetOutbound();
    return Promise.resolve();
  }
  if (slug === activeSlug && active?.slug === slug) return Promise.resolve();
  activeSlug = slug;
  resetOutbound();
  return ensure().open(slug);
}

/** Drop the outbound send cursor + any pending flush. Called on every open so
 *  a delta for the previous note can't leak into the new one's cursor. */
function resetOutbound(): void {
  lastSentVV = null;
  if (flushHandle !== null) {
    cancelAnimationFrame(flushHandle);
    flushHandle = null;
  }
}

/** Feed inbound TLR2 deltas to the active doc (no-op if none open / SSR). */
export function applyInboundToActive(updates: LoroDocUpdate[]): void {
  if (!browser || !active) return;
  active.applyInbound(updates);
}

/** The active NoteDoc, or null when nothing is open. C2.3 reads through this. */
export function getActiveNoteDoc(): NoteDoc | null {
  return active;
}

/**
 * Apply a local character splice (UTF-16 index space, CM offsets pass straight
 * through) to block `bid`'s `text_seq` on the active doc, then schedule a
 * coalesced delta broadcast. Returns false (no-op) when no doc is open, the
 * block isn't in the doc, or the splice fails — the caller keeps its existing
 * whole-text fallback intact in that case. Browser-only.
 */
export function spliceActiveBlock(
  bid: string,
  utf16Offset: number,
  utf16DeleteLen: number,
  insert: string,
): boolean {
  if (!browser || !active) return false;
  const ok = active.spliceBlock(bid, utf16Offset, utf16DeleteLen, insert);
  if (ok) scheduleOutboundFlush();
  return ok;
}

/** Schedule (or keep) a single outbound flush on the next animation frame.
 *  Multiple splices in the same frame collapse into one export+send. */
function scheduleOutboundFlush(): void {
  if (!browser) return;
  if (flushHandle !== null) return;
  flushHandle = requestAnimationFrame(() => {
    flushHandle = null;
    flushActiveOutbound();
  });
}

/**
 * Export the active doc's delta since the last send, frame it as TLR2 (one
 * update keyed by the note's 16-byte id), and ship it over the WS. Advances
 * the last-sent cursor only after a non-empty export so a failed/no-op send
 * doesn't strand newer ops. Public so a flush-before-navigate can force it.
 */
export function flushActiveOutbound(): void {
  if (!browser || !active) return;
  const noteId16 = active.noteId16;
  if (!noteId16) return;
  const bytes = active.exportSince(lastSentVV);
  if (bytes.length === 0) return;
  const frame = encodeTlr2([{ doc: noteId16, updateBytes: bytes }]);
  sendBinary(frame);
  // Advance the cursor to the version we just exported so the next export
  // only carries newer ops.
  const v = active.currentVersion();
  if (v) lastSentVV = v;
}
