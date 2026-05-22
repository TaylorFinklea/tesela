<script lang="ts">
  /**
   * Task 5: Properties row — display-only strip rendered beneath a task block.
   * Shows scheduled, deadline, and recurring properties in a compact muted row.
   * Click on a date navigates to that day's daily page via gotoNote.
   * Click on the skip button calls skipRecurrence for recurring tasks.
   *
   * Next task makes these fields click-to-edit.
   */
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import { formatDateMonthDay } from "$lib/date-format";
  import { formatRecurrence } from "$lib/recurrence-format";
  import { skipRecurrence } from "$lib/recurrence-actions";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";

  let { block }: { block: ParsedBlock } = $props();

  const scheduled = $derived((block.properties.scheduled ?? "").trim());
  const deadline = $derived((block.properties.deadline ?? "").trim());
  const recurring = $derived((block.properties.recurring ?? "").trim());

  const hasAny = $derived(!!scheduled || !!deadline || !!recurring);

  /** Strip [[YYYY-MM-DD]] brackets to get a bare ISO date for routing. */
  function toIso(raw: string): string {
    return raw.replace(/^\[\[|\]\]$/g, "").trim();
  }

  function navigateToDate(raw: string, e: MouseEvent) {
    e.stopPropagation();
    const iso = toIso(raw);
    if (iso) gotoNote(iso);
  }

  function handleSkip(e: MouseEvent) {
    e.stopPropagation();
    skipRecurrence(block.id);
  }
</script>

{#if hasAny}
  <div class="flex items-center gap-3 px-1 pb-1 text-[11px] text-muted-foreground/70">
    {#if scheduled}
      <span class="inline-flex items-center gap-1">
        <span class="text-muted-foreground/40 font-medium uppercase tracking-wide text-[9px]">Scheduled</span>
        <button
          class="text-muted-foreground/70 hover:text-primary transition-colors"
          onclick={(e) => navigateToDate(scheduled, e)}
          title="Go to {toIso(scheduled)}"
        >{formatDateMonthDay(scheduled)}</button>
      </span>
    {/if}

    {#if deadline}
      <span class="inline-flex items-center gap-1">
        <span class="text-muted-foreground/40 font-medium uppercase tracking-wide text-[9px]">Deadline</span>
        <button
          class="text-muted-foreground/70 hover:text-primary transition-colors"
          onclick={(e) => navigateToDate(deadline, e)}
          title="Go to {toIso(deadline)}"
        >{formatDateMonthDay(deadline)}</button>
      </span>
    {/if}

    {#if recurring}
      <span class="inline-flex items-center gap-1">
        <span class="text-muted-foreground/40 font-medium uppercase tracking-wide text-[9px]">Repeat</span>
        <span class="text-muted-foreground/70">{formatRecurrence(recurring)}</span>
        <button
          class="text-muted-foreground/30 hover:text-muted-foreground/70 transition-colors leading-none"
          onclick={handleSkip}
          title="Skip to next occurrence"
        >⏭</button>
      </span>
    {/if}
  </div>
{/if}
