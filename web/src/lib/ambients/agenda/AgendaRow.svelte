<script lang="ts">
  import type { AgendaRow } from "$lib/types/AgendaRow";
  import { formatDateMonthDay } from "$lib/date-format";
  import { formatRecurrence } from "$lib/recurrence-format";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";
  import { api } from "$lib/api-client";
  import { getAppQueryClient } from "$lib/app-query-client.svelte";
  import { skipRecurrence } from "$lib/recurrence-actions";
  import DatePicker from "$lib/components/DatePicker.svelte";
  import { toast } from "$lib/stores/toast.svelte";

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

  // ── Mark-done ─────────────────────────────────────────────────────────────

  let markingDone = $state(false);

  async function handleMarkDone(e: MouseEvent) {
    e.stopPropagation();
    if (!row.is_anchor || markingDone) return;
    markingDone = true;
    try {
      await api.setBlockProperty(row.block_id, "status", "done");
      const qc = getAppQueryClient();
      if (qc) await qc.invalidateQueries({ queryKey: ["agenda"] });
    } catch {
      toast("Failed to mark task done", "error");
    } finally {
      markingDone = false;
    }
  }

  // ── Reschedule (date picker) ───────────────────────────────────────────────

  let editingDate = $state(false);
  let pickerPos = $state<{ x: number; y: number }>({ x: 0, y: 0 });

  function openDatePicker(e: MouseEvent) {
    e.stopPropagation();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    pickerPos = { x: rect.left, y: rect.bottom + 4 };
    editingDate = true;
  }

  function closeDatePicker() {
    editingDate = false;
  }

  async function handlePick(
    iso: string,
    _time: string | null,
    recurrence: string | null,
    _field: "deadline" | "scheduled" | null,
  ) {
    closeDatePicker();
    try {
      // Default to "scheduled" for v1 (matches bareDateField setting default).
      await api.setBlockProperty(row.block_id, "scheduled", iso);
      if (recurrence !== null) {
        await api.setBlockProperty(row.block_id, "recurring", recurrence);
      }
      const qc = getAppQueryClient();
      if (qc) await qc.invalidateQueries({ queryKey: ["agenda"] });
    } catch {
      toast("Failed to reschedule", "error");
    }
  }

  // ── Skip ──────────────────────────────────────────────────────────────────

  async function handleSkip(e: MouseEvent) {
    e.stopPropagation();
    await skipRecurrence(row.block_id);
    const qc = getAppQueryClient();
    if (qc) await qc.invalidateQueries({ queryKey: ["agenda"] });
  }
</script>

<div class="flex items-center gap-2 py-0.5 text-[13px]">
  {#if showCheckbox}
    <button
      type="button"
      role="checkbox"
      aria-checked={row.status === "done"}
      class="inline-block w-3.5 h-3.5 border border-muted-foreground/60 rounded-sm cursor-pointer shrink-0 transition-colors hover:border-primary"
      class:opacity-50={markingDone}
      onclick={handleMarkDone}
      title="Mark done"
    ></button>
  {:else if isTask}
    <span class="inline-block w-3.5 h-3.5 shrink-0"></span>
  {:else}
    <span class="text-muted-foreground/50 text-[11px] w-3.5 text-center shrink-0">·</span>
  {/if}

  {#if row.is_anchor}
    <button
      type="button"
      class="shrink-0 transition-colors hover:text-primary"
      class:text-primary={isOverdue}
      class:text-muted-foreground={!isOverdue}
      onclick={openDatePicker}
      title="Reschedule"
    >{icon} {timeOrDate}</button>
  {:else}
    <span
      class="shrink-0"
      class:text-primary={isOverdue}
      class:text-muted-foreground={!isOverdue}
    >{icon} {timeOrDate}</span>
  {/if}

  <span class="text-foreground/90 flex-1 truncate">{row.text}</span>
  <button
    type="button"
    class="text-[11px] text-muted-foreground/60 hover:text-foreground shrink-0 transition-colors"
    onclick={() => gotoNote(row.source_note_id)}
  >in [[{row.source_note_id}]]</button>
  {#if row.recurrence}
    <span class="text-[11px] text-muted-foreground/50 shrink-0">↻ {formatRecurrence(row.recurrence)}</span>
    {#if row.is_anchor}
      <button
        type="button"
        class="text-[11px] text-muted-foreground/60 hover:text-foreground shrink-0 transition-colors"
        onclick={handleSkip}
        title="Skip to next occurrence"
      >⏭</button>
    {/if}
  {/if}
</div>

{#if editingDate}
  <DatePicker
    initialDate={row.occurrence_date}
    initialRecurrence={row.recurrence}
    position={pickerPos}
    onPick={handlePick}
    onClose={closeDatePicker}
  />
{/if}
