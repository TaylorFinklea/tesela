<script lang="ts">
  import type { AgendaRow as AgendaRowT } from "$lib/types/AgendaRow";
  import AgendaRow from "./AgendaRow.svelte";

  let {
    label,
    rows,
    emphasis = "normal",
  }: {
    /** Day header text — `Today · Friday, May 22` / `Mon May 25` / `Overdue`. */
    label: string;
    rows: AgendaRowT[];
    /** `overdue` tints the header; `empty` shows the empty-day placeholder. */
    emphasis?: "normal" | "overdue" | "empty";
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
      <AgendaRow {row} />
    {/each}
  {/if}
</div>
