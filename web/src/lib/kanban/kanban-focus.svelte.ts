/**
 * tesela-ya4.2 — whether a KanbanBoard currently owns focus.
 *
 * The board's command-registry entries (`kanban-commands.ts`) carry a `when`
 * predicate that reads `isKanbanFocused()`, so they are admitted to the
 * palette (⌘K) + leader chord menu ONLY while a board is focused — keeping
 * them out of the global command surfaces otherwise. The focused board sets
 * this true on mount / when its `focused` prop becomes true and clears it on
 * teardown, mirroring how `focused-editor.svelte.ts` gates editor commands.
 *
 * A `$state` backing field (not a plain `let`) so any reactive consumer of
 * `when` (e.g. a Svelte `$derived` over `available()`) re-runs when board
 * focus changes, instead of needing a palette re-open to pick up the flip.
 */
let focused = $state(false);

export function setKanbanFocused(value: boolean): void {
  focused = value;
}

export function isKanbanFocused(): boolean {
  return focused;
}
