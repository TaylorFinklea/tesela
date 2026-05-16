/**
 * Prism v5 derived renderer registry.
 *
 * Derived renderers are pure functions of a Reference. The registry's mount
 * guard refuses to mount a renderer with a non-matching reference kind —
 * type-level discipline at the source (a renderer for `accepts: "page"` can
 * only be registered with a page-shaped DerivedRenderer), plus a runtime
 * check at mount time that catches state loaded from disk where shapes
 * have drifted.
 */

import type { DerivedRenderer } from "../../buffer/protocol.ts";
import { RendererReferenceMismatch } from "../../buffer/protocol.ts";
import type { Reference, ReferenceKind } from "../../buffer/types.ts";

const REGISTRY = new Map<string, DerivedRenderer<ReferenceKind>>();

export function register<K extends ReferenceKind>(
  name: string,
  renderer: DerivedRenderer<K>,
): void {
  // Upcast at the registry boundary; the mount guard re-narrows on lookup.
  // The two-step cast is required because `DerivedRenderer<K>` is invariant
  // in its component prop type.
  REGISTRY.set(name, renderer as unknown as DerivedRenderer<ReferenceKind>);
}

export function get(name: string): DerivedRenderer<ReferenceKind> | undefined {
  return REGISTRY.get(name);
}

export function names(): string[] {
  return Array.from(REGISTRY.keys());
}

/**
 * Look up a derived renderer and validate the reference against its declared
 * `accepts` kind. Returns the renderer on success; throws on mismatch or on
 * unknown name.
 */
export function mount(
  name: string,
  ref: Reference,
): DerivedRenderer<ReferenceKind> {
  const r = REGISTRY.get(name);
  if (!r) {
    throw new Error(`Derived renderer "${name}" is not registered.`);
  }
  if (r.accepts !== ref.kind) {
    throw new RendererReferenceMismatch(name, r.accepts, ref.kind);
  }
  return r;
}

/** Test seam. */
export function _resetForTests(): void {
  REGISTRY.clear();
}
