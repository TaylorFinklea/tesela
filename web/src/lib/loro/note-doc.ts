/**
 * NoteDoc — the C2.2 per-note Loro doc lifecycle for the web peer.
 *
 * Holds ONE `LoroDoc` for the note the user currently has open, keeps it
 * converged with the server by (a) bootstrapping from the server snapshot on
 * open and (b) applying inbound binary TLR2 deltas in real time. This is the
 * DOC layer only: it does NOT touch the CodeMirror editor, BlockOutliner,
 * block-ops, or any save path — the editor binding lands in C2.3. The read
 * helpers and change subscription exist so C2.3 has a stable surface to consume.
 *
 * Browser-only: the underlying `loro-crdt` is wasm and must never load during
 * SSR. Every wasm path here is reached via the `loro-client` helpers, which are
 * `browser`-guarded. Callers should still only `open()` from a client lifecycle
 * (`onMount` / a browser-guarded `$effect`).
 *
 * Doc model (mirrors `crates/tesela-sync/src/engine/loro_engine.rs`):
 *   - a movable tree at key `"blocks"`; root-level children are the blocks
 *   - each node's meta map (`node.data`) carries:
 *       - `block_id`    — dashless lowercase hex of the block's 16-byte id
 *       - `indent_level`— integer
 *       - `text_seq`    — a nested `LoroText` holding the block's whole text
 *         (the LWW `text` register is the legacy fallback for old snapshots)
 */
import { createLoroDoc, importInto } from "./loro-client";
import { noteIdHex } from "./note-id";
import type { LoroDocUpdate } from "./tlr2";
import type { LoroDoc, LoroTreeNode } from "loro-crdt";

/** Base URL for the tesela-server REST/Loro endpoints. Same-origin in the
 *  browser (vite dev proxies `/loro` + `/notes` → 127.0.0.1:7474); the node
 *  convergence check passes an absolute base instead. */
function defaultBase(): string {
  return "";
}

/** One live block read off the doc's `"blocks"` tree. */
export type LiveBlock = {
  /** Dashless lowercase hex block id (server `hex_id` form). */
  bid: string;
  /** Current block text (from `text_seq`, falling back to legacy `text`). */
  text: string;
};

/** Normalize a block id to the dashless lowercase hex the doc stores in
 *  `block_id` meta. Markdown bids are dashed UUIDs (`019e8d0e-1690-…`); the
 *  server writes them dashless via `hex::encode`. Stripping dashes +
 *  lowercasing makes both forms comparable. */
function normalizeBid(bid: string): string {
  return bid.replace(/-/g, "").toLowerCase();
}

/**
 * Manages the Loro doc for a single open note. Construct one per open note;
 * call {@link open} once, feed {@link applyInbound} from the WS binary-delta
 * handler, and {@link close} when navigating away. Re-`open()`ing a different
 * slug on the same instance is supported (it closes the previous doc first).
 */
export class NoteDoc {
  /** Slug of the currently-open note, or null before the first open. */
  slug: string | null = null;
  /** 16-byte doc id (dashless hex) for {@link slug}; used to filter deltas. */
  noteIdHex: string | null = null;

  #doc: LoroDoc | null = null;
  #base: string;
  /** Loro doc-subscription unsubscribe handles + external change callbacks. */
  #docUnsub: (() => void) | null = null;
  #subscribers = new Set<() => void>();
  /** Bumped on every (re)open so a slow in-flight bootstrap for a stale slug
   *  can't clobber a newer open. */
  #generation = 0;

  constructor(base: string = defaultBase()) {
    this.#base = base;
  }

  /** The live `LoroDoc`, or null before {@link open} resolves / after
   *  {@link close}. C2.3 reads block text through the helpers below rather than
   *  poking the doc directly. */
  get doc(): LoroDoc | null {
    return this.#doc;
  }

