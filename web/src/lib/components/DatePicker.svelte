<script lang="ts">
  import { onMount } from "svelte";

  let {
    initialDate,
    position,
    onPick,
    onClose,
  }: {
    /** ISO date string `YYYY-MM-DD`. Defaults to today. */
    initialDate?: string;
    position: { x: number; y: number };
    onPick: (iso: string) => void;
    onClose: () => void;
  } = $props();

  function parseISO(s: string | undefined): Date {
    if (s) {
      const [y, m, d] = s.split("-").map(Number);
      if (!Number.isNaN(y + m + d)) return new Date(y, m - 1, d);
    }
    return new Date();
  }
  function fmt(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${dd}`;
  }

  let selected = $state<Date>(parseISO(initialDate));
  // The month being viewed (may differ from `selected` after navigating).
  let viewMonth = $state<Date>(new Date(selected.getFullYear(), selected.getMonth(), 1));

  const today = $derived(fmt(new Date()));

  // Build the day grid for the current view month, padded with adjacent-month
  // days so each row is a full week (Monday-first).
  const grid = $derived.by(() => {
    const first = new Date(viewMonth.getFullYear(), viewMonth.getMonth(), 1);
    const lastDay = new Date(viewMonth.getFullYear(), viewMonth.getMonth() + 1, 0).getDate();
    // JS Sunday=0; we want Monday=0 → shift by 1.
    const offset = (first.getDay() + 6) % 7;
    const days: { date: Date; inMonth: boolean }[] = [];
    // Leading days from previous month
    for (let i = offset; i > 0; i--) {
      const d = new Date(first);
      d.setDate(d.getDate() - i);
      days.push({ date: d, inMonth: false });
    }
    for (let d = 1; d <= lastDay; d++) {
      days.push({ date: new Date(viewMonth.getFullYear(), viewMonth.getMonth(), d), inMonth: true });
    }
    // Trailing days to fill last week
    while (days.length % 7 !== 0) {
      const last = days[days.length - 1].date;
      const d = new Date(last);
      d.setDate(d.getDate() + 1);
      days.push({ date: d, inMonth: false });
    }
    // Group into 7-day rows
    const rows: { date: Date; inMonth: boolean }[][] = [];
    for (let i = 0; i < days.length; i += 7) rows.push(days.slice(i, i + 7));
    return rows;
  });

  const monthLabel = $derived(
    viewMonth.toLocaleDateString(undefined, { month: "long", year: "numeric" }),
  );

  function move(days: number) {
    const next = new Date(selected);
    next.setDate(next.getDate() + days);
    selected = next;
    viewMonth = new Date(next.getFullYear(), next.getMonth(), 1);
  }
  function prevMonth() {
    viewMonth = new Date(viewMonth.getFullYear(), viewMonth.getMonth() - 1, 1);
  }
  function nextMonth() {
    viewMonth = new Date(viewMonth.getFullYear(), viewMonth.getMonth() + 1, 1);
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "ArrowLeft") { e.preventDefault(); move(e.shiftKey ? -7 : -1); }
    else if (e.key === "ArrowRight") { e.preventDefault(); move(e.shiftKey ? 7 : 1); }
    else if (e.key === "ArrowUp") { e.preventDefault(); move(-7); }
    else if (e.key === "ArrowDown") { e.preventDefault(); move(7); }
    else if (e.key === "Enter") { e.preventDefault(); onPick(fmt(selected)); }
    else if (e.key === "Escape") { e.preventDefault(); onClose(); }
    else if (e.key === "t" || e.key === "T") { e.preventDefault(); selected = new Date(); viewMonth = new Date(selected.getFullYear(), selected.getMonth(), 1); }
  }

  let containerEl = $state<HTMLDivElement | null>(null);
  onMount(() => {
    containerEl?.focus();
  });

  // Re-focus the container if focus drifts (e.g., user clicked a day cell).
  function refocus() {
    queueMicrotask(() => containerEl?.focus());
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={containerEl}
  tabindex="0"
  onkeydown={handleKey}
  onblur={onClose}
  role="dialog"
  aria-label="Date picker"
  class="fixed z-50 bg-popover border border-border rounded-md shadow-xl p-2 outline-none"
  style="left: {position.x}px; top: {position.y}px;"
>
  <!-- Header: month nav -->
  <div class="flex items-center justify-between mb-2 px-1">
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="text-[12px] px-1.5 text-muted-foreground/60 hover:text-foreground/80 rounded hover:bg-muted/40"
      onclick={() => { prevMonth(); refocus(); }}
      title="Previous month"
    >‹</button>
    <span class="text-[12px] font-medium text-foreground/90">{monthLabel}</span>
    <!-- svelte-ignore a11y_consider_explicit_label -->
    <button
      class="text-[12px] px-1.5 text-muted-foreground/60 hover:text-foreground/80 rounded hover:bg-muted/40"
      onclick={() => { nextMonth(); refocus(); }}
      title="Next month"
    >›</button>
  </div>

  <!-- Day-of-week header -->
  <div class="grid grid-cols-7 text-[10px] text-muted-foreground/50 uppercase tracking-wider mb-1">
    <span class="text-center">Mo</span>
    <span class="text-center">Tu</span>
    <span class="text-center">We</span>
    <span class="text-center">Th</span>
    <span class="text-center">Fr</span>
    <span class="text-center">Sa</span>
    <span class="text-center">Su</span>
  </div>

  <!-- Day grid -->
  <div class="grid grid-cols-7 gap-px text-[12px]">
    {#each grid as row}
      {#each row as cell}
        {@const iso = fmt(cell.date)}
        {@const isSelected = iso === fmt(selected)}
        {@const isToday = iso === today}
        <!-- svelte-ignore a11y_consider_explicit_label -->
        <button
          class="
            w-7 h-7 rounded text-center transition-colors
            {isSelected ? 'bg-primary text-primary-foreground' : ''}
            {!isSelected && isToday ? 'ring-1 ring-primary/40' : ''}
            {!isSelected && cell.inMonth ? 'text-foreground/85 hover:bg-muted/40' : ''}
            {!cell.inMonth ? 'text-muted-foreground/30 hover:bg-muted/30' : ''}
          "
          onclick={() => { selected = cell.date; viewMonth = new Date(cell.date.getFullYear(), cell.date.getMonth(), 1); onPick(iso); }}
          title={iso}
        >{cell.date.getDate()}</button>
      {/each}
    {/each}
  </div>

  <!-- Footer hint -->
  <div class="text-[10px] text-muted-foreground/40 mt-1.5 px-1 text-center">
    arrows · enter · t = today · esc
  </div>
</div>
