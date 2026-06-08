<script lang="ts">
  /**
   * Task 5: Properties row — display-only strip rendered beneath a task block.
   * Shows scheduled, deadline, and recurring properties in a compact muted row.
   * Click on a date navigates to that day's daily page via gotoNote.
   * Click on the skip button calls skipRecurrence for recurring tasks.
   *
   * Task 6: Fields are click-to-edit. Clicking a scheduled/deadline value opens
   * a DatePicker pre-filled with the current date. Clicking the recurring label/
   * value opens a DatePicker pre-filled with the current recurrence. On commit
   * the property is upserted into the block raw text and saved via onUpdate.
   */
  import type { ParsedBlock } from "$lib/types/ParsedBlock";
  import { formatDateMonthDay } from "$lib/date-format";
  import { formatRecurrence } from "$lib/recurrence-format";
  import { skipRecurrence } from "$lib/recurrence-actions";
  import { gotoNote } from "$lib/stores/active-pane-nav.svelte";
  import { upsertBlockProperty } from "$lib/block-tags";
  import { priorityFlag } from "$lib/priority";
  import DatePicker from "./DatePicker.svelte";

  let {
    block,
    onUpdate,
  }: {
    block: ParsedBlock;
    /** Called with the new raw block text after a date edit. Routes to
     *  BlockOutliner's handleBlockChange so the edit persists normally. */
    onUpdate?: (newText: string) => void;
  } = $props();

  const priority = $derived((block.properties.priority ?? "").trim());
  const pflag = $derived(priorityFlag(priority));
  const scheduled = $derived((block.properties.scheduled ?? "").trim());
  const deadline = $derived((block.properties.deadline ?? "").trim());
  const recurring = $derived((block.properties.recurring ?? "").trim());

  const hasAny = $derived(!!pflag || !!scheduled || !!deadline || !!recurring);

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

  // ── Click-to-edit state ────────────────────────────────────────────────────

  /** Which field is currently being edited: "scheduled" | "deadline" | "recurring" | null */
  let editingField = $state<"scheduled" | "deadline" | "recurring" | null>(null);
  let pickerPos = $state<{ x: number; y: number }>({ x: 0, y: 0 });

  function openPicker(field: "scheduled" | "deadline" | "recurring", e: MouseEvent) {
    e.stopPropagation();
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    pickerPos = { x: rect.left, y: rect.bottom + 4 };
    editingField = field;
  }

  function closePicker() {
    editingField = null;
  }

  /** ISO date string currently stored for the field being edited, or today. */
  const pickerInitialDate = $derived.by(() => {
    if (editingField === "scheduled") return toIso(scheduled) || undefined;
    if (editingField === "deadline") return toIso(deadline) || undefined;
    return undefined;
  });

  /** Existing recurrence to pre-fill when editing the "recurring" field. */
  const pickerInitialRecurrence = $derived.by(() => {
    return editingField === "recurring" ? (recurring || null) : null;
  });

  function handlePick(
    iso: string,
    _time: string | null,
    recurrence: string | null,
    _field: "deadline" | "scheduled" | null,
  ) {
    if (!editingField) return;
    let newText = block.raw_text;
    if (editingField === "recurring") {
      if (recurrence) {
        newText = upsertBlockProperty(newText, "recurring", recurrence);
      }
    } else {
      newText = upsertBlockProperty(newText, editingField, iso);
      // If the picker also carried a recurrence choice, persist that too.
      if (recurrence) {
        newText = upsertBlockProperty(newText, "recurring", recurrence);
      }
    }
    closePicker();
    onUpdate?.(newText);
  }
</script>

{#if hasAny}
  <div class="flex items-center gap-3 px-1 pb-1 text-[11px] text-muted-foreground/70">
    {#if pflag}
      <span
        class="inline-flex items-center gap-1 font-semibold leading-none"
        style="color: {pflag.color}"
        title="Priority {pflag.label}"
      >
        <span aria-hidden="true">&#9873;</span>{pflag.label}
      </span>
    {/if}

    {#if scheduled}
      <span class="inline-flex items-center gap-1">
        <span class="text-muted-foreground/40 font-medium uppercase tracking-wide text-[9px]">Scheduled</span>
        <!-- Edit button opens the date picker pre-filled with current date -->
        <button
          class="text-muted-foreground/70 hover:text-primary transition-colors"
          onclick={(e) => openPicker("scheduled", e)}
          title="Edit scheduled date"
        >{formatDateMonthDay(scheduled)}</button>
        <!-- Navigate-to-date is now a secondary icon to avoid clobbering the edit tap -->
        <button
          class="text-muted-foreground/30 hover:text-muted-foreground/60 transition-colors leading-none"
          onclick={(e) => navigateToDate(scheduled, e)}
          title="Go to {toIso(scheduled)}"
        >↗</button>
      </span>
    {/if}

    {#if deadline}
      <span class="inline-flex items-center gap-1">
        <span class="text-muted-foreground/40 font-medium uppercase tracking-wide text-[9px]">Deadline</span>
        <button
          class="text-muted-foreground/70 hover:text-primary transition-colors"
          onclick={(e) => openPicker("deadline", e)}
          title="Edit deadline date"
        >{formatDateMonthDay(deadline)}</button>
        <button
          class="text-muted-foreground/30 hover:text-muted-foreground/60 transition-colors leading-none"
          onclick={(e) => navigateToDate(deadline, e)}
          title="Go to {toIso(deadline)}"
        >↗</button>
      </span>
    {/if}

    {#if recurring}
      <span class="inline-flex items-center gap-1">
        <span class="text-muted-foreground/40 font-medium uppercase tracking-wide text-[9px]">Repeat</span>
        <button
          class="text-muted-foreground/70 hover:text-primary transition-colors"
          onclick={(e) => openPicker("recurring", e)}
          title="Edit recurrence"
        >{formatRecurrence(recurring)}</button>
        <button
          class="text-muted-foreground/30 hover:text-muted-foreground/70 transition-colors leading-none"
          onclick={handleSkip}
          title="Skip to next occurrence"
        >⏭</button>
      </span>
    {/if}
  </div>
{/if}

{#if editingField}
  <DatePicker
    initialDate={pickerInitialDate}
    initialRecurrence={pickerInitialRecurrence}
    position={pickerPos}
    onPick={handlePick}
    onClose={closePicker}
  />
{/if}
