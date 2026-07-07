/**
 * Multi-note Loro doc registry — core logic (tesela-baa).
 *
 * Evolves the single "active" NoteDoc (C2.2) into a ref-counted registry of
 * open docs, one per note with at least one mounted editor surface. The 9iy
 * storm proved the singleton's gap: only the FOCUSED note's blocks got the
 * char-splice path (the journal's default daily buffer binds today's slug
 * only), so typing into any other visible note — a past day in the journal, a
 * drawer tab, a tag page — fell back to 500ms whole-block writes and raced
 * same-block concurrent edits at block granularity. Every mounted
 * BlockOutliner now acquires its note's doc, so every visible editor splices.
 *
 * Pure + dependency-injected (no $app/browser, no wasm, no WS) so node unit
 * tests drive it directly; the svelte glue (note-doc-registry.svelte.ts)
 * supplies the real NoteDoc factory, rAF scheduling, and the TLR2 WS send.
 *
 * Lifecycle: acquire/release are ref-counted; the doc closes when the last
 * surface releases it. Release flushes pending outbound ops BEFORE closing so
 * a splice typed just before navigating away still ships (the old singleton
 * reset its cursor on every focus switch without flushing — a small loss
 * window this replaces).
 */

export interface RegistryUpdate {
  doc: Uint8Array;
  updateBytes: Uint8Array;
}

/** The NoteDoc surface the registry needs (note-doc.ts satisfies this). */
export interface RegistryDoc {
  slug: string | null;
  noteId16: Uint8Array | null;
  open(slug: string): Promise<void>;
  close(): void;
  spliceBlock(bid: string, utf16Offset: number, utf16DeleteLen: number, insert: string): boolean;
  applyInbound(updates: RegistryUpdate[]): void;
  exportSince(since: unknown | null): Uint8Array;
  currentVersion(): unknown | null;
  undo(): boolean;
  redo(): boolean;
  canUndo(): boolean;
  canRedo(): boolean;
}

export interface RegistryDeps<D extends RegistryDoc> {
  createDoc(): D;
  /** Schedule a coalesced outbound flush (rAF in the browser). */
  scheduleFlush(cb: () => void): unknown;
  cancelFlush(handle: unknown): void;
  /** Ship one note's exported delta to the server. MUST return whether the
   *  payload was really handed off (false = socket not open, frame dropped) —
   *  the outbound cursor only advances on a true handoff, so ops typed during
   *  a WS outage re-export on the next flush instead of stranding. */
  send(update: RegistryUpdate): boolean;
}

interface Entry<D extends RegistryDoc> {
  doc: D;
  refs: number;
  opening: Promise<void>;
  /** Doc version at the last SUCCESSFUL outbound send; exportSince(this)
   *  ships only newer ops. Baselined to the doc's post-bootstrap version once
   *  open resolves — a null cursor would make the first flush export the
   *  doc's ENTIRE history (bootstrap snapshot ops included), megabytes for a
   *  large note that was merely viewed. */
  lastSentVV: unknown | null;
  /** True when a LOCAL edit (splice / undo / redo) happened since the last
   *  successful flush. Flushes are no-ops on clean docs — remote-only imports
   *  must never trigger an outbound re-broadcast on release. */
  dirty: boolean;
  flushHandle: unknown | null;
}

function hex(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i++) s += bytes[i].toString(16).padStart(2, "0");
  return s;
}

export class NoteDocRegistry<D extends RegistryDoc> {
  #deps: RegistryDeps<D>;
  #entries = new Map<string, Entry<D>>();
  /** Editor-id-guarded focus: a late blur from a previously-focused editor
   *  must not clobber a fresh focus on another (same pattern as the
   *  focused-editor store). Undo/redo route to the focused note's doc. */
  #focusedEditorId: string | null = null;
  #focusedSlug: string | null = null;
  #undoApplying = false;

  constructor(deps: RegistryDeps<D>) {
    this.#deps = deps;
  }

