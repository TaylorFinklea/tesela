<script lang="ts">
  /*
   * Prism v4 — Command Station.
   *
   * Full-bleed modal that takes over the main area when ⌘K (or the
   * top-bar command field) fires. Four tabs cycle through Palette /
   * Dashboard / AI / History via ⌘1–⌘4 and the tab strip; the active
   * tab fills the modal so each surface gets room to breathe. The
   * proto's 50/50 Palette + Dashboard landing is reachable via the
   * "split" toggle that pins Dashboard alongside Palette (Phase 4
   * default-on for desktop).
   *
   * Wires:
   *   - ⌘K toggles open from /v4/+layout.svelte (capture-phase)
   *   - Esc closes + restores focus to the prior pane (registry from
   *     Phase 1.5)
   *   - Typing in the search box filters the verb list; Enter runs the
   *     focused command
   *   - Enter / click on a Dashboard widget swaps the prior pane into
   *     a widget pane pointing at that Query note (closes Station)
   */
  import { onMount, untrack } from "svelte";
  import { createQuery } from "@tanstack/svelte-query";
  import { api } from "$lib/api-client";
  import type { Note } from "$lib/types/Note";
  import {
    closeStation,
    getStationInitialQuery,
    getStationPriorPaneId,
    getStationTab,
    isStationOpen,
    setStationTab,
    type StationTab,
  } from "$lib/stores/station.svelte";
  import {
    focusPane,
    getPaneById,
    getPaneOutliner,
    jumpToTile,
    setPaneWidget,
    swapKind,
  } from "$lib/stores/pane-tree.svelte";
  import { buildV4Commands, matchesV4Command, type V4Command } from "$lib/v4/commands";
  import { scoreFuzzy } from "$lib/fuzzy";
  import { parseWidgets, widgetsBySection } from "$lib/widget-registry.svelte";
  import QueryWidgetView from "$lib/components/QueryWidgetView.svelte";

  const open = $derived(isStationOpen());
  const activeTab = $derived(getStationTab());

  // ── search / palette ──────────────────────────────────────────────────────
  let query = $state("");
  let inputEl = $state<HTMLInputElement | undefined>();
  let selectedIdx = $state(0);

  /** Unified palette row. Commands and notes share the same selectable list
   *  so the user can type once and pick either a verb (`vsplit`) or a tile
   *  (a note id / title) by fuzzy match. */
  type CmdRow = { kind: "cmd"; key: string; cmd: V4Command; score: number };
  type NoteRow = { kind: "note"; key: string; note: Note; score: number };
  type PaletteRow = CmdRow | NoteRow;

  const MAX_NOTES_IN_PALETTE = 12;

  const allCommands = buildV4Commands();

  // ── data (notes drive both palette + dashboard) ──────────────────────────
  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: open,
  }));
  const allNotes = $derived((notesQuery.data ?? []) as Note[]);
  const widgets = $derived(parseWidgets(allNotes));
  const grouped = $derived(widgetsBySection(widgets));
  const pinned = $derived(grouped.pinned);

  // Score commands + notes against the query, merge into one ranked list.
  // Empty query shows only commands (notes would be noise without a filter).
  const filteredRows = $derived.by<PaletteRow[]>(() => {
    const q = query.trim();
    const cmdRows: CmdRow[] = !q
      ? allCommands.map((c) => ({ kind: "cmd" as const, key: `c:${c.id}`, cmd: c, score: 0 }))
      : allCommands
          .filter((c) => matchesV4Command(c, q))
          .map((c) => ({
            kind: "cmd" as const,
            key: `c:${c.id}`,
            cmd: c,
            score: Math.max(scoreFuzzy(c.label, q).score, scoreFuzzy(c.verb ?? c.id, q).score),
          }));
    if (!q) return cmdRows;
    const noteRows: NoteRow[] = allNotes
      .map((n) => {
        const titleScore = scoreFuzzy(n.title ?? "", q).score;
        const idScore = scoreFuzzy(n.id, q).score;
        return { kind: "note" as const, key: `n:${n.id}`, note: n, score: Math.max(titleScore, idScore) };
      })
      .filter((r) => r.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, MAX_NOTES_IN_PALETTE);
    // Interleave by score: commands and notes mix, so a precise note title
    // beats a weak partial command match.
    return [...cmdRows, ...noteRows].sort((a, b) => b.score - a.score);
  });

  // ── lifecycle ────────────────────────────────────────────────────────────
  // When the Station opens, seed the search and focus the input. We do this
  // via a $effect that watches `open` flipping true; the inputEl is bound
  // inside the {#if open} block so it only exists while the modal is up.
  $effect(() => {
    if (!open) return;
    const seed = untrack(() => getStationInitialQuery());
    query = seed;
    selectedIdx = 0;
    // Two RAFs to wait for inputEl to mount + measure.
    requestAnimationFrame(() => {
      requestAnimationFrame(() => inputEl?.focus());
    });
  });

  function restoreFocus() {
    const prior = getStationPriorPaneId();
    if (!prior) return;
    const el = getPaneOutliner(prior);
    if (el) el.focus({ preventScroll: true });
    else {
      const hit = getPaneById(prior);
      if (hit) focusPane(hit.row, hit.col);
    }
  }

  function close() {
    closeStation();
    requestAnimationFrame(() => restoreFocus());
  }

  // Modal-local keymap: Esc closes, ⌘1–⌘4 switch tabs, ↑/↓ navigate
  // Palette rows, Enter runs the selected one. ⌘K toggles closed (the
  // layout's capture-phase ⌘K is what opens us; while we're open we
  // intercept it).
  function onKey(e: KeyboardEvent) {
    if (!open) return;
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === "Escape") {
      e.preventDefault();
      close();
      return;
    }
    if (mod && e.key === "k") {
      e.preventDefault();
      close();
      return;
    }
    if (mod && /^[1-4]$/.test(e.key)) {
      e.preventDefault();
      const tab: StationTab = (
        e.key === "1" ? "palette" : e.key === "2" ? "dashboard" : e.key === "3" ? "ai" : "history"
      ) as StationTab;
      setStationTab(tab);
      return;
    }
    if (activeTab !== "palette") return;
    if (e.key === "ArrowDown") {
      e.preventDefault();
      selectedIdx = Math.min(selectedIdx + 1, filteredRows.length - 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      selectedIdx = Math.max(0, selectedIdx - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      void runFocused();
    }
  }

  async function runFocused() {
    const row = filteredRows[selectedIdx];
    if (!row) return;
    if (row.kind === "cmd") await runCommand(row.cmd);
    else openNoteRow(row.note);
  }

  function openNoteRow(note: Note) {
    // Note row → jumpToTile in the prior pane. Same focus-restore dance as
    // runCommand so the user lands on the pane they invoked the Station from.
    const prior = getStationPriorPaneId();
    if (prior) {
      const hit = getPaneById(prior);
      if (hit) focusPane(hit.row, hit.col);
    }
    closeStation();
    jumpToTile(note.id, "palette");
  }

  async function runCommand(cmd: V4Command) {
    // Most verbs operate on the focused pane. Restore that focus FIRST so
    // (e.g.) `vsplit` happens relative to the user's prior pane, not the
    // body. closeStation flips `open` false; the run runs after.
    const prior = getStationPriorPaneId();
    if (prior) {
      const hit = getPaneById(prior);
      if (hit) focusPane(hit.row, hit.col);
    }
    let arg: string | undefined;
    if (cmd.argPrompt) {
      arg = window.prompt(cmd.argPrompt)?.trim();
      if (!arg) return;
    }
    closeStation();
    try {
      await cmd.run(arg);
    } catch (e) {
      console.error("v4: command failed", cmd.id, e);
    }
  }

  function pickWidget(widgetNoteId: string) {
    // Click on a dashboard card swaps the prior pane to a widget pane
    // pointing at that Query note. If no prior pane, jumpToTile to it
    // as a fallback so the user still sees the note open.
    const prior = getStationPriorPaneId();
    if (prior) {
      const hit = getPaneById(prior);
      if (hit) {
        focusPane(hit.row, hit.col);
        swapKind(prior, "widget");
        setPaneWidget(prior, widgetNoteId);
      }
    } else {
      jumpToTile(widgetNoteId);
    }
    closeStation();
  }

  function openTileFromWidget(noteId: string) {
    const prior = getStationPriorPaneId();
    if (prior) {
      const hit = getPaneById(prior);
      if (hit) focusPane(hit.row, hit.col);
    }
    jumpToTile(noteId);
    closeStation();
  }

  onMount(() => {
    document.addEventListener("keydown", onKey, true);
    return () => document.removeEventListener("keydown", onKey, true);
  });

  // Clamp selectedIdx whenever the filtered list shrinks.
  $effect(() => {
    if (selectedIdx >= filteredRows.length) {
      selectedIdx = Math.max(0, filteredRows.length - 1);
    }
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="station-backdrop"
    onclick={(e) => {
      if (e.target === e.currentTarget) close();
    }}
  >
    <div class="station">
      <header class="station-tabs" role="tablist">
        {#each [
          { id: "palette", label: "Palette", chord: "⌘1" },
          { id: "dashboard", label: "Dashboard", chord: "⌘2" },
          { id: "ai", label: "AI", chord: "⌘3" },
          { id: "history", label: "History", chord: "⌘4" },
        ] as t (t.id)}
          <button
            type="button"
            class="station-tab"
            class:active={activeTab === t.id}
            role="tab"
            aria-selected={activeTab === t.id}
            onclick={() => setStationTab(t.id as StationTab)}
          >
            <span class="station-tab-label">{t.label}</span>
            <span class="station-tab-chord">{t.chord}</span>
          </button>
        {/each}
        <span class="station-spacer"></span>
        <button class="station-close" type="button" onclick={close} title="close · Esc">×</button>
      </header>

      {#if activeTab === "palette"}
        <div class="station-palette">
          <input
            bind:this={inputEl}
            bind:value={query}
            class="station-input"
            placeholder="type a command or a note title, then ↵"
            spellcheck={false}
            autocorrect="off"
            autocapitalize="off"
          />
          <div class="station-results" role="listbox">
            {#each filteredRows as row, i (row.key)}
              <button
                type="button"
                role="option"
                aria-selected={i === selectedIdx}
                class="station-row"
                class:active={i === selectedIdx}
                onclick={() => {
                  selectedIdx = i;
                  if (row.kind === "cmd") void runCommand(row.cmd);
                  else openNoteRow(row.note);
                }}
                onmouseenter={() => (selectedIdx = i)}
              >
                {#if row.kind === "cmd"}
                  <span class="station-row-glyph">{row.cmd.glyph}</span>
                  <span class="station-row-label">{row.cmd.label}</span>
                  <span class="station-row-meta">
                    {#if row.cmd.verb}
                      <span class="station-row-verb">:{row.cmd.verb}</span>
                    {/if}
                    {#if row.cmd.shortcut}
                      <span class="station-row-shortcut">{row.cmd.shortcut}</span>
                    {/if}
                  </span>
                {:else}
                  <span class="station-row-glyph station-row-note-mark">≡</span>
                  <span class="station-row-label">{row.note.title || row.note.id}</span>
                  <span class="station-row-meta">
                    <span class="station-row-verb">{row.note.id}</span>
                  </span>
                {/if}
              </button>
            {:else}
              <div class="station-empty">no matches for "{query}"</div>
            {/each}
          </div>
        </div>
      {:else if activeTab === "dashboard"}
        <div class="station-dashboard">
          {#if pinned.length === 0}
            <div class="station-empty">
              <p>no pinned widgets</p>
              <p class="station-empty-hint">
                add <code>section:: pinned</code> to a Query note's frontmatter to surface it here.
              </p>
            </div>
          {:else}
            {#each pinned as w (w.id)}
              <div class="station-dash-card">
                <header class="station-dash-card-head">
                  <span class="station-dash-card-title">{w.title}</span>
                  <button
                    type="button"
                    class="station-dash-card-open"
                    title="open this widget in the focused pane"
                    onclick={() => pickWidget(w.id)}
                  >open in pane</button>
                </header>
                <div class="station-dash-card-body">
                  <QueryWidgetView widget={w} onOpenRow={openTileFromWidget} />
                </div>
              </div>
            {/each}
          {/if}
        </div>
      {:else if activeTab === "ai"}
        <div class="station-ai">
          <p class="station-empty">AI tab · coming soon</p>
          <p class="station-empty-hint">
            this is where an MCP / LLM helper lands. Phase 4 ships the surface;
            wiring follows.
          </p>
        </div>
      {:else if activeTab === "history"}
        <div class="station-history">
          <p class="station-empty">history · Phase 5 wires the Journey trail</p>
          <p class="station-empty-hint">
            recent <kbd>jumpToTile</kbd> calls will show up here once Phase 5
            lands.
          </p>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .station-backdrop {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, var(--v4-bg) 70%, transparent);
    backdrop-filter: blur(10px);
    z-index: 100;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding: 60px 20px 20px;
    animation: v4-fade-in var(--v4-dur-fast) var(--v4-ease-overlay);
  }
  .station {
    width: min(900px, calc(100vw - 40px));
    max-height: calc(100vh - 80px);
    background: var(--v4-bg);
    border: 1px solid var(--v4-hair);
    border-radius: 12px;
    box-shadow: 0 24px 64px rgba(0, 0, 0, 0.5);
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: v4-slide-down var(--v4-dur-base) var(--v4-ease-overlay);
  }

  .station-tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--v4-hair);
  }
  .station-tab {
    background: transparent;
    border: 0;
    color: var(--v4-ink4);
    padding: 6px 12px;
    font-family: var(--v4-sans);
    font-size: 12.5px;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .station-tab.active {
    color: var(--v4-ink);
    background: var(--v4-surface-lo);
  }
  .station-tab-chord {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink5);
  }
  .station-tab.active .station-tab-chord {
    color: var(--v4-accent);
  }
  .station-spacer { flex: 1; }
  .station-close {
    background: transparent;
    border: 0;
    color: var(--v4-ink5);
    font-size: 16px;
    line-height: 1;
    padding: 4px 8px;
    cursor: pointer;
    border-radius: 5px;
  }
  .station-close:hover { color: var(--v4-ink2); background: var(--v4-surface-lo); }

  .station-palette {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .station-input {
    border: 0;
    background: transparent;
    color: var(--v4-ink);
    font-family: var(--v4-sans);
    font-size: 16px;
    padding: 16px 18px;
    outline: none;
    border-bottom: 1px solid var(--v4-hair);
  }
  .station-input::placeholder {
    color: var(--v4-ink6);
  }
  .station-results {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 6px 6px 12px;
  }
  .station-row {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 12px;
    background: transparent;
    border: 0;
    border-radius: 7px;
    color: var(--v4-ink2);
    font-family: var(--v4-sans);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
  }
  .station-row.active {
    background: rgba(123, 140, 255, 0.10);
    color: var(--v4-ink);
  }
  .station-row-glyph {
    font-family: var(--v4-mono);
    font-size: 14px;
    color: var(--v4-accent);
    width: 16px;
    text-align: center;
    flex-shrink: 0;
  }
  /* Note rows use a calmer glyph color so verbs visually anchor the list. */
  .station-row-note-mark { color: var(--v4-ink5); }
  .station-row-label { flex: 1; }
  .station-row-meta {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-shrink: 0;
  }
  .station-row-verb,
  .station-row-shortcut {
    font-family: var(--v4-mono);
    font-size: 10.5px;
    color: var(--v4-ink5);
  }

  .station-dashboard {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 14px;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    grid-auto-rows: 320px;
    gap: 12px;
    align-content: start;
  }
  .station-dash-card {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--v4-hair);
    border-radius: 9px;
    overflow: hidden;
    background: var(--v4-surface-lo);
  }
  .station-dash-card-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--v4-hair);
    flex-shrink: 0;
  }
  .station-dash-card-title {
    font-family: var(--v4-sans);
    font-size: 12.5px;
    color: var(--v4-ink2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .station-dash-card-open {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 10px;
    padding: 2px 8px;
    border-radius: 5px;
    cursor: pointer;
  }
  .station-dash-card-open:hover { color: var(--v4-ink2); border-color: var(--v4-hair2); }
  .station-dash-card-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 6px 8px;
  }

  .station-ai,
  .station-history {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 40px;
  }
  .station-empty {
    color: var(--v4-ink5);
    font-family: var(--v4-mono);
    font-size: 12px;
    text-align: center;
    padding: 30px 12px;
  }
  .station-empty-hint {
    color: var(--v4-ink6);
    font-size: 11px;
  }
  .station-empty-hint code,
  .station-empty-hint kbd {
    font-family: var(--v4-mono);
    background: var(--v4-surface-lo);
    padding: 1px 5px;
    border-radius: 4px;
    color: var(--v4-ink2);
  }
</style>
