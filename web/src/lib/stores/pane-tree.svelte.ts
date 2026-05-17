/**
 * Legacy v4 pane-tree module — gutted in Phase 13b.
 *
 * The full v4 reactive store was replaced by `web/src/lib/buffer/state.svelte.ts`
 * during the v5 cutover. Every live consumer migrated; the only remaining
 * surface is the BlockOutliner's pane-outliner registry (Phase 1.5 legacy),
 * which we keep here as a tiny no-op-friendly map so BlockOutliner can
 * continue to call `registerPaneOutliner` / `unregisterPaneOutliner`
 * without crashing.
 *
 * Phase 13: when BlockOutliner is rewritten to drop this dependency, the
 * whole file can be deleted.
 */

const outlinerEls = new Map<string, HTMLElement>();

export function registerPaneOutliner(paneId: string, el: HTMLElement): void {
  outlinerEls.set(paneId, el);
}
export function unregisterPaneOutliner(paneId: string): void {
  outlinerEls.delete(paneId);
}
export function getPaneOutliner(paneId: string): HTMLElement | undefined {
  return outlinerEls.get(paneId);
}