  /** Ref-counted open. Safe to call while already open (refs++) — including a
   *  doc parked at zero refs awaiting a reconnect flush (it resurrects). */
  acquire(slug: string): Promise<void> {
    const existing = this.#entries.get(slug);
    if (existing) {
      existing.refs += 1;
      return existing.opening;
    }
    const doc = this.#deps.createDoc();
    const entry: Entry<D> = {
      doc,
      refs: 1,
      opening: doc.open(slug),
      lastSentVV: null,
      dirty: false,
      flushHandle: null,
    };
    this.#entries.set(slug, entry);
    // Baseline the outbound cursor at the doc's post-bootstrap version so the
    // first dirty flush exports only local edits, never the full history. A
    // pre-bootstrap local edit can't happen (splices need the block resident),
    // but guard on dirty anyway so a cursor can never leapfrog unsent ops.
    entry.opening.then(() => {
      if (this.#entries.get(slug) === entry && entry.lastSentVV === null && !entry.dirty) {
        entry.lastSentVV = entry.doc.currentVersion();
      }
    });
    return entry.opening;
  }

  /** Ref-counted close. Flushes pending outbound ops before the final close.
   *  If that flush cannot hand off (WS down), the entry is PARKED at zero refs
   *  instead of closed — its ops are the only copy of the user's last edits —
   *  and drained by the next flushAll (the layout calls it on WS reconnect). */
  release(slug: string): void {
    const entry = this.#entries.get(slug);
    if (!entry) return;
    entry.refs -= 1;
    if (entry.refs > 0) return;
    if (entry.flushHandle !== null) {
      this.#deps.cancelFlush(entry.flushHandle);
      entry.flushHandle = null;
    }
    this.#flushEntry(entry);
    if (entry.dirty) return; // parked: unsent local ops survive until reconnect
    this.#closeEntry(slug, entry);
  }

