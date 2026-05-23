<script lang="ts">
  import type { QueryItem } from "$lib/types/QueryItem";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";

  let {
    row,
    selected = false,
  }: {
    row: QueryItem;
    /** Whether this row holds keyboard focus. The parent (`Inbox`)
     * computes selection over the flat row list and passes it down
     * here purely for the visual focus ring; keyboard actions are
     * dispatched by the parent via `data-action` selectors on the
     * row's buttons, so mouse and keyboard paths share one handler. */
    selected?: boolean;
  } = $props();

  const primaryTag = $derived(row.primary_tag);
  const breadcrumb = $derived(
    // Source page title — preferred over the raw page id since the
    // Inbox is meant to read like a stack of triage tasks, not slugs.
    row.title || row.page_id,
  );
</script>

<div
  class="flex items-center gap-2 py-1 px-1 -mx-1 text-[13px] rounded"
  class:bg-accent={selected}
  data-inbox-row={row.block_id ?? row.page_id}
>
  <!-- Open/triage trigger lives on the text span so Enter routes
       through it. Mouse users click the source pill on the right. -->
  <button
    type="button"
    class="flex-1 min-w-0 text-left truncate text-foreground/90 hover:text-foreground transition-colors"
    onclick={() => gotoNote(row.page_id, row.block_id ?? null)}
    data-action="open-source"
  >{row.text || "(empty block)"}</button>

  {#if primaryTag}
    <span class="text-[11px] text-muted-foreground/70 shrink-0">#{primaryTag}</span>
  {/if}

  <button
    type="button"
    class="text-[11px] text-muted-foreground/60 hover:text-foreground shrink-0 transition-colors max-w-[40%] truncate"
    onclick={() => gotoNote(row.page_id, row.block_id ?? null)}
    data-action="open-source-pill"
    title="Open source"
  >in [[{breadcrumb}]]</button>
</div>