  /**
   * Open the doc for `slug`: compute its note id, create a fresh `LoroDoc`,
   * wire the change subscription, and bootstrap from the server snapshot. A
   * 404 (no server doc yet) is fine — the doc stays empty and converges once
   * the first delta arrives. Any previous doc is closed first.
   */
  async open(slug: string): Promise<void> {
    this.close();
    const gen = ++this.#generation;
    this.slug = slug;
    this.noteIdHex = noteIdHex(slug);

    const doc = await createLoroDoc();
    // A newer open() (or a close()) raced ahead while wasm/loro initialized —
    // drop this now-stale doc instead of installing it.
    if (gen !== this.#generation) return;
    this.#doc = doc;
    this.#installSubscription(doc);

    await this.#bootstrap(slug, doc, gen);
  }

  /** Fetch + import the server snapshot for `slug`. 404 → leave the doc empty
   *  (no server doc yet). Network/parse failures are swallowed (the doc still
   *  converges from live deltas); they're logged for debuggability. */
  async #bootstrap(slug: string, doc: LoroDoc, gen: number): Promise<void> {
    let bytes: Uint8Array | null = null;
    try {
      const res = await fetch(
        `${this.#base}/loro/notes/${encodeURIComponent(slug)}/snapshot`,
      );
      if (res.status === 404) {
        return; // no server doc yet — empty doc is correct
      }
      if (!res.ok) {
        console.debug(`[note-doc] snapshot ${slug} → HTTP ${res.status}`);
        return;
      }
      bytes = new Uint8Array(await res.arrayBuffer());
    } catch (e) {
      console.debug(`[note-doc] snapshot ${slug} fetch failed`, e);
      return;
    }
    // The bootstrap finished after a newer open()/close() — discard it.
    if (gen !== this.#generation || this.#doc !== doc) return;
    if (bytes.length === 0) return;
    try {
      importInto(doc, bytes);
    } catch (e) {
      console.debug(`[note-doc] snapshot ${slug} import failed`, e);
    }
  }

  /**
   * Apply inbound TLR2 deltas. Each update whose `doc` id matches this note's
   * id is imported into the open doc; updates for other docs are ignored. This
   * is what the WS `onBinaryDelta` handler feeds. No-op if no doc is open.
   */
  applyInbound(updates: LoroDocUpdate[]): void {
    const doc = this.#doc;
    const wantHex = this.noteIdHex;
    if (!doc || !wantHex) return;
    for (const u of updates) {
      if (hexOf(u.doc) !== wantHex) continue;
      try {
        doc.import(u.updateBytes);
      } catch (e) {
        console.debug("[note-doc] inbound import failed", e);
      }
    }
  }

  /**
   * Return the current text of the block whose id === `bid` (dashed or
   * dashless), or null if no live (non-deleted) block matches / no doc is open.
   * Walks the `"blocks"` tree's root children, matching `block_id` meta.
   */
  blockTextByBid(bid: string): string | null {
    const doc = this.#doc;
    if (!doc) return null;
    const want = normalizeBid(bid);
    for (const node of liveRootNodes(doc)) {
      const meta = readNodeMeta(node);
      if (meta && meta.bid === want) {
        return meta.text;
      }
    }
    return null;
  }

  /** All live (non-deleted) root-level blocks in tree order. */
  liveBlocks(): LiveBlock[] {
    const doc = this.#doc;
    if (!doc) return [];
    const out: LiveBlock[] = [];
    for (const node of liveRootNodes(doc)) {
      const meta = readNodeMeta(node);
      if (meta && meta.bid) {
        out.push({ bid: meta.bid, text: meta.text });
      }
    }
    return out;
  }

  /**
   * Register a callback fired whenever the doc changes (local import OR remote
   * delta). C2.3 uses this to reconcile the editor with remote edits. Returns
   * an unsubscribe fn. Safe to call before {@link open}; the registration
   * survives re-opens (the underlying doc subscription is rewired each open).
   */
  subscribe(cb: () => void): () => void {
    this.#subscribers.add(cb);
    return () => this.#subscribers.delete(cb);
  }

  /** Tear down: drop the doc subscription and the doc itself. Idempotent.
   *  External {@link subscribe} callbacks are kept so a subsequent
   *  {@link open} re-attaches them to the new doc. */
  close(): void {
    this.#generation++; // invalidate any in-flight bootstrap
    if (this.#docUnsub) {
      try {
        this.#docUnsub();
      } catch {
        // best-effort unsubscribe
      }
      this.#docUnsub = null;
    }
    this.#doc = null;
    this.slug = null;
    this.noteIdHex = null;
  }

  /** Wire `doc.subscribe(...)` → fan out to external subscribers. The loro
   *  subscription fires after every applied transaction (import included). */
  #installSubscription(doc: LoroDoc): void {
    this.#docUnsub = doc.subscribe(() => {
      for (const cb of this.#subscribers) {
        try {
          cb();
        } catch (e) {
          console.debug("[note-doc] subscriber threw", e);
        }
      }
    });
  }
}

/** Lowercase dashless hex of a 16-byte id (a decoded TLR2 `doc`). */
function hexOf(bytes: Uint8Array): string {
  let s = "";
  for (let i = 0; i < bytes.length; i++) {
    s += bytes[i].toString(16).padStart(2, "0");
  }
  return s;
}

/** Live (non-deleted) root-level children of the `"blocks"` tree. */
function liveRootNodes(doc: LoroDoc): LoroTreeNode[] {
  const tree = doc.getTree("blocks");
  const roots = tree.roots();
  return roots.filter((n) => !n.isDeleted());
}

/** Read `{bid, text}` off a block node's meta map. `node.data.toJSON()`
 *  recursively resolves the nested `text_seq` LoroText to its string and
 *  exposes the scalar `block_id` / legacy `text`. Returns null if there's no
 *  block_id. */
function readNodeMeta(node: LoroTreeNode): { bid: string; text: string } | null {
  const meta = node.data.toJSON() as {
    block_id?: unknown;
    text_seq?: unknown;
    text?: unknown;
  };
  const rawBid = typeof meta.block_id === "string" ? meta.block_id : "";
  if (!rawBid) return null;
  const bid = rawBid.replace(/-/g, "").toLowerCase();
  // Prefer the LoroText sequence (`text_seq`); fall back to the legacy `text`
  // register for snapshots written before the LoroText migration.
  let text = typeof meta.text_seq === "string" ? meta.text_seq : "";
  if (!text && typeof meta.text === "string") text = meta.text;
  return { bid, text };
}
