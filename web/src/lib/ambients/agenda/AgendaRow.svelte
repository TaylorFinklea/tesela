<script lang="ts">
  import type { AgendaRow } from "$lib/types/AgendaRow";
  import { formatDateMonthDay } from "$lib/date-format";
  import { formatRecurrence } from "$lib/recurrence-format";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";

  let { row }: { row: AgendaRow } = $props();

  const isTask = $derived(row.kind === "task");
  const isOverdue = $derived(row.overdue);
  const showCheckbox = $derived(isTask && row.is_anchor);
  // Visual cue:
  //  - overdue tasks: ⚑ in orange (matches --accent-primary / coral brand)
  //  - everything else: 🕒 in muted blue
  // (When the AgendaRow shape grows a `field: "deadline" | "scheduled"`
  //  later, we can render ⚑ for deadline-anchored rows specifically.)
  const icon = $derived(isOverdue && isTask ? "⚑" : "🕒");

  const timeOrDate = $derived(
    row.occurrence_time
      ? row.occurrence_time
      : formatDateMonthDay(row.occurrence_date),
  );
</script>

<div class="flex items-center gap-2 py-0.5 text-[13px]">
  {#if showCheckbox}
    <span
      role="checkbox"
      aria-checked={row.status === "done"}
      tabindex="0"
      class="inline-block w-3.5 h-3.5 border border-muted-foreground/60 rounded-sm cursor-pointer shrink-0"
    ></span>
  {:else if isTask}
    <span class="inline-block w-3.5 h-3.5 shrink-0"></span>
  {:else}
    <span class="text-muted-foreground/50 text-[11px] w-3.5 text-center shrink-0">·</span>
  {/if}
  <span
    class="shrink-0"
    class:text-primary={isOverdue}
    class:text-muted-foreground={!isOverdue}
  >{icon} {timeOrDate}</span>
  <span class="text-foreground/90 flex-1 truncate">{row.text}</span>
  <button
    type="button"
    class="text-[11px] text-muted-foreground/60 hover:text-foreground shrink-0 transition-colors"
    onclick={() => gotoNote(row.source_note_id)}
  >in [[{row.source_note_id}]]</button>
  {#if row.recurrence}
    <span class="text-[11px] text-muted-foreground/50 shrink-0">↻ {formatRecurrence(row.recurrence)}</span>
  {/if}
</div>
