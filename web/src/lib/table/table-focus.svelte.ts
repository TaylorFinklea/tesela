/**
 * tesela-ya4.3 — whether a QueryTable currently owns focus.
 *
 * Mirrors `kanban/kanban-focus.svelte.ts`: the table's command-registry
 * entries (`table-commands.ts`) carry a `when` predicate that reads
 * `isTableFocused()`, so they are admitted to the palette (⌘K) + leader
 * chord menu ONLY while a table is focused — keeping them out of the
 * global command surfaces otherwise. The focused table sets this true on
 * mount / when its `focused` prop becomes true and clears it on teardown,
 * mirroring how `kanban-focus.svelte.ts` gates board commands.
 *
 * A `$state` backing field (not a plain `let`) so any reactive consumer of
 * `when` (e.g. a Svelte `$derived` over `available()`) re-runs when table
 * focus changes, instead of needing a palette re-open to pick up the flip.
 */
let focused = $state(false);

export function setTableFocused(value: boolean): void {
  focused = value;
}

export function isTableFocused(): boolean {
  return focused;
}