  #closeEntry(slug: string, entry: Entry<D>): void {
    if (entry.flushHandle !== null) {
      this.#deps.cancelFlush(entry.flushHandle);
      entry.flushHandle = null;
    }
    entry.doc.close();
    this.#entries.delete(slug);
    if (this.#focusedSlug === slug) {
      this.#focusedSlug = null;
      this.#focusedEditorId = null;
    }
  }

  doc(slug: string | null | undefined): D | null {
    if (!slug) return null;
    return this.#entries.get(slug)?.doc ?? null;
  }

  /** Apply a local character splice to `bid`'s text_seq on `slug`'s doc, then
   *  schedule that doc's coalesced outbound flush. False → caller falls back
   *  to the whole-text HTTP path (not open, block not in doc, splice failed). */
  splice(
    slug: string | null | undefined,
    bid: string,
    utf16Offset: number,
    utf16DeleteLen: number,
    insert: string,
  ): boolean {
    if (!slug) return false;
    const entry = this.#entries.get(slug);
    if (!entry) return false;
    const ok = entry.doc.spliceBlock(bid, utf16Offset, utf16DeleteLen, insert);
    if (ok) {
      entry.dirty = true;
      this.#scheduleFlush(slug, entry);
    }
    return ok;
  }

  /** Route inbound TLR2 updates to open docs by 16-byte note id. Returns the
   *  updates that matched NO open doc (callers broad-refresh for those). */
  applyInbound(updates: RegistryUpdate[]): RegistryUpdate[] {
    if (updates.length === 0) return [];
    const byId = new Map<string, Entry<D>>();
    for (const entry of this.#entries.values()) {
      const id16 = entry.doc.noteId16;
      if (id16) byId.set(hex(id16), entry);
    }
    const unmatched: RegistryUpdate[] = [];
    for (const u of updates) {
      const entry = byId.get(hex(u.doc));
      if (entry) entry.doc.applyInbound([u]);
      else unmatched.push(u);
    }
    return unmatched;
  }

  #scheduleFlush(slug: string, entry: Entry<D>): void {
    if (entry.flushHandle !== null) return;
    entry.flushHandle = this.#deps.scheduleFlush(() => {
      entry.flushHandle = null;
      // The entry may have been released+closed before the flush fired; the
      // release path already did its own final flush then.
      if (this.#entries.get(slug) === entry) this.#flushEntry(entry);
    });
  }

  /** Export `slug`'s dirty delta since its last successful send and ship it. */
  flush(slug: string): void {
    const entry = this.#entries.get(slug);
    if (!entry) return;
    if (entry.flushHandle !== null) {
      this.#deps.cancelFlush(entry.flushHandle);
      entry.flushHandle = null;
    }
    this.#flushEntry(entry);
    // A parked entry (zero refs, kept alive only for its unsent ops) closes
    // as soon as a flush finally hands its ops off.
    if (!entry.dirty && entry.refs <= 0 && this.#entries.get(slug) === entry) {
      this.#closeEntry(slug, entry);
    }
  }

  /** Flush every open doc. The layout calls this on WS reconnect so ops that
   *  couldn't hand off during the outage (cursor un-advanced) ship now. */
  flushAll(): void {
    for (const slug of [...this.#entries.keys()]) this.flush(slug);
  }

  #flushEntry(entry: Entry<D>): void {
    if (!entry.dirty) return; // clean docs never re-broadcast (remote-only imports)
    const noteId16 = entry.doc.noteId16;
    if (!noteId16) return;
    const bytes = entry.doc.exportSince(entry.lastSentVV);
    if (bytes.length === 0) {
      entry.dirty = false;
      return;
    }
    // Advance the cursor ONLY on a confirmed handoff: sendBinary drops the
    // frame when the socket isn't open, and an advanced cursor would exclude
    // these ops from every later export — silently losing the keystrokes.
    if (!this.#deps.send({ doc: noteId16, updateBytes: bytes })) return;
    const v = entry.doc.currentVersion();
    if (v) entry.lastSentVV = v;
    entry.dirty = false;
  }

  // ── focused-doc tracking (undo/redo routing) ──────────────────────────────

  setFocused(editorId: string, slug: string | null | undefined): void {
    this.#focusedEditorId = editorId;
    this.#focusedSlug = slug ?? null;
  }

  clearFocused(editorId: string): void {
    if (this.#focusedEditorId !== editorId) return;
    this.#focusedEditorId = null;
    this.#focusedSlug = null;
  }

  focusedSlug(): string | null {
    return this.#focusedSlug;
  }

  /** True while a Loro undo/redo applies its inverse ops. The editor binding
   *  skips `by:"local"` events (its own splices) — but undo's inverse ops are
   *  ALSO local and the editor does NOT have them yet, so it must apply them
   *  while this flag is set. */
  isUndoApplying(): boolean {
    return this.#undoApplying;
  }

  undoFocused(): boolean {
    return this.#runUndoLike((doc) => doc.undo());
  }

  redoFocused(): boolean {
    return this.#runUndoLike((doc) => doc.redo());
  }

  canUndoFocused(): boolean {
    return this.doc(this.#focusedSlug)?.canUndo() ?? false;
  }

  canRedoFocused(): boolean {
    return this.doc(this.#focusedSlug)?.canRedo() ?? false;
  }

  #runUndoLike(op: (doc: D) => boolean): boolean {
    const slug = this.#focusedSlug;
    const entry = slug ? this.#entries.get(slug) : null;
    if (!slug || !entry) return false;
    this.#undoApplying = true;
    let did = false;
    try {
      did = op(entry.doc);
    } finally {
      // Clear AFTER the loro change events (fired by the inverse-op commit)
      // have been delivered; queueMicrotask runs after any microtask the
      // commit enqueued during this synchronous call.
      queueMicrotask(() => {
        this.#undoApplying = false;
      });
    }
    // Ship the inverse ops immediately so the persisted note reflects the
    // undo even if the user navigates away right after.
    if (did) {
      entry.dirty = true;
      this.flush(slug);
    }
    return did;
  }

  // ── test/introspection helpers ────────────────────────────────────────────

  size(): number {
    return this.#entries.size;
  }

  refs(slug: string): number {
    return this.#entries.get(slug)?.refs ?? 0;
  }
}
