/**
 * Prism v5 page renderer registry.
 *
 * Phase 3 wires in concrete renderers (note, daily, query, scratch, …) via
 * explicit imports + register() calls in this file. No filesystem-discovery
 * magic — greppable and HMR-safe.
 */

import type { PageRenderer } from "../../buffer/protocol.ts";

const REGISTRY = new Map<string, PageRenderer>();

export function register(name: string, renderer: PageRenderer): void {
  REGISTRY.set(name, renderer);
}

export function get(name: string): PageRenderer | undefined {
  return REGISTRY.get(name);
}

export function getByType(pageType: string): PageRenderer | undefined {
  for (const r of REGISTRY.values()) {
    if (r.acceptsType === pageType) return r;
  }
  return undefined;
}

export function names(): string[] {
  return Array.from(REGISTRY.keys());
}

/** Test seam. */
export function _resetForTests(): void {
  REGISTRY.clear();
}
