<!-- web/src/lib/graphite/views/GrInbox.svelte — Part A, Task A4.
     Graphite inbox triage. NEW presentation over the SAME data layer the
     v5 Inbox ambient uses: the inbox-DSL chip helpers (chipsFromDsl /
     dslFromChips / defaultInboxDsl / CHIP_REGISTRY), api.executeQuery for
     rows (filtered to kind==="block"), and triageActionForKey / applyTriage
     for the file/triage actions. The chip bar uses the foundation GrChip;
     cards are the verbatim `.gr-icard` Graphite design. j/k nav + t/d/x
     triage + s schedule + Enter/o open mirror the ambient's keymap. All
     shared modules imported READ-ONLY. -->
<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { getAppQueryClient } from "$lib/app-query-client.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import {
    applyTriage,
    triageActionForKey,
    type TriageAction,
  } from "$lib/triage.svelte";
  import {
    chipsFromDsl,
    dslFromChips,
    defaultInboxDsl,
    CHIP_REGISTRY,
    type ChipState,
  } from "$lib/ambients/inbox/chips";
  import type { QueryItem } from "$lib/types/QueryItem";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  import GrChip from "$lib/graphite/GrChip.svelte";
  import GrButton from "$lib/graphite/GrButton.svelte";
  import GrIcon from "$lib/graphite/GrIcon.svelte";

  // The Graphite inbox reads the same default DSL as the ambient seed.
  // (The full saved-filter switcher + DSL persistence stays in the v5
  // Inbox; the Graphite view drives its chips from in-memory state over
  // the registry, which is enough for triage.)
  let chipState = $state<ChipState>(chipsFromDsl(defaultInboxDsl()));
  const activeDsl = $derived(dslFromChips(chipState));

  const rowsQuery = createQuery(() => ({
    queryKey: ["widget", "inbox", activeDsl] as const,
    queryFn: () => api.executeQuery(activeDsl),
    enabled: activeDsl.length > 0,
  }));

  const ROW_CAP = 200;
  const rows = $derived.by<QueryItem[]>(() => {
    const result = rowsQuery.data;
    if (!result) return [];
    const out: QueryItem[] = [];
    for (const g of result.groups) {
      for (const it of g.items) {
        if (it.kind !== "block") continue;
        out.push(it);
        if (out.length >= ROW_CAP) return out;
      }
    }
    return out;
  });

  function toggleChip(chipId: string) {
    chipState = {
      ...chipState,
      active: { ...chipState.active, [chipId]: !chipState.active[chipId] },
    };
  }

  // ── selection / keyboard nav ───────────────────────────────────────────
  let selectedIndex = $state(0);
  const selectedRow = $derived<QueryItem | null>(
    rows.length > 0 ? rows[Math.min(selectedIndex, rows.length - 1)] : null,
  );
  const rowKey = (r: QueryItem) => r.block_id ?? r.page_id;

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
  $effect(() => {
    const key = selectedRow ? rowKey(selectedRow) : null;
    if (!key || !rootEl) return;
    const el = rootEl.querySelector(
      `[data-inbox-row="${CSS.escape(key)}"]`,
    ) as HTMLElement | null;
    el?.scrollIntoView({ block: "nearest" });
  });

  async function triage(row: QueryItem, action: TriageAction) {
    if (!row.block_id) return;
    try {
      const ok = await applyTriage(row.page_id, row.block_id, action);
      if (ok) {
        const qc = getAppQueryClient();
        if (qc) await qc.invalidateQueries({ queryKey: ["widget", "inbox"] });
      }
    } catch {
      toast("Triage failed", "error");
    }
  }

  async function processAll() {
    const qc = getAppQueryClient();
    for (const row of rows) {
      if (!row.block_id) continue;
      try {
        await applyTriage(row.page_id, row.block_id, "todo");
      } catch {
        /* keep going; one failure shouldn't abort the batch */
      }
    }
    if (qc) await qc.invalidateQueries({ queryKey: ["widget", "inbox"] });
  }

  function openSource(row: QueryItem) {
    openPageInFocused(asPageId(row.page_id));
  }

  async function snooze(row: QueryItem) {
    if (!row.block_id) return;
    // Snooze == schedule for tomorrow. (The full DatePicker stays in the
    // v5 Inbox; the Graphite quick-action just pushes a day.)
    const d = new Date();
    d.setDate(d.getDate() + 1);
    const iso = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    try {
      await api.setBlockProperty(row.block_id, "scheduled", iso);
      const qc = getAppQueryClient();
      if (qc) await qc.invalidateQueries({ queryKey: ["widget", "inbox"] });
      toast(`Snoozed → ${iso}`, "info");
    } catch {
      toast("Failed to snooze", "error");
    }
  }

  function handleKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) {
      return;
    }
    if (rows.length === 0) return;
    if (selectedRow) {
      const action = triageActionForKey(e.key);
      if (action !== null) {
        e.preventDefault();
        triage(selectedRow, action);
        return;
      }
    }
    switch (e.key) {
      case "j":
      case "ArrowDown":
        e.preventDefault();
        selectedIndex = Math.min(selectedIndex + 1, rows.length - 1);
        break;
      case "k":
      case "ArrowUp":
        e.preventDefault();
        selectedIndex = Math.max(selectedIndex - 1, 0);
        break;
      case "g":
        e.preventDefault();
        selectedIndex = 0;
        break;
      case "G":
        e.preventDefault();
        selectedIndex = rows.length - 1;
        break;
      case "s":
        e.preventDefault();
        if (selectedRow) snooze(selectedRow);
        break;
      case "Enter":
      case "o":
        e.preventDefault();
        if (selectedRow) openSource(selectedRow);
        break;
    }
  }

  function srcGlyph(row: QueryItem): string {
    return row.primary_tag ? "hash" : "file-text";
  }
