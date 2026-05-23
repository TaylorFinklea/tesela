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
  let upperOffset = $state(60); // days past today
  let includeDone = $state(false);

  const upperIso = $derived.by(() => {
    const d = new Date();
    d.setDate(d.getDate() + upperOffset);
    return isoDate(d);
  });

  const q = createQuery(() => ({
    queryKey: ["agenda", { from: todayIso, to: upperIso, includeDone }] as const,
    queryFn: () => api.getAgenda(todayIso, upperIso, includeDone),
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
</script>

<div class="flex flex-col h-full min-h-0 overflow-auto px-4 py-3">
  <header class="flex items-center justify-between mb-3 text-[12px]">
    <div class="font-semibold">📋 Agenda</div>
    <label class="flex items-center gap-2 cursor-pointer text-muted-foreground">
      <input type="checkbox" bind:checked={includeDone} class="cursor-pointer" />
      <span>show done</span>
    </label>
  </header>

  {#if q.isLoading}
    <div class="text-muted-foreground/60 text-[12px]">loading…</div>
  {:else}
    {#if buckets.overdue.length > 0}
      <AgendaDay label="Overdue" rows={buckets.overdue} emphasis="overdue" />
    {/if}
    {#each buckets.days as day (day.iso)}
      {#if day.rows.length > 0}
        <AgendaDay label={day.label} rows={day.rows} />
      {:else}
        <AgendaDay label={`${day.label} — empty`} rows={[]} emphasis="empty" />
      {/if}
    {/each}
    <div bind:this={sentinel} class="h-px"></div>
  {/if}
</div>
