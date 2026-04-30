<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { goto } from "$app/navigation";
  import { api, ApiError } from "$lib/api-client";

  // Visible month — defaults to today's. Phase 9.4 adds keyboard nav.
  const now = new Date();
  let visibleYear = $state(now.getFullYear());
  let visibleMonth = $state(now.getMonth()); // 0-indexed
  let selected = $state<string | null>(null);
  let rootEl = $state<HTMLElement | undefined>();
  // `g`-prefix chord state for vim-style `g t` (jump to today).
  let gPending = $state(false);
  let gTimer: ReturnType<typeof setTimeout> | null = null;

  function pad2(n: number): string {
    return n < 10 ? `0${n}` : `${n}`;
  }
  function isoOf(y: number, m: number, d: number): string {
    return `${y}-${pad2(m + 1)}-${pad2(d)}`;
  }
  function todayIso(): string {
    return isoOf(now.getFullYear(), now.getMonth(), now.getDate());
  }

  const fromDate = $derived(isoOf(visibleYear, visibleMonth, 1));
  const toDate = $derived.by(() => {
    const last = new Date(visibleYear, visibleMonth + 1, 0).getDate();
    return isoOf(visibleYear, visibleMonth, last);
  });

  const marksQuery = createQuery(() => ({
    queryKey: ["calendar-marks", fromDate, toDate] as const,
    queryFn: () => api.getCalendarMarks(fromDate, toDate),
  }));
  const marks = $derived(marksQuery.data?.days ?? {});

  const monthName = $derived.by(() => {
    return new Date(visibleYear, visibleMonth, 1).toLocaleString("en-US", { month: "long" });
  });

  // Build the calendar grid: 6 rows × 7 columns, starting Sunday.
  type Cell = {
    iso: string;
    day: number;
    muted: boolean; // belongs to prev/next month
  };
  const cells = $derived.by((): Cell[] => {
    const first = new Date(visibleYear, visibleMonth, 1);
    const dow = first.getDay(); // 0 = Sunday
    const daysInMonth = new Date(visibleYear, visibleMonth + 1, 0).getDate();
    const out: Cell[] = [];
    // Leading days from previous month
    for (let i = dow; i > 0; i--) {
      const d = new Date(visibleYear, visibleMonth, 1 - i);
      out.push({ iso: isoOf(d.getFullYear(), d.getMonth(), d.getDate()), day: d.getDate(), muted: true });
    }
    for (let d = 1; d <= daysInMonth; d++) {
      out.push({ iso: isoOf(visibleYear, visibleMonth, d), day: d, muted: false });
    }
    // Trailing to fill 6 rows (42 cells total)
    while (out.length < 42) {
      const i = out.length - dow - daysInMonth + 1;
      const d = new Date(visibleYear, visibleMonth + 1, i);
      out.push({ iso: isoOf(d.getFullYear(), d.getMonth(), d.getDate()), day: d.getDate(), muted: true });
    }
    return out;
  });

  const queryClient = useQueryClient();

  async function clickDay(iso: string) {
    selected = iso;
    try {
      const note = await api.getDailyNote(iso);
      queryClient.invalidateQueries({ queryKey: ["notes"] });
      queryClient.invalidateQueries({ queryKey: ["calendar-marks"] });
      goto(`/p/${encodeURIComponent(note.id)}`);
    } catch (e) {
      if (e instanceof ApiError) {
        console.error("Failed to load/create daily note:", e);
      } else {
        throw e;
      }
    }
  }

  function prevMonth() {
    if (visibleMonth === 0) {
      visibleYear -= 1;
      visibleMonth = 11;
    } else {
      visibleMonth -= 1;
    }
  }
  function nextMonth() {
    if (visibleMonth === 11) {
      visibleYear += 1;
      visibleMonth = 0;
    } else {
      visibleMonth += 1;
    }
  }

  const today = todayIso();

  // ----- Phase 9.4: keyboard navigation -----

  /** Move `selected` by `deltaDays` (signed). If selection crosses the month
   *  boundary, advance/retreat the visible month so the new selection stays
   *  in view. */
  function moveSelection(deltaDays: number) {
    const base = selected ?? today;
    const [y, m, d] = base.split("-").map(Number);
    const next = new Date(y, m - 1, d + deltaDays);
    selected = isoOf(next.getFullYear(), next.getMonth(), next.getDate());
    // Sync visible month if selection moved out of view.
    if (next.getFullYear() !== visibleYear || next.getMonth() !== visibleMonth) {
      visibleYear = next.getFullYear();
      visibleMonth = next.getMonth();
    }
  }

  function clearGPending() {
    gPending = false;
    if (gTimer) {
      clearTimeout(gTimer);
      gTimer = null;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    // Don't intercept when a child input/textarea is focused (we don't have any
    // today, but defensive).
    const target = e.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA") return;

    // `g t` chord — jump to today.
    if (gPending) {
      if (e.key === "t") {
        e.preventDefault();
        clearGPending();
        selected = today;
        const td = new Date(today);
        visibleYear = td.getFullYear();
        visibleMonth = td.getMonth();
        return;
      }
      // Any other key cancels the chord.
      clearGPending();
    }

    if (e.key === "g") {
      e.preventDefault();
      gPending = true;
      if (gTimer) clearTimeout(gTimer);
      gTimer = setTimeout(clearGPending, 1500);
      return;
    }

    switch (e.key) {
      case "ArrowLeft":
      case "h":
        e.preventDefault();
        moveSelection(-1);
        break;
      case "ArrowRight":
      case "l":
        e.preventDefault();
        moveSelection(1);
        break;
      case "ArrowUp":
      case "k":
        e.preventDefault();
        moveSelection(-7);
        break;
      case "ArrowDown":
      case "j":
        e.preventDefault();
        moveSelection(7);
        break;
      case "PageUp":
        e.preventDefault();
        prevMonth();
        break;
      case "PageDown":
        e.preventDefault();
        nextMonth();
        break;
      case "Enter":
        e.preventDefault();
        if (selected) clickDay(selected);
        break;
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={rootEl}
  class="v9-cal"
  tabindex="0"
  onkeydown={handleKeydown}
  style="outline: none;"
>
  <div class="cal-head">
    <span class="month">{monthName} {visibleYear}</span>
    <span class="nav">
      <span role="button" tabindex="-1" onclick={prevMonth} onkeydown={(e) => { if (e.key === "Enter") prevMonth(); }}>‹</span>
      <span role="button" tabindex="-1" onclick={nextMonth} onkeydown={(e) => { if (e.key === "Enter") nextMonth(); }}>›</span>
    </span>
  </div>
  <div class="cal-grid">
    {#each ["S", "M", "T", "W", "T", "F", "S"] as d}
      <span class="dow">{d}</span>
    {/each}
    {#each cells as cell}
      {@const m = marks[cell.iso]}
      {@const isToday = cell.iso === today}
      {@const isSel = cell.iso === selected}
      <span
        class="day {cell.muted ? 'muted' : ''} {isToday ? 'today' : ''} {isSel ? 'selected' : ''}"
        role="button"
        tabindex="-1"
        onclick={() => clickDay(cell.iso)}
        onkeydown={(e) => { if (e.key === "Enter") clickDay(cell.iso); }}
      >
        {cell.day}
        {#if m && (m.tasks > 0 || m.events > 0 || m.notes)}
          <span class="marks">
            {#if m.tasks > 0}<i></i>{/if}
            {#if m.events > 0}<i class="event"></i>{/if}
            {#if m.notes}<i class="note"></i>{/if}
          </span>
        {/if}
      </span>
    {/each}
  </div>
  <div class="cal-foot">
    <span class="lg task"><i></i>task</span>
    <span class="lg event"><i></i>event</span>
    <span class="lg note"><i></i>note</span>
  </div>
</div>
