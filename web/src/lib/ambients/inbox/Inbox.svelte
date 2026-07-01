<script lang="ts">
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import { getAppQueryClient } from "$lib/app-query-client.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { applyTriage, triageActionForKey } from "$lib/triage.svelte";
  import DatePicker from "$lib/components/DatePicker.svelte";
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  import type { QueryItem } from "$lib/types/QueryItem";
  import type { Note } from "$lib/types/Note";
  import type { TypeDefinition } from "$lib/types/TypeDefinition";
  import {
    chipsFromDsl,
    dslFromChips,
    defaultInboxDsl,
    type ChipState,
  } from "./chips";
  import ChipBar from "./ChipBar.svelte";
  import InboxRow from "./InboxRow.svelte";
  import RawDslSheet from "./RawDslSheet.svelte";

  let { onNavigate }: AmbientRendererProps = $props();
  void onNavigate;

  // ── The active filter note (saved query) ──────────────────────────────
  // The Inbox is backed by a real note with `note_type: Query` whose
  // slug is the "active filter." Default is `inbox`; the user can
  // create more saved filters via "Save as…" and switch between them
  // via the header dropdown — each rebuilds the chip state from a
  // different note's `query::` line.
  //
  // `activeSlug` persists in localStorage so the user's last-used
  // filter survives reloads.
  const DEFAULT_FILTER_SLUG = "inbox";
  const ACTIVE_SLUG_STORAGE_KEY = "tesela.inbox.activeFilterSlug";

  let activeSlug = $state<string>(
    (typeof localStorage !== "undefined" &&
      localStorage.getItem(ACTIVE_SLUG_STORAGE_KEY)) ||
      DEFAULT_FILTER_SLUG,
  );

  function setActiveSlug(slug: string) {
    activeSlug = slug;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem(ACTIVE_SLUG_STORAGE_KEY, slug);
    }
  }

  const inboxNoteQuery = createQuery(() => ({
    queryKey: ["note", activeSlug] as const,
    // Tolerate 404 and surface as `null` so the seed flow can run.
    queryFn: async () => {
      try {
        return await api.getNote(activeSlug);
      } catch {
        return null as Note | null;
      }
    },
  }));

  // All saved filters (every `note_type: Query` note in the mosaic).
  // Drives the "Filter ▾" dropdown. Piggybacks on the same notes cache
  // that Station / RailWidgets use, so opening Inbox doesn't fire an
  // extra round-trip when one of those is already mounted.
  // Raised 500→5000 (tesela-sclr.1): 500 silently hid saved-filter notes
  // created past note #500 from the switcher.
  const allNotesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  /** Only inbox-shaped saved filters appear in the switcher. Default
   *  `inbox` slug + anything namespaced `inbox-*` (matches the slug
   *  convention `saveAsFilter` writes). Filters out the system
   *  widgets (calendar / tasks / dailies / etc) — those are also
   *  `note_type: Query` notes but aren't inbox alternatives. */
  const availableFilters = $derived<Array<{ slug: string; title: string }>>(
    ((allNotesQuery.data ?? []) as Note[])
      .filter((n) => n.metadata.note_type === "Query")
      .filter((n) => n.id === DEFAULT_FILTER_SLUG || n.id.startsWith("inbox-"))
      .map((n) => ({ slug: n.id, title: n.title || n.id }))
      .sort((a, b) => a.title.localeCompare(b.title)),
  );

  // /types drives the dynamic Types chip-group. Cached aggressively
  // (types rarely change at runtime); refetched on focus so a user who
  // edits types.toml sees their new types appear without a full reload.
  const typesQuery = createQuery(() => ({
    queryKey: ["types"] as const,
    queryFn: () => api.listTypes(),
    staleTime: 60_000,
  }));
  const availableTypes = $derived<string[]>(
    ((typesQuery.data ?? []) as TypeDefinition[])
      .map((t) => t.name)
      // Filter out lowercase types — the user's TypeRegistry mixes
      // metaclasses (Domain, Issue, Task) with thing-names (book,
      // flashlight). For the chip group we want the inclusive
      // metaclasses; lowercase entries are typically per-thing tags.
      .filter((n) => /^[A-Z]/.test(n))
      .sort((a, b) => a.localeCompare(b)),
  );

  /** Extract the `query::` body line from a Query-type note. Mirrors
   * `readBodyProperty` in widget-registry. */
  function readQueryFromNote(note: Note): string {
    const custom = note.metadata.custom ?? {};
    const fromFrontmatter = typeof custom.query === "string" ? custom.query : "";
    if (fromFrontmatter.length > 0) return fromFrontmatter;
    const body = note.content.includes("\n---")
      ? note.content.slice(note.content.indexOf("\n---", 3) + 4)
      : note.content;
    const m = body.match(/^\s*query::\s*(.*)$/im);
    return m ? m[1].trim() : "";
  }

  /**
   * Optimistic override used ONLY when the inbox note doesn't exist
   * yet (no cache entry to mutate). For the common case we update the
   * cached Note in place via `qc.setQueryData` in `scheduleSave` so
   * the derived chain re-runs naturally — that way the chip state
   * and rows query both stay coherent and there's nothing to "clear"
   * after a save (the server's PUT response writes back the same DSL
   * we already put in the cache).
   */
  let pendingSeedDsl = $state<string | null>(null);

  /** Active DSL — read from the cached note (mutated optimistically in
   *  `scheduleSave`), or from the pending seed if no note exists yet,
   *  or the registry default while still loading. */
  const activeDsl = $derived.by<string>(() => {
    const note = inboxNoteQuery.data;
    if (note) {
      const fromNote = readQueryFromNote(note);
      return fromNote.length > 0 ? fromNote : defaultInboxDsl();
    }
    if (pendingSeedDsl !== null) return pendingSeedDsl;
    return defaultInboxDsl();
  });

  const chipState = $derived<ChipState>(chipsFromDsl(activeDsl));

  // ── Rows ──────────────────────────────────────────────────────────────
  // Now that `is:heading`, `on:daily-page`, `on:system-pages` are real
  // DSL clauses, the post-fetch filter is gone — every active chip
  // contributes its clause(s) to the DSL we send, and the server hands
  // back exactly what should display.
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
  const totalAvailable = $derived.by<number>(() => {
    const result = rowsQuery.data;
    if (!result) return 0;
    let n = 0;
    for (const g of result.groups) {
      for (const it of g.items) if (it.kind === "block") n++;
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
    if (!selectedKey || !rootEl) return;
    const el = rootEl.querySelector(
      `[data-inbox-row="${CSS.escape(selectedKey)}"]`,
    ) as HTMLElement | null;
    el?.scrollIntoView({ block: "nearest" });
  });
  function handlePointerDown() {
    rootEl?.focus({ preventScroll: true });
  }

  // ── DSL persistence ───────────────────────────────────────────────────
  // Toggling a chip immediately re-derives the DSL and starts a 500ms
  // debounce; on flush we PUT the inbox note (creating it on the first
  // toggle if the note doesn't yet exist). Mirrors the BlockOutliner
  // save model so the user gets the same "save-as-you-edit" feel.
  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingDsl: string | null = null;

  function scheduleSave(nextDsl: string) {
    // Optimistic cache update — mutate the cached Note's content so
    // the derived `activeDsl` / `chipState` / `rowsQuery` chain
    // immediately reflects the change. The eventual PUT writes back
    // the same DSL, so the WS-driven refetch is a no-op. No racey
    // "clear override after save" dance needed.
    const qc = getAppQueryClient();
    const existing = inboxNoteQuery.data;
    if (qc && existing) {
      const optimisticNote: Note = {
        ...existing,
        content: buildInboxNoteContent(existing, nextDsl),
      };
      qc.setQueryData(["note", activeSlug], optimisticNote);
    } else {
      // No note cached yet (first-ever toggle on a fresh mosaic) —
      // keep the new DSL in a fallback state so the chip lights up
      // while the seed write is in flight. Cleared once the create
      // round-trips and populates the cache.
      pendingSeedDsl = nextDsl;
    }
    pendingDsl = nextDsl;
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(() => void flushSave(), 500);
  }

  async function flushSave() {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    if (pendingDsl === null) return;
    const dsl = pendingDsl;
    pendingDsl = null;
    const qc = getAppQueryClient();
    try {
      const existing = inboxNoteQuery.data;
      const newContent = buildInboxNoteContent(existing, dsl);
      if (existing) {
        await api.updateNote(activeSlug, newContent);
      } else {
        // Seed the note on first save. createNote derives the slug
        // from the title; for the default Inbox slug we pass the
        // canonical name. The system-widgets seeder may also create
        // it lazily — if it races, the catch below swallows the
        // dup-id error and we re-PUT into the existing note.
        try {
          await api.createNote(titleForNewFilter(activeSlug), newContent, []);
        } catch {
          await api.updateNote(activeSlug, newContent);
        }
      }
      // Only invalidate the rows query — the note cache was already
      // updated optimistically in `scheduleSave`, so re-fetching it
      // here would just race the user's next toggle. Rows have to
      // re-run because the underlying mosaic data may have shifted.
      if (qc) {
        await qc.invalidateQueries({ queryKey: ["widget", "inbox"] });
      }
      // Now that the cache holds the real note (created or updated),
      // the seed fallback is no longer needed. Safe to clear.
      pendingSeedDsl = null;
    } catch {
      toast("Failed to save Inbox query", "error");
    }
  }

  /** Build a Query-note content string. Reuses the existing note's
   *  frontmatter when present (preserving icon/color/section), splicing
   *  in a fresh `query::` line. Greenfield case writes a minimal
   *  frontmatter + body — title derived from the active slug so a
   *  saved filter at `inbox-work` reads as "Inbox Work" in the sidebar. */
  function buildInboxNoteContent(existing: Note | null | undefined, dsl: string): string {
    if (existing) {
      const content = existing.content;
      const fmEnd = content.indexOf("\n---", 3);
      const frontmatter = fmEnd >= 0 ? content.slice(0, fmEnd + 4) : null;
      const bodyRaw = fmEnd >= 0 ? content.slice(fmEnd + 4) : content;
      // Strip any leading newlines so the rewritten body starts clean.
      const body = bodyRaw.replace(/^\n+/, "");
      const lines = body.split("\n");
      const queryLineIdx = lines.findIndex((l) => /^\s*query::/i.test(l));
      if (queryLineIdx >= 0) {
        lines[queryLineIdx] = `query:: ${dsl}`;
      } else {
        lines.unshift(`query:: ${dsl}`);
      }
      const newBody = lines.join("\n");
      return frontmatter ? `${frontmatter}\n\n${newBody}` : newBody;
    }
    // First-write — minimal frontmatter so the rest of the app
    // recognizes this as a Query widget.
    const title = titleForNewFilter(activeSlug);
    return [
      "---",
      `title: "${title}"`,
      'type: "Query"',
      'icon: "inbox"',
      'color: "teal"',
      'section: "saved"',
      "---",
      "",
      `query:: ${dsl}`,
      "",
    ].join("\n");
  }

  /** Render a friendly title from a slug. `inbox` → "Inbox"; any
   *  other slug becomes Title Case with hyphens replaced by spaces. */
  function titleForNewFilter(slug: string): string {
    if (slug === DEFAULT_FILTER_SLUG) return "Inbox";
    return slug
      .split("-")
      .filter((w) => w.length > 0)
      .map((w) => w[0].toUpperCase() + w.slice(1))
      .join(" ");
  }

  /** Slugify a user-entered filter name. Lowercases, replaces
   *  whitespace with `-`, drops non-alphanumeric. */
  function slugify(name: string): string {
    return name
      .trim()
      .toLowerCase()
      .replace(/\s+/g, "-")
      .replace(/[^a-z0-9-]/g, "");
  }

  // ── Save-as-filter + switcher ─────────────────────────────────────

  let savingFilter = $state(false);

  async function saveAsFilter() {
    if (savingFilter) return;
    const name = window.prompt(
      "Name this filter:\n(e.g. \"Work\" → :inbox-work)",
      "",
    );
    if (!name) return;
    const baseSlug = slugify(name);
    if (!baseSlug) {
      toast("That name doesn't slugify cleanly. Try letters + spaces.", "error");
      return;
    }
    // Namespace under `inbox-` so the user can tell saved filters apart
    // from arbitrary Query notes. The default Inbox keeps its raw slug.
    const slug = baseSlug.startsWith("inbox-") || baseSlug === DEFAULT_FILTER_SLUG
      ? baseSlug
      : `inbox-${baseSlug}`;
    if (availableFilters.some((f) => f.slug === slug)) {
      toast(`A filter named ${slug} already exists. Pick a different name.`, "error");
      return;
    }
    savingFilter = true;
    try {
      // Build the new note's content from the current chip state DSL.
      const content = buildContentForNewFilter(slug, activeDsl);
      await api.createNote(titleForNewFilter(slug), content, []);
      const qc = getAppQueryClient();
      if (qc) {
        await qc.invalidateQueries({ queryKey: ["notes"] });
      }
      setActiveSlug(slug);
      toast(`Saved as filter ${slug}`, "success");
    } catch (e) {
      toast(`Failed to save filter: ${e}`, "error");
    } finally {
      savingFilter = false;
    }
  }

  /** Greenfield content for a brand-new saved filter — never reuses
   *  the current activeSlug's note since we want a distinct file. */
  function buildContentForNewFilter(slug: string, dsl: string): string {
    const title = titleForNewFilter(slug);
    return [
      "---",
      `title: "${title}"`,
      'type: "Query"',
      'icon: "inbox"',
      'color: "teal"',
      'section: "saved"',
      "---",
      "",
      `query:: ${dsl}`,
      "",
    ].join("\n");
  }

  function switchFilter(slug: string) {
    if (slug === activeSlug) return;
    setActiveSlug(slug);
    // The TanStack queryKey `["note", activeSlug]` re-runs on the new
    // slug; no manual invalidation needed.
  }

  function toggleChip(chipId: string) {
    const next: ChipState = {
      ...chipState,
      active: { ...chipState.active, [chipId]: !chipState.active[chipId] },
    };
    scheduleSave(dslFromChips(next));
  }

  function toggleType(typeName: string) {
    const current = new Set(chipState.activeTypes);
    if (current.has(typeName)) current.delete(typeName);
    else current.add(typeName);
    const next: ChipState = {
      ...chipState,
      activeTypes: Array.from(current),
    };
    scheduleSave(dslFromChips(next));
  }

  function hidePage(pageId: string) {
    if (chipState.hiddenPages.includes(pageId)) return;
    const next: ChipState = {
      ...chipState,
      hiddenPages: [...chipState.hiddenPages, pageId],
    };
    scheduleSave(dslFromChips(next));
    toast(`Hidden ${pageId} from Inbox`, "info");
  }

  function unhidePage(pageId: string) {
    const next: ChipState = {
      ...chipState,
      hiddenPages: chipState.hiddenPages.filter((p) => p !== pageId),
    };
    scheduleSave(dslFromChips(next));
  }

  function unhideBlock(blockId: string) {
    const next: ChipState = {
      ...chipState,
      hiddenBlocks: chipState.hiddenBlocks.filter((b) => b !== blockId),
    };
    scheduleSave(dslFromChips(next));
  }

  function saveRawDsl(dsl: string) {
    rawDslOpen = false;
    scheduleSave(dsl);
  }

  // ── Actions ───────────────────────────────────────────────────────────

  async function triage(row: QueryItem, key: string) {
    const action = triageActionForKey(key);
    if (!action || !row.block_id) return;
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

  function fireSourceOpen() {
    if (!selectedKey || !rootEl) return;
    const btn = rootEl.querySelector(
      `[data-inbox-row="${CSS.escape(selectedKey)}"] [data-action="open-source"]`,
    ) as HTMLElement | null;
    btn?.click();
  }

  // ── Date picker (s key) ───────────────────────────────────────────────
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
      const qc = getAppQueryClient();
      if (qc) await qc.invalidateQueries({ queryKey: ["widget", "inbox"] });
    } catch {
      toast("Failed to schedule", "error");
    }
  }

  // ── Raw-DSL editor sheet ──────────────────────────────────────────────
  let rawDslOpen = $state(false);

  // ── Keyboard handler ──────────────────────────────────────────────────
  function handleKey(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA")) {
      return;
    }
    if (rows.length === 0) return;
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
  onpointerdown={handlePointerDown}
