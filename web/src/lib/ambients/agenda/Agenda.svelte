<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import type { AgendaRow as AgendaRowT } from "$lib/types/AgendaRow";
  import AgendaDay from "./AgendaDay.svelte";

  let { onNavigate }: AmbientRendererProps = $props();
  void onNavigate;

  // Window state — initial fetch is today → today + 60d; the scroll
  // sentinel bumps the upper bound by another 60 days as it scrolls into view.
  function isoDate(d: Date): string {
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const dd = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${dd}`;
  }
  const todayIso = isoDate(new Date());
  // Fetch from `today - LOOKBACK_DAYS` so overdue rows (anchor < today) are
  // included — the server gates `date >= from`, so without lookback the
  // overdue bucket is unreachable. 90 days catches recently-missed work
  // without flooding the view with ancient abandoned tasks.
  const LOOKBACK_DAYS = 90;
  const lowerIso = (() => {
    const d = new Date();
    d.setDate(d.getDate() - LOOKBACK_DAYS);
    return isoDate(d);
  })();
  let upperOffset = $state(60); // days past today
  let includeDone = $state(false);

  const upperIso = $derived.by(() => {
    const d = new Date();
    d.setDate(d.getDate() + upperOffset);
    return isoDate(d);
  });

  const q = createQuery(() => ({
    queryKey: ["agenda", { from: lowerIso, to: upperIso, includeDone }] as const,
    queryFn: () => api.getAgenda(lowerIso, upperIso, includeDone),
  }));

  const rows = $derived((q.data ?? []) as AgendaRowT[]);

  function formatDayHeader(d: Date): string {
    return d.toLocaleDateString("en-US", { weekday: "long", month: "short", day: "numeric" });
  }

  // Split into Overdue + per-day buckets across [today, upperIso].
  const buckets = $derived.by(() => {
    const overdue: AgendaRowT[] = [];
    const byDay = new Map<string, AgendaRowT[]>();
    for (const r of rows) {
      if (r.overdue) {
        overdue.push(r);
        continue;
      }
      let arr = byDay.get(r.occurrence_date);
      if (!arr) {
        arr = [];
        byDay.set(r.occurrence_date, arr);
      }
      arr.push(r);
    }
    // Walk the window day-by-day so empty days render as placeholders.
    const days: { iso: string; label: string; rows: AgendaRowT[] }[] = [];
    const start = new Date();
    for (let i = 0; i <= upperOffset; i++) {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      const iso = isoDate(d);
      const dayRows = byDay.get(iso) ?? [];
      const label =
        i === 0 ? `Today · ${formatDayHeader(d)}`
        : i === 1 ? `Tomorrow · ${formatDayHeader(d)}`
        : formatDayHeader(d);
      days.push({ iso, label, rows: dayRows });
    }
    return { overdue, days };
  });

  // Infinite scroll — when the sentinel is near, extend the window.
  let sentinel = $state<HTMLElement | undefined>();
  $effect(() => {
    const node = sentinel;
    if (!node) return;
    const obs = new IntersectionObserver((entries) => {
      for (const e of entries) {
        if (e.isIntersecting) upperOffset = upperOffset + 60;
      }
    }, { rootMargin: "200px" });
    obs.observe(node);
    return () => obs.disconnect();
  });

  // ── Keyboard nav ──────────────────────────────────────────────────────
  // Flatten the bucketed view into a single ordered list so j/k can move
  // through every visible row without caring which day bucket it lives in.
  const flatRows = $derived.by(() => {
    const out: AgendaRowT[] = [];
    out.push(...buckets.overdue);
    for (const day of buckets.days) {
      out.push(...day.rows);
    }
    return out;
  });
  const rowKey = (r: AgendaRowT) => `${r.block_id}:${r.occurrence_date}`;
  let selectedIndex = $state(0);
  const selectedKey = $derived(
    flatRows.length > 0 ? rowKey(flatRows[Math.min(selectedIndex, flatRows.length - 1)]) : null,
  );

  // Root element holds focus; tabindex makes it programmatically focusable
  // so we can grab focus on mount and dispatch keys from anywhere inside.
  let rootEl = $state<HTMLDivElement | undefined>();
  $effect(() => {
    if (rootEl && flatRows.length > 0 && selectedIndex === 0) {
      rootEl.focus({ preventScroll: true });
    }
  });

  // When the selected row changes, scroll it into view so j/k keep up
  // even if the picked row was just off-screen.
  $effect(() => {
    if (!selectedKey || !rootEl) return;
    const el = rootEl.querySelector(
      `[data-agenda-row="${CSS.escape(selectedKey)}"]`,
    ) as HTMLElement | null;
    el?.scrollIntoView({ block: "nearest" });
  });

  /** Synthesize a mouse click on the focused row's button matching the
   * given `data-action` attribute. The keyboard path piggybacks on the
   * row's existing handlers so there is exactly one code path per
   * action (no behavior drift between mouse and keyboard). */
  function fireRowAction(action: "mark-done" | "open-date" | "open-source" | "skip") {
    if (!selectedKey || !rootEl) return;
    const btn = rootEl.querySelector(
      `[data-agenda-row="${CSS.escape(selectedKey)}"] [data-action="${action}"]`,
    ) as HTMLElement | null;
    btn?.click();
  }

  function handleKey(e: KeyboardEvent) {
    // Let the DatePicker (and any other inputs/popovers) own their keys.
    const target = e.target as HTMLElement;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) {
      return;
    }
    if (flatRows.length === 0) return;
    switch (e.key) {
      case "j":
      case "ArrowDown":
        e.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, flatRows.length - 1);
        break;
      case "k":
      case "ArrowUp":
        e.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        break;
      case "g":
        // `gg` would need chord state; for v1 just G/g as alias for top.
        e.preventDefault();
        selectedIndex = 0;
        break;
      case "G":
        e.preventDefault();
        selectedIndex = flatRows.length - 1;
        break;
      case "Enter":
        e.preventDefault();
        fireRowAction("open-source");
        break;
      case "x":
        e.preventDefault();
        fireRowAction("mark-done");
        break;
      case "d":
        e.preventDefault();
        fireRowAction("open-date");
        break;
      case "s":
        // `s` = skip (only fires on recurring anchor rows; no-op otherwise).
        e.preventDefault();
        fireRowAction("skip");
        break;
    }
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<div
  bind:this={rootEl}
  class="flex flex-col h-full min-h-0 overflow-auto px-4 py-3 outline-none"
  tabindex="0"
  onkeydown={handleKey}
>
  <header class="flex items-center justify-between mb-3 text-[12px]">
    <div class="font-semibold">📋 Agenda <span class="text-muted-foreground/40 font-normal text-[11px]">j/k · ↵ open · x done · d date · s skip</span></div>
    <label class="flex items-center gap-2 cursor-pointer text-muted-foreground">
      <input type="checkbox" bind:checked={includeDone} class="cursor-pointer" />
      <span>show done</span>
    </label>
  </header>

  {#if q.isLoading}
    <div class="text-muted-foreground/60 text-[12px]">loading…</div>
  {:else}
    {#if buckets.overdue.length > 0}
      <AgendaDay label="Overdue" rows={buckets.overdue} emphasis="overdue" {selectedKey} />
    {/if}
    {#each buckets.days as day (day.iso)}
      {#if day.rows.length > 0}
        <AgendaDay label={day.label} rows={day.rows} {selectedKey} />
      {:else}
        <AgendaDay label={`${day.label} — empty`} rows={[]} emphasis="empty" />
      {/if}
    {/each}
    <div bind:this={sentinel} class="h-px"></div>
  {/if}
</div>
