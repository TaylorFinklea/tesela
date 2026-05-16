<script lang="ts">
  /* Ambient calendar buffer. Phase 5 ships a simple month grid + selected
   * date display, backed by workspace-level state so two mounts in
   * different tabs share the selection. */
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import {
    getSelectedDate,
    getViewMonth,
    setSelectedDate,
    setViewMonth,
  } from "./state.svelte";

  let { onNavigate }: AmbientRendererProps = $props();

  const month = $derived(getViewMonth());
  const selected = $derived(getSelectedDate());

  function monthDate(m: string): Date {
    const [y, mm] = m.split("-").map(Number);
    return new Date(y, mm - 1, 1);
  }

  function shiftMonth(by: number): void {
    const d = monthDate(month);
    d.setMonth(d.getMonth() + by);
    setViewMonth(
      `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`,
    );
  }

  const days = $derived.by(() => {
    const d = monthDate(month);
    const y = d.getFullYear();
    const mIdx = d.getMonth();
    const first = new Date(y, mIdx, 1);
    const padStart = (first.getDay() + 6) % 7; // Monday-first
    const daysInMonth = new Date(y, mIdx + 1, 0).getDate();
    const cells: { date: string; day: number; inMonth: boolean }[] = [];
    for (let i = 0; i < padStart; i++) cells.push({ date: "", day: 0, inMonth: false });
    for (let i = 1; i <= daysInMonth; i++) {
      const date = `${y}-${String(mIdx + 1).padStart(2, "0")}-${String(i).padStart(2, "0")}`;
      cells.push({ date, day: i, inMonth: true });
    }
    while (cells.length % 7 !== 0)
      cells.push({ date: "", day: 0, inMonth: false });
    return cells;
  });

  function pick(date: string) {
    if (!date) return;
    setSelectedDate(date);
    onNavigate({ kind: "open-page", path: date, how: "replace" });
  }
</script>

<div class="v5-calendar">
  <header>
    <button type="button" onclick={() => shiftMonth(-1)} title="prev month"
      >‹</button
    >
    <span class="v5-cal-month">{month}</span>
    <button type="button" onclick={() => shiftMonth(1)} title="next month"
      >›</button
    >
  </header>
  <div class="v5-cal-grid v5-cal-grid-head">
    {#each ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] as h}
      <span class="v5-cal-hd">{h}</span>
    {/each}
  </div>
  <div class="v5-cal-grid">
    {#each days as cell, i (i)}
      {#if cell.inMonth}
        <button
          type="button"
          class="v5-cal-cell"
          class:selected={cell.date === selected}
          onclick={() => pick(cell.date)}
        >
          {cell.day}
        </button>
      {:else}
        <span class="v5-cal-cell empty"></span>
      {/if}
    {/each}
  </div>
  <footer>
    selected:
    <button type="button" onclick={() => pick(selected)}>{selected}</button>
  </footer>
</div>

<style>
  .v5-calendar {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 12px 14px;
    font-family: var(--v4-mono);
    font-size: 12px;
    color: var(--v4-ink2);
    gap: 10px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  header button {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink2);
    border-radius: 5px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .v5-cal-month {
    color: var(--v4-ink);
    font-size: 13px;
  }
  .v5-cal-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    gap: 3px;
  }
  .v5-cal-grid-head .v5-cal-hd {
    text-align: center;
    color: var(--v4-ink5);
    font-size: 10.5px;
  }
  .v5-cal-cell {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink2);
    border-radius: 4px;
    padding: 6px 0;
    text-align: center;
    cursor: pointer;
    font-family: var(--v4-mono);
    font-size: 11px;
  }
  .v5-cal-cell:hover {
    border-color: var(--v4-hair2);
    color: var(--v4-ink);
  }
  .v5-cal-cell.selected {
    border-color: var(--v4-accent);
    color: var(--v4-ink);
    background: color-mix(in srgb, var(--v4-accent) 8%, transparent);
  }
  .v5-cal-cell.empty {
    visibility: hidden;
  }
  footer {
    margin-top: auto;
    color: var(--v4-ink5);
    font-size: 11px;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  footer button {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink2);
    border-radius: 4px;
    padding: 2px 8px;
    cursor: pointer;
    font-family: var(--v4-mono);
  }
</style>
