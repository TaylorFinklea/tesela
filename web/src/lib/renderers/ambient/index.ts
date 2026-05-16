/**
 * Prism v5 ambient renderer registry.
 *
 * Ambient buffers have no reference (workspace singletons); the registry is
 * a thin name → renderer map. Workspace-level state for each ambient lives
 * in `web/src/lib/ambients/<name>/state.svelte.ts` and is consulted by the
 * renderer at render time.
 */

import type { AmbientRenderer } from "../../buffer/protocol.ts";

const REGISTRY = new Map<string, AmbientRenderer>();

export function register(name: string, renderer: AmbientRenderer): void {
  REGISTRY.set(name, renderer);
}

export function get(name: string): AmbientRenderer | undefined {
  return REGISTRY.get(name);
}

export function names(): string[] {
  return Array.from(REGISTRY.keys());
}

/** Test seam. */
export function _resetForTests(): void {
  REGISTRY.clear();
}
