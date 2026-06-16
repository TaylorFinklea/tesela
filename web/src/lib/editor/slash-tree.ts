/**
 * Phase C — pure slash-tree assembler.
 *
 * The slash menu's top level is a pared list:
 *   - the 8 insertion verbs (heading, task, link, tag, date, template,
 *     query, collection) — passed in as `verbLeaves` from
 *     `commandRegistry.availableOn('slash', baseCtx)`
 *   - one `Properties` node (key `p`) whose children are the
 *     context-aware property definitions from `getPropertyChildren()`
 *     (the focused block's tag PropertyDefinitions, or a "Manual key::
 *     value" leaf when the block has no tag properties).
 *
 * No hoisted top-level properties, no `/s` fallback, no `New widget` —
 * widget moved to the leader `new` bucket (Phase B).
 *
 * Pure: takes already-built children, no Svelte, no DOM. Tested in
 * `tests/unit/slash-tree.test.mjs`.
 */

export type SlashTreeNode = {
  key: string;
  label: string;
  action?: () => void;
  children?: SlashTreeNode[];
  hint?: string;
};

export function buildSlashTree(args: {
  verbLeaves: SlashTreeNode[];
  propertyChildren: SlashTreeNode[];
}): SlashTreeNode[] {
  const { verbLeaves, propertyChildren } = args;
  return [
    ...verbLeaves,
    {
      key: "p",
      label: "Properties",
      children: propertyChildren,
    },
  ];
}
