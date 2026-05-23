<script lang="ts">
  import type { AgendaRow as AgendaRowT } from "$lib/types/AgendaRow";
  import AgendaRow from "./AgendaRow.svelte";

  let {
    label,
    rows,
    emphasis = "normal",
    selectedKey = null,
  }: {
    /** Day header text — `Today · Friday, May 22` / `Mon May 25` / `Overdue`. */
    label: string;
    rows: AgendaRowT[];
    /** `overdue` tints the header; `empty` shows the empty-day placeholder. */
    emphasis?: "normal" | "overdue" | "empty";
    /** Key of the currently keyboard-focused row, in the form
     * `block_id:occurrence_date` (matches what AgendaRow renders into
     * its `data-agenda-row` attribute). Used to draw the focus ring on
     * the right row when the user navigates with j/k. */
    selectedKey?: string | null;
  } = $props();

  const headerColor = $derived(
    emphasis === "overdue"
      ? "text-primary"
      : emphasis === "empty"
        ? "text-muted-foreground/40"
        : "text-muted-foreground/70",
  );
</script>

<div class="mb-3">
  <div class="text-[11px] font-semibold tracking-wide uppercase mb-1 {headerColor}"
  >{label}</div>
  {#if emphasis !== "empty"}
    {#each rows as row (row.block_id + ":" + row.occurrence_date)}
      <AgendaRow
        {row}
        selected={selectedKey === row.block_id + ":" + row.occurrence_date}
      />
    {/each}
  {/if}
</div>
