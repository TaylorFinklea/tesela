/**
 * Singleton holder for the TanStack QueryClient.
 *
 * The QueryClient is constructed inside the root `+layout.svelte` (Svelte
 * requires it because it has to live inside `QueryClientProvider`'s
 * context). Plain TS modules outside a component (like `commands.ts`) can
 * still need to invalidate / set cache entries — they grab the instance
 * via `getAppQueryClient()` after the layout has registered it.
 *
 * If the layout hasn't registered yet (very early SSR / first paint),
 * the getter returns `null` and callers should no-op the cache touch.
 */
import type { QueryClient } from "@tanstack/svelte-query";

let instance: QueryClient | null = null;

export function registerAppQueryClient(qc: QueryClient): void {
  instance = qc;
}

export function getAppQueryClient(): QueryClient | null {
  return instance;
}
