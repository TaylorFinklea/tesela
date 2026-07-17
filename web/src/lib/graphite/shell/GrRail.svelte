<!-- web/src/lib/graphite/shell/GrRail.svelte -->
<script lang="ts">
  /*
   * Graphite widget rail — the left-hand widget host. New presentation
   * over existing stores:
   *   - Quick capture → openColonMode() (same verb the `:` keypress and
   *     v4's capture affordance use)
   *   - Favorites → the localStorage-backed favorites store
   *   - Pinned → getPinned() from the LIVE v5 workspace store
   *   - Today → getRecent() from the LIVE v5 workspace store
   *     ('tesela:workspace:recent' — written by the focusPane chokepoint),
   *     badge = count.
   *   - Tasks → the same agenda query source used by the Agenda surface
   *   - Add widget → stub (configurability deferred)
   * The widget set is fixed for parity; configurability is the iterate phase.
   */
  import { createQuery } from '@tanstack/svelte-query';
  import { onMount } from 'svelte';
  import GrWidget from '$lib/graphite/GrWidget.svelte';
  import GrRow from '$lib/graphite/GrRow.svelte';
  import GrIcon from '$lib/graphite/GrIcon.svelte';
  import { api } from '$lib/api-client';
  import type { AgendaRow as AgendaRowT } from '$lib/types/AgendaRow';
  import { commandRegistry } from '$lib/command-registry.svelte';
  import { getFavorites, isFavorite } from '$lib/stores/favorites.svelte';
  import { getPinned, getRecent } from '$lib/state/shared.svelte';
  import {
    agendaQueryKey,
    agendaRange,
    railNavigationTargetIndex,
    railTaskLabel,
    splitRailTasks,
  } from '$lib/graphite/rail-utils';

  let rail: HTMLElement;
  let previousFocus: HTMLElement | null = null;

  const favorites = $derived(getFavorites());
  const pinned = $derived(getPinned());
  const recents = $derived(getRecent());

  const agenda = agendaRange(new Date());
  const tasksQuery = createQuery(() => ({
    queryKey: agendaQueryKey(agenda.from, agenda.to),
    queryFn: () => api.getAgenda(agenda.from, agenda.to, false),
  }));
  const taskRows = $derived((tasksQuery.data ?? []) as AgendaRowT[]);
  const taskBuckets = $derived(splitRailTasks(taskRows));

  function railActions(): HTMLButtonElement[] {
    return rail
      ? Array.from(rail.querySelectorAll<HTMLButtonElement>('[data-rail-action]:not(:disabled)'))
      : [];
  }

  function runRailCommand(id: string, arg?: string, event?: Event) {
    event?.stopPropagation();
    const command = commandRegistry.get(id);
    if (!command) {
      console.warn(`graphite rail: command not registered: ${id}`);
      return;
    }
    void command.run(arg);
  }

  function focusRail() {
    const active = document.activeElement;
    if (active instanceof HTMLElement && !rail?.contains(active)) {
      previousFocus = active;
    }
    railActions()[0]?.focus();
  }

  function handleRailFocusIn(event: FocusEvent) {
    const target = event.target;
    if (!(target instanceof HTMLElement) || !rail.contains(target)) return;
    const prior = event.relatedTarget;
    if (prior instanceof HTMLElement && !rail.contains(prior)) {
      previousFocus = prior;
    }
  }

  function handleRailKeydown(event: KeyboardEvent) {
    const active = document.activeElement;
    if (!(active instanceof HTMLButtonElement) || !rail.contains(active)) return;
    const actions = railActions();
    const currentIndex = actions.indexOf(active);
    if (currentIndex < 0) return;

    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      const target = previousFocus;
      previousFocus = null;
      requestAnimationFrame(() => {
        if (document.activeElement !== active) return;
        if (target?.isConnected) target.focus();
        else active.blur();
      });
      return;
    }

    const nextIndex = railNavigationTargetIndex(event.key, currentIndex, actions.length);
    if (nextIndex === null) return;
    event.preventDefault();
    event.stopPropagation();
    actions[nextIndex]?.focus();
  }

  onMount(() => {
    document.addEventListener('tesela:focus-rail', focusRail);
    return () => document.removeEventListener('tesela:focus-rail', focusRail);
  });
