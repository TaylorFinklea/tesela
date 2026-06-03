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
import type { LoroDocUpdate } from "./tlr2";

let active: NoteDoc | null = null;
/** Slug the singleton is currently open on, to dedupe redundant opens. */
let activeSlug: string | null = null;

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
    return Promise.resolve();
  }
  if (slug === activeSlug && active?.slug === slug) return Promise.resolve();
  activeSlug = slug;
  return ensure().open(slug);
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