>
  <header class="flex items-center justify-between mb-2 text-[12px]">
    <div class="font-semibold">
      📥 Inbox
      <span class="text-muted-foreground/40 font-normal text-[11px]">
        j/k · ↵ open · t todo · d doing · x done · s schedule
      </span>
    </div>
    <div class="text-muted-foreground/60 text-[11px]">
      {#if totalAvailable > rows.length}
        showing {rows.length} of {totalAvailable}
      {:else}
        {rows.length}
      {/if}
    </div>
  </header>

  <ChipBar
    state={chipState}
    {availableTypes}
    {availableFilters}
    {activeSlug}
    onToggleStatic={toggleChip}
    onToggleType={toggleType}
    onUnhidePage={unhidePage}
    onUnhideBlock={unhideBlock}
    onEditRaw={() => (rawDslOpen = true)}
    onSwitchFilter={switchFilter}
    onSaveAs={saveAsFilter}
  />

  {#if rowsQuery.isLoading}
    <div class="text-muted-foreground/60 text-[12px]">loading…</div>
  {:else if rows.length === 0}
    <div class="text-muted-foreground/50 text-[12px] italic">Inbox clear ✓</div>
  {:else}
    {#each rows as row (rowKey(row))}
      <InboxRow
        {row}
        selected={selectedKey === rowKey(row)}
        onHidePage={hidePage}
      />
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

{#if rawDslOpen}
  <RawDslSheet
    initialDsl={activeDsl}
    onSave={saveRawDsl}
    onCancel={() => (rawDslOpen = false)}
  />
{/if}
