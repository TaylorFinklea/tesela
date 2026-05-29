<!-- web/src/lib/graphite/views/GrAgenda.svelte — Part A, Task A5.
     Graphite agenda week. NEW presentation over api.getAgenda — a 5-day
     time grid (Mon–Fri) with a 56px hour gutter, GR_HOURS 8..16, GR_SLOT
     62px rows, GR_START 8. Type-colored `.gr-ev` blocks are positioned by
     the verbatim formula (top=(h-8)*62+(m/60)*62, height=dur*62-5); untimed
     rows fall back to a 1h block at GR_START. A `.gr-now` indicator marks
     the current time on today's column. Vim nav: h/l shift the week anchor,
     t jumps to this week. api + the agenda types imported READ-ONLY. -->
<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { AgendaRow as AgendaRowT } from "$lib/types/AgendaRow";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  // Layout constants (from the plan's verbatim Agenda spec).
  const GR_HOURS = Array.from({ length: 9 }, (_, i) => 8 + i); // [8..16]
  const GR_SLOT = 62;
  const GR_START = 8;

  function isoDate(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${dd}`;
  }

  // Monday-anchored week. `weekOffset` shifts the visible week by ±N weeks.
  let weekOffset = $state(0);

  function mondayOf(d: Date): Date {
    const out = new Date(d);
    const dow = out.getDay(); // 0=Sun..6=Sat
    const delta = dow === 0 ? -6 : 1 - dow;
    out.setDate(out.getDate() + delta);
    out.setHours(0, 0, 0, 0);
    return out;
  }

  const weekStart = $derived.by(() => {
    const m = mondayOf(new Date());
    m.setDate(m.getDate() + weekOffset * 7);
    return m;
  });

  // 5 columns, Mon–Fri.
  const days = $derived.by(() => {
    const out: { iso: string; dow: string; dn: number; isToday: boolean }[] = [];
    const todayIso = isoDate(new Date());
    for (let i = 0; i < 5; i++) {
      const d = new Date(weekStart);
      d.setDate(weekStart.getDate() + i);
      out.push({
        iso: isoDate(d),
        dow: d.toLocaleDateString("en-US", { weekday: "short" }),
        dn: d.getDate(),
        isToday: isoDate(d) === todayIso,
      });
    }
    return out;
  });

  const fromIso = $derived(days[0]?.iso ?? isoDate(weekStart));
  const toIso = $derived(days[4]?.iso ?? isoDate(weekStart));

  const q = createQuery(() => ({
    queryKey: ["agenda", { from: fromIso, to: toIso, includeDone: false }] as const,
    queryFn: () => api.getAgenda(fromIso, toIso, false),
    enabled: !!fromIso && !!toIso,
  }));
  const rows = $derived((q.data ?? []) as AgendaRowT[]);

  type Positioned = {
    row: AgendaRowT;
    top: number;
    height: number;
    label: string;
    kind: "event" | "task";
  };

  // Rows for one day column → positioned `.gr-ev` blocks.
  function eventsForDay(iso: string): Positioned[] {
    const out: Positioned[] = [];
    for (const r of rows) {
      if (r.occurrence_date !== iso) continue;
      let h = GR_START;
      let m = 0;
      if (r.occurrence_time) {
        const [hh, mm] = r.occurrence_time.split(":").map((s) => parseInt(s, 10));
        if (!Number.isNaN(hh)) h = hh;
        if (!Number.isNaN(mm)) m = mm;
      }
      const dur = 1; // no end-time in AgendaRow → default 1h block
      const top = (h - GR_START) * GR_SLOT + (m / 60) * GR_SLOT;
      const height = dur * GR_SLOT - 5;
      out.push({
        row: r,
        top,
        height,
        label: r.text || "(untitled)",
        kind: r.kind,
      });
    }
    return out;
  }

  // `.gr-now` offset for today's column (only when current time is inside
  // the visible hour band).
  let nowTop = $state<number | null>(null);
  $effect(() => {
    const compute = () => {
      const d = new Date();
      const h = d.getHours();
      const m = d.getMinutes();
      if (h < GR_START || h > GR_HOURS[GR_HOURS.length - 1]) {
        nowTop = null;
        return;
      }
      nowTop = (h - GR_START) * GR_SLOT + (m / 60) * GR_SLOT;
    };
    compute();
    const id = setInterval(compute, 60_000);
    return () => clearInterval(id);
  });

  function openSource(r: AgendaRowT) {
    openPageInFocused(asPageId(r.source_note_id));
  }

  function fmtHour(h: number): string {
    const period = h >= 12 ? "PM" : "AM";
    const disp = h % 12 === 0 ? 12 : h % 12;
    return `${disp} ${period}`;
  }

  let rootEl = $state<HTMLDivElement | undefined>();
  $effect(() => {
    if (!rootEl) return;
    let cancelled = false;
    let elapsed = 0;
    const tick = () => {
      if (cancelled || !rootEl) return;
      if (!rootEl.contains(document.activeElement)) {
        rootEl.focus({ preventScroll: true });
      }
      elapsed += 50;
      if (elapsed > 500) return;
      setTimeout(tick, 50);
    };
    const start = setTimeout(tick, 0);
    return () => {
      cancelled = true;
      clearTimeout(start);
    };
  });

  function handleKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) {
      return;
    }
    switch (e.key) {
      case "h":
      case "ArrowLeft":
        e.preventDefault();
        weekOffset -= 1;
        break;
      case "l":
      case "ArrowRight":
        e.preventDefault();
        weekOffset += 1;
        break;
      case "t":
        e.preventDefault();
        weekOffset = 0;
        break;
    }
  }

  const weekLabel = $derived.by(() => {
    const start = weekStart;
    const end = new Date(weekStart);
    end.setDate(start.getDate() + 4);
    const opts: Intl.DateTimeFormatOptions = { month: "short", day: "numeric" };
    return `${start.toLocaleDateString("en-US", opts)} – ${end.toLocaleDateString("en-US", opts)}`;
  });
</script>

<div class="gr-pane focus">
  <div class="gr-pane-head">
    <span class="ttl">Agenda</span>
    <span class="sub">{weekLabel}</span>
    <span class="sp"></span>
    <span class="meta">h/l week · t today</span>
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    bind:this={rootEl}
    class="gr-agenda"
    tabindex="0"
    onkeydown={handleKey}
  >
    <div class="gr-agrid">
      <!-- gutter column: corner + hour labels -->
      <div class="gr-ag-col gutter">
        <div class="gr-ag-colhdr"></div>
        {#each GR_HOURS as h (h)}
          <div class="gr-ag-time">{fmtHour(h)}</div>
        {/each}
      </div>

      <!-- 5 day columns -->
      {#each days as day (day.iso)}
        <div class="gr-ag-col">
          <div class="gr-ag-colhdr" class:today={day.isToday}>
            <span class="dw">{day.dow}</span>
            <span class="dn">{day.dn}</span>
          </div>
          <div class="gr-ag-slots">
            {#each GR_HOURS as h (h)}
              <div class="gr-ag-slot"></div>
            {/each}

            {#if day.isToday && nowTop !== null}
              <div class="gr-now" style="top:{nowTop}px"></div>
            {/if}

            {#each eventsForDay(day.iso) as ev (ev.row.block_id + ":" + ev.row.occurrence_date)}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div
                class="gr-ev {ev.kind}"
                style="top:{ev.top}px;height:{ev.height}px"
                title={ev.label}
                onclick={() => openSource(ev.row)}
              >
                <div class="et">{ev.label}</div>
                {#if ev.row.occurrence_time}
                  <div class="em">{ev.row.occurrence_time}</div>
                {/if}
              </div>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .gr-pane {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    background: var(--bg);
    min-height: 0;
  }
  .gr-pane-head {
    display: flex;
    align-items: center;
    gap: 11px;
    padding: 14px 18px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .gr-pane-head .ttl {
    font-size: 16px;
    font-weight: 600;
    letter-spacing: -0.01em;
    color: var(--fg);
    white-space: nowrap;
  }
  .gr-pane-head .sub {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
  }
  .gr-pane-head .sp {
    flex: 1;
  }
  .gr-pane-head .meta {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
    white-space: nowrap;
  }

  /* Agenda time grid (verbatim Graphite CSS). */
  .gr-agenda {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    outline: none;
  }
  .gr-agrid {
    flex: 1;
    display: grid;
    grid-template-columns: 56px repeat(5, 1fr);
    overflow: auto;
  }
  .gr-ag-col {
    border-right: 1px solid var(--line);
    position: relative;
    min-width: 0;
  }
  .gr-ag-col.gutter {
    border-right: 1px solid var(--line);
  }
  .gr-ag-colhdr {
    height: 46px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 1px;
    border-bottom: 1px solid var(--line);
    border-right: 1px solid var(--line);
  }
  .gr-ag-colhdr .dw {
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--faint);
  }
  .gr-ag-colhdr .dn {
    font-size: 15px;
    font-weight: 600;
    color: var(--fg);
  }
  .gr-ag-colhdr.today .dn {
    color: var(--coral);
  }
  .gr-ag-time {
    height: 62px;
    font-family: var(--mono);
    font-size: 9.5px;
    color: var(--faint);
    text-align: right;
    padding: 2px 7px 0 0;
    border-top: 1px solid var(--line);
  }
  .gr-ag-slots {
    position: relative;
  }
  .gr-ag-slot {
    height: 62px;
    border-top: 1px solid var(--line);
  }
  .gr-ev {
    position: absolute;
    left: 5px;
    right: 5px;
    border-radius: 7px;
    padding: 6px 8px;
    overflow: hidden;
    border: 1px solid transparent;
    cursor: pointer;
  }
  .gr-ev .et {
    font-size: 11.5px;
    font-weight: 550;
    line-height: 1.25;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gr-ev .em {
    font-family: var(--mono);
    font-size: 9.5px;
    opacity: 0.8;
    margin-top: 1px;
  }
  .gr-ev.event {
    background: rgba(98, 184, 206, 0.16);
    border-color: rgba(98, 184, 206, 0.34);
    color: var(--event);
  }
  .gr-ev.task {
    background: rgba(232, 105, 127, 0.15);
    border-color: rgba(232, 105, 127, 0.34);
    color: var(--task);
  }
  .gr-now {
    position: absolute;
    left: 0;
    right: 0;
    height: 0;
    border-top: 1.5px solid var(--coral);
    z-index: 3;
  }
  .gr-now::before {
    content: "";
    position: absolute;
    left: -3px;
    top: -3.5px;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--coral);
  }
</style>
