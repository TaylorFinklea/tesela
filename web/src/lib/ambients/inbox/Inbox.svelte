<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { getAppQueryClient } from "$lib/app-query-client.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { applyTriage, triageActionForKey } from "$lib/triage.svelte";
  import DatePicker from "$lib/components/DatePicker.svelte";
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import type { QueryItem } from "$lib/types/QueryItem";
  import InboxRow from "./InboxRow.svelte";

  let { onNavigate }: AmbientRendererProps = $props();
  void onNavigate;

  // ── Data ──────────────────────────────────────────────────────────────
  // Same query the v4 Inbox widget uses so both surfaces stay in lockstep.
  const INBOX_QUERY = "kind:block -has:status";
  const q = createQuery(() => ({
    queryKey: ["widget", "inbox"] as const,
    queryFn: () => api.executeQuery(INBOX_QUERY),
  }));

  /** Daily / system pages are filtered out — they're not "untriaged
   * captures." Mirrors `isInboxableRow` in `QueryWidgetView.svelte`. */
  const TRIAGED_PAGE_TYPES = new Set(["Tag", "Property", "Query", "Template"]);
  function isInboxable(item: QueryItem): boolean {
    if (item.kind !== "block") return false;
    if (/^\d{4}-\d{2}-\d{2}$/.test(item.page_id)) return false;
    if (item.page_note_type && TRIAGED_PAGE_TYPES.has(item.page_note_type)) return false;
    return true;
  }

  // Soft cap so an old mosaic with thousands of legacy untriaged blocks
  // doesn't choke the renderer on first open. A future virtualization
  // pass can lift this; for now, 200 is enough headroom for any real
  // triage session.
  const ROW_CAP = 200;
  const rows = $derived.by<QueryItem[]>(() => {
    const result = q.data;
    if (!result) return [];
    const out: QueryItem[] = [];
    for (const g of result.groups) {
      for (const it of g.items) {
        if (isInboxable(it)) out.push(it);
        if (out.length >= ROW_CAP) return out;
      }
    }
    return out;
  });
  const totalAvailable = $derived.by<number>(() => {
    const result = q.data;
    if (!result) return 0;
    let n = 0;
    for (const g of result.groups) {
      for (const it of g.items) if (isInboxable(it)) n++;
    }
    return n;
  });

  const rowKey = (r: QueryItem) => r.block_id ?? r.page_id;

  // ── Keyboard nav / focus ──────────────────────────────────────────────
  let selectedIndex = $state(0);
  const selectedKey = $derived(
    rows.length > 0 ? rowKey(rows[Math.min(selectedIndex, rows.length - 1)]) : null,
  );
  const selectedRow = $derived<QueryItem | null>(
    rows.length > 0 ? rows[Math.min(selectedIndex, rows.length - 1)] : null,
  );

  let rootEl = $state<HTMLDivElement | undefined>();
  $effect(() => {
    // Grab focus on first paint so j/k work without the user clicking
    // into the pane. preventScroll keeps the container from snapping.
    if (rootEl && rows.length > 0 && selectedIndex === 0) {
      rootEl.focus({ preventScroll: true });
    }
  });
  $effect(() => {
    // Keep the focused row visible as the selection moves.
    if (!selectedKey || !rootEl) return;
    const el = rootEl.querySelector(
      `[data-inbox-row="${CSS.escape(selectedKey)}"]`,
    ) as HTMLElement | null;
    el?.scrollIntoView({ block: "nearest" });
  });

  // ── Actions ───────────────────────────────────────────────────────────

  async function refresh() {
    const qc = getAppQueryClient();
    if (qc) await qc.invalidateQueries({ queryKey: ["widget", "inbox"] });
  }

  async function triage(row: QueryItem, key: string) {
    const action = triageActionForKey(key);
    if (!action || !row.block_id) return;
    try {
      const ok = await applyTriage(row.page_id, row.block_id, action);
      if (ok) await refresh();
    } catch {
      toast("Triage failed", "error");
    }
  }

  function fireSourceOpen() {
    if (!selectedKey || !rootEl) return;
    const btn = rootEl.querySelector(
      `[data-inbox-row="${CSS.escape(selectedKey)}"] [data-action="open-source"]`,
    ) as HTMLElement | null;
    btn?.click();
  }

  // ── Date picker (s key) ───────────────────────────────────────────────
  // Hoisted to the pane so the keyboard path doesn't need each row to
  // own its own popover. Anchored to the selected row's bounding rect.
  let pickerForKey = $state<string | null>(null);
  let pickerPos = $state<{ x: number; y: number }>({ x: 0, y: 0 });

  function openSchedule() {
    if (!selectedKey || !rootEl) return;
    const el = rootEl.querySelector(
      `[data-inbox-row="${CSS.escape(selectedKey)}"]`,
    ) as HTMLElement | null;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    pickerPos = { x: rect.left, y: rect.bottom + 4 };
    pickerForKey = selectedKey;
  }

  async function handlePick(
    iso: string,
    _time: string | null,
    recurrence: string | null,
    _field: "deadline" | "scheduled" | null,
  ) {
    const row = selectedRow;
    pickerForKey = null;
    if (!row?.block_id) return;
    try {
      await api.setBlockProperty(row.block_id, "scheduled", iso);
      if (recurrence !== null) {
        await api.setBlockProperty(row.block_id, "recurring", recurrence);
      }
      await refresh();
    } catch {
      toast("Failed to schedule", "error");
    }
  }

  // ── Keyboard handler ──────────────────────────────────────────────────
  function handleKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) {
      return;
    }
    if (rows.length === 0) return;
    // Triage keys first so we can short-circuit before normal nav.
    if (selectedRow && triageActionForKey(e.key) !== null) {
      e.preventDefault();
      triage(selectedRow, e.key);
      return;
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
      case "Enter":
        e.preventDefault();
        fireSourceOpen();
        break;
      case "s":
        e.preventDefault();
        openSchedule();
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
    <div class="font-semibold">
      📥 Inbox
      <span class="text-muted-foreground/40 font-normal text-[11px]">
        j/k · ↵ open · t todo · d doing · x done · s schedule
      </span>
    </div>
    <div class="text-muted-foreground/60 text-[11px]">
      {#if totalAvailable > rows.length}
        showing {rows.length} of {totalAvailable} untriaged
      {:else}
        {rows.length} untriaged
      {/if}
    </div>
  </header>

  {#if q.isLoading}
    <div class="text-muted-foreground/60 text-[12px]">loading…</div>
  {:else if rows.length === 0}
    <div class="text-muted-foreground/50 text-[12px] italic">Inbox clear ✓</div>
  {:else}
    {#each rows as row (rowKey(row))}
      <InboxRow {row} selected={selectedKey === rowKey(row)} />
    {/each}
  {/if}
</div>

{#if pickerForKey}
  <DatePicker
    initialRecurrence={null}
    position={pickerPos}
    onPick={handlePick}
    onClose={() => (pickerForKey = null)}
  />
{/if}