</script>

<nav
  class="gr-rail"
  aria-label="Widget rail"
  bind:this={rail}
>
  <div
    class="gr-rail-scroll"
    role="toolbar"
    tabindex="-1"
    aria-label="Widget rail actions"
    aria-orientation="vertical"
    onfocusin={handleRailFocusIn}
    onkeydown={handleRailKeydown}
  >
    <GrWidget title="Quick capture" icon="bolt">
      <button
        type="button"
        class="gr-capture"
        data-rail-action=""
        data-command-id="rail-quick-capture"
        onclick={() => runRailCommand('rail-quick-capture')}
      >
        <span class="pl">Capture a thought…</span>
        <span class="pk">r c</span>
      </button>
    </GrWidget>

    <GrWidget title="Favorites" icon="star">
      {#if favorites.length === 0}
        <div class="gr-empty">No favorite pages yet</div>
      {:else}
        {#each favorites as id (id)}
          <div class="gr-row-wrap">
            <GrRow
              icon="file-text"
              label={id}
              data-rail-action=""
              data-command-id="jump"
              onclick={() => runRailCommand('jump', id)}
              aria-label={`Open favorite page ${id}`}
            />
            <button
              type="button"
              class="gr-row-favorite active"
              aria-pressed="true"
              aria-label={`Remove ${id} from favorites`}
              title="Remove from favorites"
              data-rail-action=""
              data-command-id="rail-toggle-favorite"
              onclick={(event) => runRailCommand('rail-toggle-favorite', id, event)}
            >
              <GrIcon name="star" size={13} />
            </button>
          </div>
        {/each}
      {/if}
    </GrWidget>

    <GrWidget title="Pinned" icon="pin">
      {#if pinned.length === 0}
        <div class="gr-empty">No pinned pages yet</div>
      {:else}
        {#each pinned as id (id)}
          <div class="gr-row-wrap">
            <GrRow
              icon="file-text"
              label={id}
              data-rail-action=""
              data-command-id="jump"
              onclick={() => runRailCommand('jump', id)}
              aria-label={`Open pinned page ${id}`}
            />
            <button
              type="button"
              class="gr-row-favorite"
              class:active={isFavorite(id)}
              aria-pressed={isFavorite(id)}
              aria-label={isFavorite(id) ? `Remove ${id} from favorites` : `Add ${id} to favorites`}
              title={isFavorite(id) ? "Remove from favorites" : "Add to favorites"}
              data-rail-action=""
              data-command-id="rail-toggle-favorite"
              onclick={(event) => runRailCommand('rail-toggle-favorite', id, event)}
            >
              <GrIcon name="star" size={13} />
            </button>
          </div>
        {/each}
      {/if}
    </GrWidget>

    <GrWidget title="Today" icon="sun" badge={String(recents.length)}>
      {#if recents.length === 0}
        <div class="gr-empty">Nothing recent</div>
      {:else}
        {#each recents as id (id)}
          <div class="gr-row-wrap">
            <GrRow
              icon="circle-dot"
              label={id}
              data-rail-action=""
              data-command-id="jump"
              onclick={() => runRailCommand('jump', id)}
              aria-label={`Open recent page ${id}`}
            />
            <button
              type="button"
              class="gr-row-favorite"
              class:active={isFavorite(id)}
              aria-pressed={isFavorite(id)}
              aria-label={isFavorite(id) ? `Remove ${id} from favorites` : `Add ${id} to favorites`}
              title={isFavorite(id) ? "Remove from favorites" : "Add to favorites"}
              data-rail-action=""
              data-command-id="rail-toggle-favorite"
              onclick={(event) => runRailCommand('rail-toggle-favorite', id, event)}
            >
              <GrIcon name="star" size={13} />
            </button>
          </div>
        {/each}
      {/if}
    </GrWidget>

    <GrWidget title="Tasks" icon="square-check" badge={String(taskBuckets.total)}>
      {#if tasksQuery.isLoading}
        <div class="gr-empty">Loading tasks…</div>
      {:else if taskBuckets.total === 0}
        <div class="gr-empty">No open tasks</div>
      {:else}
        {#if taskBuckets.doing.length > 0}
          <div class="gr-sub">Doing · {taskBuckets.doing.length}</div>
          {#each taskBuckets.doing as row (row.block_id + ':' + row.occurrence_date)}
            <GrRow
              icon="circle-dot"
              label={railTaskLabel(row)}
              meta={row.occurrence_date}
              data-rail-action=""
              data-command-id="jump"
              onclick={() => runRailCommand('jump', row.source_note_id)}
              aria-label={`Open task ${railTaskLabel(row)}`}
            />
          {/each}
        {/if}
        {#if taskBuckets.next.length > 0}
          <div class="gr-sub">Next · {taskBuckets.next.length}</div>
          {#each taskBuckets.next as row (row.block_id + ':' + row.occurrence_date)}
            <GrRow
              icon="circle-dot"
              label={railTaskLabel(row)}
              meta={row.occurrence_date}
              data-rail-action=""
              data-command-id="jump"
              onclick={() => runRailCommand('jump', row.source_note_id)}
              aria-label={`Open task ${railTaskLabel(row)}`}
            />
          {/each}
        {/if}
      {/if}
    </GrWidget>

    <button
      type="button"
      class="gr-addw"
      data-rail-action=""
      data-command-id="rail-add-widget"
      onclick={() => runRailCommand('rail-add-widget')}
    >
      <GrIcon name="plus" size={14} /> Add widget
    </button>
  </div>
</nav>

<style>
  .gr-rail {
    width: 256px;
    flex-shrink: 0;
    background: var(--surface);
    border-right: 1px solid var(--line);
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .gr-rail-scroll {
    flex: 1;
    overflow: auto;
    padding: 12px 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .gr-capture {
    width: calc(100% - 8px);
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px 4px;
    padding: 9px 11px;
    border-radius: 8px;
    background: var(--bg);
    border: 1px solid var(--line);
    color: var(--subtle);
    font-size: 12.5px;
    font-family: inherit;
    text-align: left;
    cursor: pointer;
  }
  .gr-capture .pl {
    flex: 1;
  }
  .gr-capture .pk {
    font-family: var(--mono);
    font-size: 10px;
    color: var(--faint);
  }
  .gr-row-wrap {
    position: relative;
  }
  .gr-row-wrap :global(.gr-row) {
    width: 100%;
    padding-right: 32px;
    text-align: left;
  }
  .gr-row-favorite {
    position: absolute;
    top: 50%;
    right: 5px;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    transform: translateY(-50%);
    border: 0;
    border-radius: 5px;
    background: transparent;
    color: var(--faint);
    cursor: pointer;
  }
  .gr-row-favorite:hover,
  .gr-row-favorite.active {
    color: var(--coral);
    background: var(--coral-dim);
  }
  .gr-sub {
    font-family: var(--mono);
    font-size: 9.5px;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
    padding: 7px 8px 3px;
  }
  .gr-empty {
    padding: 6px 8px;
    font-size: 12px;
    color: var(--faint);
  }
  .gr-addw {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    margin-top: auto;
    white-space: nowrap;
    padding: 9px;
    border-radius: 8px;
    border: 1px dashed var(--line-2);
    background: transparent;
    color: var(--subtle);
    font-size: 12px;
    font-family: inherit;
    cursor: pointer;
  }
  .gr-addw:hover {
    color: var(--fg2);
    border-color: var(--line-3);
  }
  .gr-rail :global([data-rail-action]:focus-visible) {
    outline: 2px solid var(--coral);
    outline-offset: 1px;
  }
</style>
