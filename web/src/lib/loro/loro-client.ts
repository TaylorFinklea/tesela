/**
 * SSR-safe browser init for the `loro-crdt` WASM peer.
 *
 * The `loro-crdt` package ships a WebAssembly module. We must NOT let it load
 * during SvelteKit SSR (Node would try to bring up the wasm on the server and
 * it has no place in the SSR HTML), so every entry point here is guarded by
 * `browser` and the package is reached ONLY via a dynamic `import()`. There is
 * no top-level `import "loro-crdt"` in this file — the static import below is
 * `import type`, which the compiler erases, so it never pulls the wasm into the
 * SSR bundle.
 *
 * This is the C2.0 de-risk scaffold: it exists to load the peer safely and to
 * import snapshot/delta bytes from the Rust server's per-note Loro doc. The
 * editor binding and WebSocket splice plumbing land in later C2 steps.
 */
import { browser } from "$app/environment";
import type { LoroDoc, LoroText } from "loro-crdt";

/** The lazily-resolved `loro-crdt` module, cached after first browser load. */
type LoroModule = typeof import("loro-crdt");

let modulePromise: Promise<LoroModule> | null = null;

/**
 * Dynamically load `loro-crdt` in the browser only. Throws if called during
 * SSR — callers must guard with `browser` (or only call from client-side
 * lifecycle like `onMount`). The promise is cached so the wasm initializes
 * once per session.
 */
export function loadLoro(): Promise<LoroModule> {
  if (!browser) {
    return Promise.reject(
      new Error("loro-crdt can only be loaded in the browser (SSR guard)"),
    );
  }
  if (!modulePromise) {
    modulePromise = import("loro-crdt").then((m) => {
      loadedModule = m;
      return m;
    });
  }
  return modulePromise;
}

/** Create a fresh, empty `LoroDoc` (browser-only). */
export async function createLoroDoc(): Promise<LoroDoc> {
  const { LoroDoc } = await loadLoro();
  return new LoroDoc();
}

/** The `loro-crdt` module once it has finished loading, else null. Cached on
 *  first successful {@link loadLoro}; lets synchronous code (e.g. a CM6
 *  updateListener that must run in the same tick as the edit) construct a
 *  `LoroText` without re-awaiting. Callers must have already `await`ed a load
 *  (e.g. via {@link createLoroDoc}) — it's null until then. */
let loadedModule: LoroModule | null = null;

/** Construct a detached `LoroText` synchronously. Returns null if the wasm
 *  module hasn't loaded yet (caller must have opened a doc first). Used by
 *  `NoteDoc.spliceBlock` to seed a `text_seq` container via
 *  `getOrCreateContainer`, mirroring the Rust engine's `LoroText::new()`. */
export function newLoroTextSync(): LoroText | null {
  if (!loadedModule) return null;
  return new loadedModule.LoroText();
}

/**
 * Create a `LoroDoc` and import a snapshot/delta produced by the Rust server's
 * `export_doc_update`. The bytes share the Loro 1.x wire format, so the JS peer
 * reconstructs the same `"blocks"` tree the server holds. Returns the populated
 * doc.
 */
export async function importLoroDoc(bytes: Uint8Array): Promise<LoroDoc> {
  const doc = await createLoroDoc();
  doc.import(bytes);
  return doc;
}

/** Import bytes into an existing `LoroDoc` (e.g. an inbound WS delta). */
export function importInto(doc: LoroDoc, bytes: Uint8Array): void {
  doc.import(bytes);
}
