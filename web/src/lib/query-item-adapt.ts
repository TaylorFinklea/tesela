/**
 * tesela-ya4.3 — shared adapter from one flat `executeQuery` row
 * (`QueryItem`) to the `ParsedBlock` shape the card/table/move machinery
 * already speaks. Extracted from `KanbanBoard.svelte` (tesela-ya4.1) so
 * `QueryTable.svelte` doesn't duplicate the same mapping (spec decision 2:
 * ONE generalized block source, `executeQuery(dsl)`, for every display
 * mode — kanban and table alike).
 *
 * `QueryItem` has no `bid` — writes fall back to the line-addressed
 * `block_id`, same as QueryWidgetView's list/table rows already did (they
 * never had a bid either). Callers filter out page-kind rows
 * (`block_id === null`) before mapping — both kanban and table only show
 * blocks.
 */
import type { ParsedBlock } from "$lib/types/ParsedBlock";
import type { QueryItem } from "$lib/types/QueryItem";

export function queryItemToParsedBlock(item: QueryItem): ParsedBlock {
  return {
    id: item.block_id as string,
    bid: null,
    text: item.text,
    raw_text: item.text,
    tags: item.primary_tag ? [item.primary_tag] : [],
    inline_tags: [],
    trailing_tags: [],
    inherited_tags: [],
    properties: item.properties,
    indent_level: 0,
    note_id: item.page_id,
    parent_note_type: item.page_note_type,
  };
}
