<!-- web/src/lib/graphite/views/GrInbox.svelte — the Views surface
     (saved-views spec 2026-06-10: views ARE the Inbox).

     The inbox pane is now a saved-views switcher over the synced registry
     (`GET /views`; the seeded builtin Inbox is the default selection). The
     selected view's DSL runs through the SAME pipeline as before
     (api.executeQuery → kind==="block" rows, capped) in list mode, with
     the triage verbs (t/d/x keys, snooze, Process all) unchanged;
     `display_mode` table/kanban mounts QueryWidgetView over a synthetic
     widget (the post-7cf456d GrPage pattern), which itself falls back to
     table when a kanban query isn't tag-scoped.

     Editing is DSL-first (product-locked): the editor row has a monospace
     query input with live validation (the TS parser mirror of the
     server's validate_dsl — the server 400s as backstop), key
     autocomplete (base keys + property-registry names), and the existing
     inbox chips RE-POINTED AS INSERTERS that toggle their DSL fragments
     into the string (view-dsl.ts owns the pure logic). Save/rename/
     delete (builtin delete disabled with the server's message)/reorder
     (↑↓ → POST /views/reorder) round out CRUD. The `views_changed` WS
     event seeds the ["views"] cache from the layout, so edits on another
     device show up live. Shared modules imported READ-ONLY. -->
<script lang="ts">
  import { tick } from "svelte";
  import { createQuery } from "@tanstack/svelte-query";
  import { api, ApiError, type ViewRecord } from "$lib/api-client";
  import { getAppQueryClient } from "$lib/app-query-client.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import {
    applyTriage,
    triageActionForKey,
    type TriageAction,
  } from "$lib/triage.svelte";
  import { CHIP_REGISTRY, type ChipDef } from "$lib/ambients/inbox/chips";
  import {
    validateViewDsl,
    toggleClausesInDsl,
    clausesActiveInDsl,
    dslKeySuggestions,
    applyDslSuggestion,
    type DslSuggestion,
  } from "$lib/views/view-dsl";
  import { INBOX_VIEW_DSL } from "$lib/query-language";
  import type { QueryItem } from "$lib/types/QueryItem";
  import type { Widget } from "$lib/types/Widget";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  import QueryWidgetView from "$lib/components/QueryWidgetView.svelte";
  import GrChip from "$lib/graphite/GrChip.svelte";
  import GrButton from "$lib/graphite/GrButton.svelte";
  import GrIcon from "$lib/graphite/GrIcon.svelte";

  /** Mirrors the server's builtin-delete refusal (routes/views.rs). */
  const BUILTIN_DELETE_MSG =
    "builtin and cannot be deleted — builtins are editable; reset it to its default instead";
  const DISPLAY_MODES = ["list", "table", "kanban"] as const;
  type DisplayMode = (typeof DISPLAY_MODES)[number];

  // ── views registry ─────────────────────────────────────────────────────
  const viewsQuery = createQuery(() => ({
    queryKey: ["views"] as const,
    queryFn: () => api.listViews(),
  }));
  const views = $derived<ViewRecord[]>(viewsQuery.data ?? []);

  // Selected view, sticky across reloads. Falls back builtin → first when
  // the stored id no longer exists (deleted on another device).
  const SELECTED_KEY = "tesela:graphite:views-selected";
  function loadSelected(): string {
    if (typeof localStorage === "undefined") return "builtin-inbox";
    return localStorage.getItem(SELECTED_KEY) ?? "builtin-inbox";
  }
  let selectedId = $state<string>(loadSelected());
  const selected = $derived<ViewRecord | null>(
    views.find((v) => v.id === selectedId) ??
      views.find((v) => v.builtin) ??
      views[0] ??
      null,
  );
  function selectView(id: string) {
    selectedId = id;
    try {
      localStorage.setItem(SELECTED_KEY, id);
    } catch {
      /* private mode etc. — non-fatal */
    }
  }

  const activeDsl = $derived(selected?.dsl ?? "");
  const displayMode = $derived<DisplayMode>(
    selected?.display_mode === "table" || selected?.display_mode === "kanban"
      ? selected.display_mode
      : "list",
  );

  // table/kanban mode → QueryWidgetView over a synthetic widget (the same
  // shape widgetFromNote builds for Query pages). The id varies by mode so
  // QWV's per-widget localStorage view pref can't pin a stale mode over
  // the view's saved display_mode.
  const modeWidget = $derived<Widget | null>(
    selected && displayMode !== "list"
      ? {
          id: `view:${selected.id}:${displayMode}`,
          title: selected.name,
          query: selected.dsl,
          group: selected.display_group_by,
          sort: null,
          icon: null,
          color: null,
          section: "saved",
          view: displayMode,
          system: false,
          // ya4.1 — marks this widget as a saved-view mount so KanbanBoard's
          // group-by resolution decision 3a fires and group-by changes
          // round-trip through `updateView` instead of localStorage.
          viewId: selected.id,
        }
      : null,
  );

  // ── result rows (list mode — the triage pipeline, unchanged) ──────────
  const rowsQuery = createQuery(() => ({
    queryKey: ["widget", "inbox", activeDsl] as const,
    queryFn: () => api.executeQuery(activeDsl),
    enabled: displayMode === "list" && activeDsl.length > 0,
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

  // ── editor (DSL-first; chips as inserters) ─────────────────────────────
  type EditorState = {
    /** null = creating a new view. */
    id: string | null;
    builtin: boolean;
    name: string;
    dsl: string;
    displayMode: DisplayMode;
    serverError: string | null;
    saving: boolean;
  };
  let editor = $state<EditorState | null>(null);
  const draftError = $derived(editor ? validateViewDsl(editor.dsl) : null);
  const canSave = $derived(
    editor !== null &&
      !editor.saving &&
      editor.name.trim().length > 0 &&
      draftError === null,
  );
  const editedIndex = $derived.by(() => {
    const id = editor?.id;
    return id ? views.findIndex((v) => v.id === id) : -1;
  });

  function openEditor(v: ViewRecord) {
    editor = {
      id: v.id,
      builtin: v.builtin,
      name: v.name,
      dsl: v.dsl,
      displayMode: (DISPLAY_MODES as readonly string[]).includes(v.display_mode)
        ? (v.display_mode as DisplayMode)
        : "list",
      serverError: null,
      saving: false,
    };
    suggest = null;
  }
  function openNewEditor() {
    editor = {
      id: null,
      builtin: false,
      name: "",
      dsl: "",
      displayMode: "list",
      serverError: null,
      saving: false,
    };
    suggest = null;
  }
  function closeEditor() {
    editor = null;
    suggest = null;
  }

  function apiErrorMessage(e: unknown): string {
    if (e instanceof ApiError) {
      try {
        const j = JSON.parse(e.body) as { error?: unknown };
        if (j && typeof j.error === "string") return j.error;
      } catch {
        /* non-JSON body — fall through */
      }
      return e.body || `HTTP ${e.status}`;
    }
    return e instanceof Error ? e.message : String(e);
  }

  async function invalidateViews() {
    const qc = getAppQueryClient();
    if (qc) await qc.invalidateQueries({ queryKey: ["views"] });
  }

  async function saveEditor() {
    if (!editor || !canSave) return;
    editor.serverError = null;
    editor.saving = true;
    try {
      let saved: ViewRecord;
      if (editor.id) {
        saved = await api.updateView(editor.id, {
          name: editor.name.trim(),
          dsl: editor.dsl.trim(),
          display_mode: editor.displayMode,
        });
      } else {
        saved = await api.createView({
          name: editor.name.trim(),
          dsl: editor.dsl.trim(),
          display_mode: editor.displayMode,
        });
      }
      await invalidateViews();
      selectView(saved.id);
      closeEditor();
    } catch (e) {
      if (editor) editor.serverError = apiErrorMessage(e);
    } finally {
      if (editor) editor.saving = false;
    }
  }

  async function deleteEdited() {
    if (!editor?.id || editor.builtin) return;
    if (!window.confirm(`Delete view "${editor.name}"?`)) return;
    try {
      await api.deleteView(editor.id);
      await invalidateViews();
      if (selectedId === editor.id) selectView("builtin-inbox");
      closeEditor();
    } catch (e) {
      if (editor) editor.serverError = apiErrorMessage(e);
    }
  }

  /** Move the edited view one slot up/down → POST /views/reorder with the
   *  full id list (the server reassigns order = idx*10). */
  async function moveEdited(delta: -1 | 1) {
    if (!editor?.id) return;
    const ids = views.map((v) => v.id);
    const i = ids.indexOf(editor.id);
    const j = i + delta;
    if (i < 0 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    try {
      await api.reorderViews(ids);
      await invalidateViews();
    } catch (e) {
      if (editor) editor.serverError = apiErrorMessage(e);
    }
  }

  function insertChip(chip: ChipDef) {
    if (!editor) return;
    editor.dsl = toggleClausesInDsl(editor.dsl, chip.clauses);
    suggest = null;
    dslInputEl?.focus();
  }

  /** Reset a builtin's draft to its shipped default (Inbox only today). */
  function resetBuiltinDraft() {
    if (!editor?.builtin) return;
    editor.dsl = INBOX_VIEW_DSL;
    suggest = null;
  }

  // ── DSL key autocomplete (cheap version: keys only) ────────────────────
  const propsQuery = createQuery(() => ({
    queryKey: ["properties"] as const,
    queryFn: () => api.listProperties(),
    enabled: editor !== null,
  }));
  const propertyKeys = $derived((propsQuery.data ?? []).map((p) => p.name));

  let dslInputEl = $state<HTMLInputElement | undefined>();
  let suggest = $state<DslSuggestion | null>(null);
  let suggestIndex = $state(0);

  function refreshSuggest() {
    if (!editor || !dslInputEl) {
      suggest = null;
      return;
    }
    const cursor = dslInputEl.selectionStart ?? editor.dsl.length;
    suggest = dslKeySuggestions(editor.dsl, cursor, propertyKeys);
    suggestIndex = 0;
  }

  async function acceptSuggestion(item: string) {
    if (!editor || !suggest) return;
    const applied = applyDslSuggestion(editor.dsl, suggest, item);
    editor.dsl = applied.dsl;
    suggest = null;
    await tick();
    dslInputEl?.focus();
    dslInputEl?.setSelectionRange(applied.cursor, applied.cursor);
  }

  function dslKeydown(e: KeyboardEvent) {
    if (suggest && suggest.items.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        suggestIndex = (suggestIndex + 1) % suggest.items.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        suggestIndex =
          (suggestIndex - 1 + suggest.items.length) % suggest.items.length;
        return;
      }
      if (e.key === "Tab" || e.key === "Enter") {
        e.preventDefault();
        void acceptSuggestion(suggest.items[suggestIndex]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        suggest = null;
        return;
      }
    }
    if (e.key === "Enter" && canSave) {
      e.preventDefault();
      void saveEditor();
    }
  }

  // ── selection / keyboard nav (list mode — unchanged triage surface) ────
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
    const step = () => {
      if (cancelled || !rootEl) return;
      if (!rootEl.contains(document.activeElement)) {
        rootEl.focus({ preventScroll: true });
      }
      elapsed += 50;
      if (elapsed > 500) return;
      setTimeout(step, 50);
    };
    const start = setTimeout(step, 0);
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
    // Track per-row outcomes. applyTriage signals failure two ways — it
    // throws (network / concurrent edit) OR returns false (block not found /
    // stale row). Count both so a partial failure is reported, not silently
    // dropped (the single-row triage()/snooze() already toast on failure).
    let ok = 0;
    let failed = 0;
    const total = rows.filter((r) => r.block_id).length;
    for (const row of rows) {
      if (!row.block_id) continue;
      try {
        if (await applyTriage(row.page_id, row.block_id, "todo")) ok++;
        else failed++;
      } catch (e) {
        failed++;
        console.warn("processAll: triage failed for", row.block_id, e);
      }
    }
    if (qc) await qc.invalidateQueries({ queryKey: ["widget", "inbox"] });
    if (failed > 0) toast(`Triaged ${ok} of ${total} items; ${failed} failed`, "error");
  }

  function openSource(row: QueryItem) {
    openPageInFocused(asPageId(row.page_id));
  }

  function openRef(pageId: string) {
    openPageInFocused(asPageId(pageId));
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
    <span class="ttl">Views</span>
    <span class="sp"></span>
    {#if displayMode === "list"}
      <span class="meta">{rows.length}</span>
      <GrButton variant="cta" onclick={() => void processAll()}>Process all</GrButton>
    {/if}
  </div>

  <!-- View switcher: the registry's ordered views as a segmented control
       (the GrAgenda List|Week idiom, wrapping); pencil edits the active
       view; + creates a new one. -->
  <div class="gr-views-bar">
    <div class="gr-views-switch">
      {#if viewsQuery.isLoading}
        <span class="gr-views-loading">loading views…</span>
      {:else}
        {#each views as v (v.id)}
          <button
            type="button"
            class:active={selected?.id === v.id}
            title={v.dsl}
            onclick={() => selectView(v.id)}
          >{v.name}</button>
        {/each}
      {/if}
    </div>
    {#if selected}
      <GrButton
        icon="pencil"
        title={`Edit view "${selected.name}"`}
        onclick={() => selected && openEditor(selected)}
      />
    {/if}
    <GrButton icon="plus" title="New view" onclick={openNewEditor} />
  </div>

  {#if editor}
    <div class="gr-vedit">
      <div class="gr-vedit-row">
        <input
          class="gr-vname"
          type="text"
          placeholder="View name"
          bind:value={editor.name}
        />
        <div class="gr-vmodes" title="Display mode">
          {#each DISPLAY_MODES as m (m)}
            <button
              type="button"
              class:active={editor.displayMode === m}
              onclick={() => {
                if (editor) editor.displayMode = m;
              }}
            >{m}</button>
          {/each}
        </div>
        <span class="sp"></span>
        {#if editor.id}
          <GrButton
            icon="arrow-up"
            title="Move up"
            disabled={editedIndex <= 0}
            onclick={() => void moveEdited(-1)}
          />
          <GrButton
            icon="arrow-down"
            title="Move down"
            disabled={editedIndex < 0 || editedIndex >= views.length - 1}
            onclick={() => void moveEdited(1)}
          />
          {#if editor.builtin}
            <GrButton
              icon="restore"
              title="Reset the query to the Inbox default"
              onclick={resetBuiltinDraft}
            />
          {/if}
          <span title={editor.builtin ? BUILTIN_DELETE_MSG : "Delete view"}>
            <GrButton
              icon="trash"
              disabled={editor.builtin}
              onclick={() => void deleteEdited()}
            />
          </span>
        {/if}
      </div>

      <div class="gr-vdsl-wrap">
        <input
          class="gr-vdsl"
          type="text"
          spellcheck="false"
          autocomplete="off"
          placeholder="status = todo AND type = project AND scheduled IS NULL"
          bind:this={dslInputEl}
          bind:value={editor.dsl}
          oninput={refreshSuggest}
          onclick={refreshSuggest}
          onkeydown={dslKeydown}
          onblur={() => (suggest = null)}
        />
        {#if suggest && suggest.items.length > 0}
          <div class="gr-vsuggest">
            {#each suggest.items as item, i (item)}
              <button
                type="button"
                class:hl={i === suggestIndex}
                onmousedown={(e) => {
                  e.preventDefault();
                  void acceptSuggestion(item);
                }}
              >{item}</button>
            {/each}
          </div>
        {/if}
      </div>
      {#if editor.dsl.trim().length > 0 && draftError}
        <div class="gr-verr">{draftError}</div>
      {/if}
      {#if editor.serverError}
        <div class="gr-verr">{editor.serverError}</div>
      {/if}

      <!-- The inbox chips, re-pointed as one-tap INSERTERS into the DSL
           string: active when every clause is present; tapping toggles
           the fragment in/out of the draft. -->
      <div class="gr-vchips">
        {#each CHIP_REGISTRY as chip (chip.id)}
          <span class="gr-chip-wrap" title={chip.hint}>
            <GrChip
              active={clausesActiveInDsl(editor.dsl, chip.clauses)}
              onclick={() => insertChip(chip)}
            >
              {chip.glyph} {chip.label}
            </GrChip>
          </span>
        {/each}
      </div>

      <div class="gr-vedit-actions">
        <GrButton variant="cta" disabled={!canSave} onclick={() => void saveEditor()}>
          {editor.id ? "Save" : "Create"}
        </GrButton>
        <GrButton onclick={closeEditor}>Cancel</GrButton>
      </div>
    </div>
  {/if}

  {#if displayMode !== "list" && modeWidget}
    <!-- table / kanban — the same QueryWidgetView Query pages use on /g. -->
    <div class="gr-views-qwv">
      {#key modeWidget.id}
        <QueryWidgetView widget={modeWidget} onOpenRow={(pageId) => openRef(pageId)} />
      {/key}
    </div>
  {:else}
    <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
    <div
      bind:this={rootEl}
      class="gr-inbox-body"
      tabindex="0"
      onkeydown={handleKey}
    >
      {#if viewsQuery.isLoading || rowsQuery.isLoading}
        <div class="gr-empty">loading…</div>
      {:else if rows.length === 0}
        <div class="gr-empty">{selected?.builtin ? "Inbox clear ✓" : "No matches"}</div>
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
  {/if}
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

  /* View switcher row (segmented-control idiom from GrAgenda's
   * List|Week toggle, wrapping for 6–12 views). */
  .gr-views-bar {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 11px 18px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }
  .gr-views-switch {
    display: inline-flex;
    flex-wrap: wrap;
    border: 1px solid var(--line-2);
    border-radius: 7px;
    overflow: hidden;
  }
  .gr-views-switch button {
    appearance: none;
    background: transparent;
    border: none;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--subtle);
    padding: 4px 10px;
    cursor: pointer;
    white-space: nowrap;
  }
  .gr-views-switch button + button {
    border-left: 1px solid var(--line-2);
  }
  .gr-views-switch button:hover {
    color: var(--fg);
  }
  .gr-views-switch button.active {
    background: var(--raised-2);
    color: var(--fg);
  }
  .gr-views-loading {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
    padding: 4px 10px;
  }
  .gr-views-bar :global(.gr-btn:last-child) {
    margin-left: auto;
  }

  /* Inline view editor. */
  .gr-vedit {
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 12px 18px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
    flex-shrink: 0;
  }
  .gr-vedit-row {
    display: flex;
    align-items: center;
    gap: 7px;
  }
  .gr-vedit-row .sp {
    flex: 1;
  }
  .gr-vname {
    width: 200px;
    height: 28px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--raised);
    border: 1px solid var(--line-2);
    color: var(--fg);
    font-size: 12.5px;
    font-family: var(--sans);
    outline: none;
  }
  .gr-vname:focus {
    border-color: var(--coral-line);
  }
  .gr-vmodes {
    display: inline-flex;
    border: 1px solid var(--line-2);
    border-radius: 7px;
    overflow: hidden;
    flex-shrink: 0;
  }
  .gr-vmodes button {
    appearance: none;
    background: transparent;
    border: none;
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--subtle);
    padding: 3px 9px;
    cursor: pointer;
  }
  .gr-vmodes button + button {
    border-left: 1px solid var(--line-2);
  }
  .gr-vmodes button:hover {
    color: var(--fg);
  }
  .gr-vmodes button.active {
    background: var(--raised-2);
    color: var(--fg);
  }
  .gr-vdsl-wrap {
    position: relative;
  }
  .gr-vdsl {
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border-radius: 8px;
    background: var(--raised);
    border: 1px solid var(--line-2);
    color: var(--fg);
    font-family: var(--mono);
    font-size: 12px;
    outline: none;
  }
  .gr-vdsl:focus {
    border-color: var(--coral-line);
  }
  .gr-vsuggest {
    position: absolute;
    top: 32px;
    left: 0;
    z-index: 20;
    display: flex;
    flex-direction: column;
    min-width: 160px;
    background: var(--raised);
    border: 1px solid var(--line-2);
    border-radius: 8px;
    overflow: hidden;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  }
  .gr-vsuggest button {
    appearance: none;
    background: transparent;
    border: none;
    text-align: left;
    font-family: var(--mono);
    font-size: 11.5px;
    color: var(--fg2);
    padding: 5px 10px;
    cursor: pointer;
  }
  .gr-vsuggest button:hover,
  .gr-vsuggest button.hl {
    background: var(--raised-2);
    color: var(--fg);
  }
  .gr-verr {
    font-family: var(--mono);
    font-size: 11px;
    color: var(--task);
  }
  .gr-vchips {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-wrap: wrap;
  }
  .gr-chip-wrap {
    display: inline-flex;
  }
  .gr-vedit-actions {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  /* table / kanban host (QueryWidgetView). */
  .gr-views-qwv {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px 18px;
  }

  /* Result list + cards (verbatim Graphite CSS). */
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
