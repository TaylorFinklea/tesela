<!-- web/src/lib/graphite/shell/GrCommandPalette.svelte -->
<script lang="ts">
  /*
   * Graphite ⌘K command palette — NEW presentation over the EXISTING
   * command registry + station behavior. This mirrors
   * web/src/lib/components/v4/Station.svelte's palette tab exactly:
   *   - open/close state from the station store (isStationOpen / closeStation
   *     / getStationInitialQuery / getStationPriorPaneId)
   *   - the command rows are buildV4Commands() (the same V4Command set the
   *     real ⌘K runs), filtered with matchesV4Command + ranked with
   *     scoreFuzzy from $lib/fuzzy
   *   - notes are fetched + ranked the same way the Station does (createQuery
   *     + scoreFuzzy on title/id), interleaved by score
   *   - exec restores prior pane focus FIRST, prompts for argPrompt verbs,
   *     then runs cmd.run(arg) / opens the note page — identical to the
   *     Station's runCommand / openNoteRow
   * Only the markup + CSS (the mockup's `.gr-cmdk`) is new.
   */
  import { onMount, untrack } from 'svelte';
  import { createQuery } from '@tanstack/svelte-query';
  import { api } from '$lib/api-client';
  import type { Note } from '$lib/types/Note';
  import GrIcon from '$lib/graphite/GrIcon.svelte';
  import {
    isStationOpen,
    closeStation,
    getStationInitialQuery,
    getStationPriorPaneId,
  } from '$lib/stores/station.svelte';
  import { focusLeaf, openPageInFocused } from '$lib/buffer/state.svelte';
  import { asPageId, type LeafId } from '$lib/buffer/types';
  import { buildV4Commands, matchesV4Command, type V4Command } from '$lib/v4/commands';
  import { scoreFuzzy, highlightRuns } from '$lib/fuzzy';

  const open = $derived(isStationOpen());

  let query = $state('');
  let inputEl = $state<HTMLInputElement | undefined>();
  let selectedIdx = $state(0);

  type CmdRow = { kind: 'cmd'; key: string; cmd: V4Command; score: number };
  type NoteRow = { kind: 'note'; key: string; note: Note; score: number };
  type PaletteRow = CmdRow | NoteRow;

  const MAX_NOTES_IN_PALETTE = 12;

  const allCommands = buildV4Commands();

  const notesQuery = createQuery(() => ({
    queryKey: ['notes', { limit: 500 }] as const,
    queryFn: () => api.listNotes({ limit: 500 }),
    enabled: open,
  }));
  const allNotes = $derived((notesQuery.data ?? []) as Note[]);

  // Flat, score-ranked row list — mirrors Station.filteredRows.
  const filteredRows = $derived.by<PaletteRow[]>(() => {
    const q = query.trim();
    const cmdRows: CmdRow[] = !q
      ? allCommands.map((c) => ({ kind: 'cmd' as const, key: `c:${c.id}`, cmd: c, score: 0 }))
      : allCommands
          .filter((c) => matchesV4Command(c, q))
          .map((c) => ({
            kind: 'cmd' as const,
            key: `c:${c.id}`,
            cmd: c,
            score: Math.max(scoreFuzzy(c.label, q).score, scoreFuzzy(c.verb ?? c.id, q).score),
          }));
    if (!q) return cmdRows;
    const noteRows: NoteRow[] = allNotes
      .map((n) => {
        const titleScore = scoreFuzzy(n.title ?? '', q).score;
        const idScore = scoreFuzzy(n.id, q).score;
        return { kind: 'note' as const, key: `n:${n.id}`, note: n, score: Math.max(titleScore, idScore) };
      })
      .filter((r) => r.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, MAX_NOTES_IN_PALETTE);
    return [...cmdRows, ...noteRows].sort((a, b) => b.score - a.score);
  });

  // Group the flat list for the `.gr-cmdk-grp` sections, preserving rank
  // order within each group. "Jump to" = pages/navigation; "Actions" = the
  // rest of the verbs.
  type Group = { label: string; rows: { row: PaletteRow; idx: number }[] };
  const groups = $derived.by<Group[]>(() => {
    const jumpRaw: PaletteRow[] = [];
    const actionsRaw: PaletteRow[] = [];
    for (const row of filteredRows) {
      const isJump =
        row.kind === 'note' ||
        (row.kind === 'cmd' && (row.cmd.category === 'navigate' || row.cmd.category === 'tile'));
      (isJump ? jumpRaw : actionsRaw).push(row);
    }
    // Number rows in DISPLAY order (jump group, then actions) so the ⌘N quick-
    // select badges, the j/k selection highlight, and ⌘N quick-run all agree
    // with what's on screen. `displayRows` (below) is this same flat order, and
    // is the index space `selectedIdx` lives in.
    let di = 0;
    const jump = jumpRaw.map((row) => ({ row, idx: di++ }));
    const actions = actionsRaw.map((row) => ({ row, idx: di++ }));
    const out: Group[] = [];
    if (jump.length) out.push({ label: 'Jump to', rows: jump });
    if (actions.length) out.push({ label: 'Actions', rows: actions });
    return out;
  });
  // Flat row list in DISPLAY order — the index space `selectedIdx`, the ⌘N
  // badges, and the keyboard nav all share.
  const displayRows = $derived(groups.flatMap((g) => g.rows.map((r) => r.row)));

  function rowLabel(row: PaletteRow): string {
    return row.kind === 'cmd' ? row.cmd.label : (row.note.title || row.note.id);
  }
  function rowGlyph(row: PaletteRow): string {
    return row.kind === 'cmd' ? row.cmd.glyph : '→';
  }
  function rowShortcut(row: PaletteRow): string | undefined {
    return row.kind === 'cmd' ? row.cmd.shortcut : undefined;
  }

  // Seed query + focus input when the palette opens (mirrors Station's effect).
  $effect(() => {
    if (!open) return;
    const seed = untrack(() => getStationInitialQuery());
    query = seed;
    selectedIdx = 0;
    requestAnimationFrame(() => {
      requestAnimationFrame(() => inputEl?.focus());
    });
  });

  // Re-pin selection to the top row on each keystroke.
  $effect(() => {
    query;
    selectedIdx = 0;
  });

  // Clamp selection when the list shrinks.
  $effect(() => {
    if (selectedIdx >= filteredRows.length) {
      selectedIdx = Math.max(0, filteredRows.length - 1);
    }
  });

  function restoreFocus() {
    const prior = getStationPriorPaneId();
    if (prior) focusLeaf(prior as LeafId);
  }

  async function runCommand(cmd: V4Command) {
    // Most verbs operate on the focused pane — restore that focus first.
    restoreFocus();
    let arg: string | undefined;
    if (cmd.argPrompt) {
      arg = window.prompt(cmd.argPrompt)?.trim();
      if (!arg) return;
    }
    closeStation();
    try {
      await cmd.run(arg);
    } catch (e) {
      console.error('graphite: command failed', cmd.id, e);
    }
  }

  function openNoteRow(note: Note) {
    closeStation();
    restoreFocus();
    openPageInFocused(asPageId(note.id));
  }

  async function runRow(row: PaletteRow) {
    if (row.kind === 'cmd') await runCommand(row.cmd);
    else openNoteRow(row.note);
  }

  async function runSelected() {
    const row = displayRows[selectedIdx];
    if (row) await runRow(row);
  }

  function onKey(e: KeyboardEvent) {
    if (!open) return;
    if (e.key === 'Escape') {
      e.preventDefault();
      closeStation();
      requestAnimationFrame(() => restoreFocus());
      return;
    }
    if (e.metaKey && e.key === 'k') {
      e.preventDefault();
      closeStation();
      requestAnimationFrame(() => restoreFocus());
      return;
    }
    // Ctrl-j / Ctrl-k navigate the list (vim-style), mirroring Arrow Down/Up.
    if (e.ctrlKey && (e.key === 'j' || e.key === 'k')) {
      e.preventDefault();
      selectedIdx =
        e.key === 'j'
          ? Math.min(selectedIdx + 1, filteredRows.length - 1)
          : Math.max(0, selectedIdx - 1);
      return;
    }
    // ⌘1..⌘9 (or Ctrl-1..9) jump to + run the Nth visible row — matches the
    // quick-select badges. Only active while the palette is open.
    if ((e.metaKey || e.ctrlKey) && e.key >= '1' && e.key <= '9') {
      const n = Number(e.key) - 1;
      if (n < filteredRows.length) {
        e.preventDefault();
        selectedIdx = n;
        void runSelected();
      }
      return;
    }
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      selectedIdx = Math.min(selectedIdx + 1, filteredRows.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      selectedIdx = Math.max(0, selectedIdx - 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      void runSelected();
    }
  }

  onMount(() => {
    document.addEventListener('keydown', onKey, true);
    return () => document.removeEventListener('keydown', onKey, true);
  });
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="gr-scrim" onclick={() => { closeStation(); requestAnimationFrame(() => restoreFocus()); }}>
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="gr-cmdk" onclick={(e) => e.stopPropagation()}>
      <div class="gr-cmdk-in">
        <GrIcon name="search" size={18} />
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:this={inputEl}
          bind:value={query}
          class="gr-cmdk-input"
          placeholder="Search or run a command…"
          autocomplete="off"
          spellcheck="false"
        />
      </div>
      <div class="gr-cmdk-body">
        {#each groups as group (group.label)}
          <div class="gr-cmdk-grp">{group.label}</div>
          {#each group.rows as { row, idx } (row.key)}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="gr-cmdk-row"
              class:sel={idx === selectedIdx}
              onmouseenter={() => (selectedIdx = idx)}
              onclick={() => void runRow(row)}
            >
              <span class="gl">{rowGlyph(row)}</span>
              <span class="lb">
                {#each highlightRuns(rowLabel(row), query.trim() ? scoreFuzzy(rowLabel(row), query.trim()).positions : []) as run}
                  {#if run.match}<b>{run.ch}</b>{:else}{run.ch}{/if}
                {/each}
              </span>
              <span class="rk">
                {#if idx < 9}
                  <kbd>⌘{idx + 1}</kbd>
                {/if}
                {#if rowShortcut(row)}
                  <kbd>{rowShortcut(row)}</kbd>
                {/if}
              </span>
            </div>
          {/each}
        {/each}
        {#if filteredRows.length === 0}
          <div class="gr-cmdk-empty">No matches</div>
        {/if}
      </div>
      <div class="gr-cmdk-foot">
        <span><kbd>↑↓</kbd> navigate</span>
        <span><kbd>↵</kbd> run</span>
        <span><kbd>esc</kbd> close</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .gr-scrim {
    position: absolute;
    inset: 0;
    z-index: 40;
    background: rgba(8, 9, 12, 0.58);
    backdrop-filter: blur(3px);
    display: flex;
    flex-direction: column;
    align-items: center;
  }
  .gr-cmdk {
    width: min(640px, 92%);
    margin-top: 72px;
    background: var(--raised);
    border: 1px solid var(--line-2);
    border-radius: 14px;
    box-shadow: 0 28px 90px rgba(0, 0, 0, 0.55);
    overflow: hidden;
  }
  .gr-cmdk-in {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 18px;
    border-bottom: 1px solid var(--line);
    color: var(--subtle);
  }
  .gr-cmdk-input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    color: var(--fg);
    font-family: var(--sans);
    font-size: 15px;
  }
  .gr-cmdk-input::placeholder {
    color: var(--faint);
  }
  .gr-cmdk-body {
    padding: 8px;
    max-height: 430px;
    overflow: auto;
  }
  .gr-cmdk-grp {
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
    padding: 9px 12px 5px;
  }
  .gr-cmdk-row {
    display: grid;
    grid-template-columns: 26px 1fr auto;
    align-items: center;
    gap: 13px;
    padding: 10px 12px;
    border-radius: 9px;
    cursor: pointer;
  }
  .gr-cmdk-row.sel {
    background: var(--raised-3);
  }
  .gr-cmdk-row .gl {
    display: grid;
    place-items: center;
    color: var(--subtle);
    font-size: 14px;
  }
  .gr-cmdk-row .lb {
    font-size: 13.5px;
    color: var(--fg2);
    display: flex;
    align-items: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .gr-cmdk-row .lb b {
    color: var(--fg);
    font-weight: 600;
  }
  .gr-cmdk-row .rk {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
    display: flex;
    gap: 4px;
  }
  .gr-cmdk-row .rk kbd {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 2px 6px;
  }
  .gr-cmdk-empty {
    padding: 18px 12px;
    text-align: center;
    color: var(--faint);
    font-size: 13px;
  }
  .gr-cmdk-foot {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 16px;
    border-top: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--faint);
  }
  .gr-cmdk-foot kbd {
    color: var(--fg2);
  }
</style>