</script>

<div class="gr-pane focus">
  <div class="gr-pane-head">
    <span class="ttl">Inbox</span>
    <span class="sp"></span>
    <span class="meta">{rows.length}</span>
    <GrButton variant="cta" onclick={() => void processAll()}>Process all</GrButton>
  </div>

  <div class="gr-chipbar">
    {#each CHIP_REGISTRY as chip (chip.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <span
        class="gr-chip-wrap"
        title={chip.hint}
        onclick={() => toggleChip(chip.id)}
      >
        <GrChip active={chipState.active[chip.id] ?? false}>
          {chip.glyph} {chip.label}
        </GrChip>
      </span>
    {/each}
  </div>

  <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
  <div
    bind:this={rootEl}
    class="gr-inbox-body"
    tabindex="0"
    onkeydown={handleKey}
  >
    {#if rowsQuery.isLoading}
      <div class="gr-empty">loading…</div>
    {:else if rows.length === 0}
      <div class="gr-empty">Inbox clear ✓</div>
    {:else}
      {#each rows as row (rowKey(row))}
        {@const sel = selectedRow ? rowKey(selectedRow) === rowKey(row) : false}
        <!-- svelte-ignore a11y_click_events_have_key_events -->
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="gr-icard"
          class:sel
          data-inbox-row={rowKey(row)}
          onclick={() => openSource(row)}
        >
          <div class="src"><GrIcon name={srcGlyph(row)} size={15} /></div>
          <div class="gr-icard-body">
            <div class="txt">{row.text || "(empty block)"}</div>
            <div class="meta">
              <span class="pill">{row.title || row.page_id}</span>
              {#if row.primary_tag}<span class="pill">#{row.primary_tag}</span>{/if}
            </div>
          </div>
          <div class="gr-icard-acts">
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span
              class="gr-iact"
              title="todo (t)"
              onclick={(e) => {
                e.stopPropagation();
                triage(row, "todo");
              }}
            ><GrIcon name="square-check" size={15} /></span>
            <span
              class="gr-iact"
              title="doing (d)"
              onclick={(e) => {
                e.stopPropagation();
                triage(row, "doing");
              }}
            ><GrIcon name="bolt" size={15} /></span>
            <span
              class="gr-iact"
              title="snooze (s)"
              onclick={(e) => {
                e.stopPropagation();
                void snooze(row);
              }}
            ><GrIcon name="clock" size={15} /></span>
            <span
              class="gr-iact go"
              title="open (o)"
              onclick={(e) => {
                e.stopPropagation();
                openSource(row);
              }}
            ><GrIcon name="corner-down-right" size={15} /></span>
          </div>
        </div>
      {/each}
    {/if}
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
  .gr-pane.focus {
    flex: 1;
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
  .gr-pane-head .sp {
    flex: 1;
  }
  .gr-pane-head .meta {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
    white-space: nowrap;
  }
  .gr-empty {
    color: var(--faint);
    font-family: var(--mono);
    font-size: 12px;
    padding: 8px 2px;
  }

  /* Chip bar + cards (verbatim Graphite CSS). */
  .gr-chipbar {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 11px 18px;
    border-bottom: 1px solid var(--line);
    flex-wrap: wrap;
  }
  .gr-chip-wrap {
    display: inline-flex;
  }
  .gr-inbox-body {
    flex: 1;
    overflow: auto;
    padding: 12px 18px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    outline: none;
  }
  .gr-icard {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 13px 14px;
    border-radius: 11px;
    background: var(--surface);
    border: 1px solid var(--line);
    transition: border-color 0.14s;
    cursor: pointer;
  }
  .gr-icard:hover {
    border-color: var(--line-2);
  }
  .gr-icard.sel {
    background: var(--raised);
    border-color: var(--coral-line);
  }
  .gr-icard .src {
    width: 30px;
    height: 30px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: var(--raised-2);
    color: var(--subtle);
    flex-shrink: 0;
  }
  .gr-icard-body {
    flex: 1;
    min-width: 0;
  }
  .gr-icard .txt {
    font-size: 14px;
    color: var(--fg);
    line-height: 1.45;
  }
  .gr-icard .meta {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 7px;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
  }
  .gr-icard .meta .pill {
    padding: 2px 7px;
    border-radius: 5px;
    background: var(--raised-2);
    color: var(--subtle);
  }
  .gr-icard-acts {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
  }
  .gr-iact {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    border-radius: 7px;
    color: var(--subtle);
    cursor: pointer;
    border: 1px solid transparent;
  }
  .gr-iact:hover {
    background: var(--raised-2);
    color: var(--fg);
    border-color: var(--line);
  }
  .gr-iact.go:hover {
    color: var(--coral);
  }
</style>
