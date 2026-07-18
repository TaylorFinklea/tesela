<!-- Configurable Graphite widget rail (tesela-tko). -->
<script lang="ts">
  import { createQuery, useQueryClient } from "@tanstack/svelte-query";
  import { onMount } from "svelte";
  import { api, type ViewRecord } from "$lib/api-client";
  import { commandRegistry } from "$lib/command-registry.svelte";
  import GrIcon from "$lib/graphite/GrIcon.svelte";
  import GrRow from "$lib/graphite/GrRow.svelte";
  import RailQueryProjection from "$lib/graphite/shell/RailQueryProjection.svelte";
  import RailSyncHealth from "$lib/graphite/shell/RailSyncHealth.svelte";
  import RailWidgetFrame from "$lib/graphite/shell/RailWidgetFrame.svelte";
  import {
    addRailWidget,
    BUILTIN_RAIL_WIDGETS,
    loadRailWidgetLayout,
    moveRailWidget,
    placementKind,
    projectionFromQueryWidget,
    projectionFromSavedView,
    queryWidgetCandidate,
    RAIL_PROJECTION_QUERY_KEY,
    removeRailWidget,
    savedViewCandidate,
    saveRailWidgetLayout,
    sourceIdFromPlacement,
    toggleRailWidgetCollapsed,
    type RailQueryProjection as RailQueryProjectionT,
    type RailWidgetCandidate,
    type RailWidgetLayout,
    type RailWidgetPlacement,
  } from "$lib/graphite/rail-widget-layout";
  import {
    agendaQueryKey,
    agendaRange,
    railNavigationTargetIndex,
    railTaskLabel,
    splitRailTasks,
  } from "$lib/graphite/rail-utils";
  import { getFavorites, isFavorite } from "$lib/stores/favorites.svelte";
  import { getPinned, getRecent } from "$lib/state/shared.svelte";
  import { refreshRelayStatus } from "$lib/relay-status.svelte";
  import type { AgendaRow as AgendaRowT } from "$lib/types/AgendaRow";
  import type { Note } from "$lib/types/Note";
  import { parseWidgets } from "$lib/widget-registry.svelte";

  let rail: HTMLElement;
  let addButton: HTMLButtonElement;
  let previousFocus: HTMLElement | null = null;
  let pickerOpen = $state(false);
  let layout = $state<RailWidgetLayout>(loadRailWidgetLayout());
  const queryClient = useQueryClient();

  const favorites = $derived(getFavorites());
  const pinned = $derived(getPinned());
  const recents = $derived(getRecent());

  const notesQuery = createQuery(() => ({
    queryKey: ["notes", { limit: 5000 }] as const,
    queryFn: () => api.listNotes({ limit: 5000 }),
  }));
  const viewsQuery = createQuery(() => ({
    queryKey: ["views"] as const,
    queryFn: () => api.listViews(),
  }));
  const notes = $derived((notesQuery.data ?? []) as Note[]);
  const views = $derived((viewsQuery.data ?? []) as ViewRecord[]);
  const queryWidgets = $derived(parseWidgets(notes));
  const notesById = $derived(new Map(notes.map((note) => [note.id, note])));

  const queryProjections = $derived.by(() => {
    const out = new Map<string, RailQueryProjectionT>();
    for (const widget of queryWidgets) {
      const note = notesById.get(widget.id);
      if (note) out.set(`query:${widget.id}`, projectionFromQueryWidget(widget, note.checksum));
    }
    return out;
  });
  const viewProjections = $derived.by(() => {
    const out = new Map<string, RailQueryProjectionT>();
    for (const view of views) out.set(`view:${view.id}`, projectionFromSavedView(view));
    const inbox = views.find((view) => view.id === "builtin-inbox");
    if (inbox) out.set("builtin:inbox", projectionFromSavedView(inbox, "builtin:inbox"));
    return out;
  });
  const allCandidates = $derived.by((): RailWidgetCandidate[] => [
    ...BUILTIN_RAIL_WIDGETS,
    ...queryWidgets.map(queryWidgetCandidate),
    ...views.filter((view) => view.id !== "builtin-inbox").map(savedViewCandidate),
  ]);
  const candidateById = $derived(new Map(allCandidates.map((candidate) => [candidate.id, candidate])));
  const availableCandidates = $derived(
    allCandidates.filter((candidate) => !layout.placements.some((placement) => placement.id === candidate.id)),
  );

  const agenda = agendaRange(new Date());
  const tasksQuery = createQuery(() => ({
    queryKey: agendaQueryKey(agenda.from, agenda.to),
    queryFn: () => api.getAgenda(agenda.from, agenda.to, false),
  }));
  const taskRows = $derived((tasksQuery.data ?? []) as AgendaRowT[]);
  const taskBuckets = $derived(splitRailTasks(taskRows));

  function railActions(): HTMLButtonElement[] {
    return rail
      ? Array.from(rail.querySelectorAll<HTMLButtonElement>("[data-rail-action]:not(:disabled)"))
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

  function persist(next: RailWidgetLayout) {
    layout = next;
    saveRailWidgetLayout(next);
  }

  function handleWidgetAction(event: Event) {
    const detail = (event as CustomEvent<{ action?: string; id?: string }>).detail;
    const action = detail?.action;
    const id = detail?.id;
    if (action === "open-picker") {
      pickerOpen = true;
      requestAnimationFrame(() => {
        const target = rail.querySelector<HTMLButtonElement>(".picker-item")
          ?? rail.querySelector<HTMLButtonElement>(".picker-close");
        target?.focus();
      });
      return;
    }
    if (!id) return;
    if (action === "add") {
      const candidate = candidateById.get(id);
      if (!candidate) return;
      persist(addRailWidget(layout, candidate));
      pickerOpen = false;
      requestAnimationFrame(() => addButton?.focus());
    } else if (action === "remove") persist(removeRailWidget(layout, id));
    else if (action === "move-up") persist(moveRailWidget(layout, id, -1));
    else if (action === "move-down") persist(moveRailWidget(layout, id, 1));
    else if (action === "toggle") persist(toggleRailWidgetCollapsed(layout, id));
    else if (action === "refresh") {
      if (id === "builtin:agenda") void queryClient.invalidateQueries({ queryKey: ["agenda"] });
      else if (id === "builtin:sync-health") void refreshRelayStatus();
      else void queryClient.invalidateQueries({ queryKey: [RAIL_PROJECTION_QUERY_KEY, id] });
    }
  }

  function closePicker() {
    pickerOpen = false;
    requestAnimationFrame(() => addButton?.focus());
  }

  function handlePickerKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.preventDefault();
    event.stopPropagation();
    closePicker();
  }

  function focusRail() {
    const active = document.activeElement;
    if (active instanceof HTMLElement && !rail?.contains(active)) previousFocus = active;
    const quickCapture = rail?.querySelector<HTMLButtonElement>(
      '[data-command-id="rail-quick-capture"]',
    );
    (quickCapture ?? railActions()[0])?.focus();
  }

  function handleRailFocusIn(event: FocusEvent) {
    const target = event.target;
    if (!(target instanceof HTMLElement) || !rail.contains(target)) return;
    const prior = event.relatedTarget;
    if (prior instanceof HTMLElement && !rail.contains(prior)) previousFocus = prior;
  }

  function handleRailKeydown(event: KeyboardEvent) {
    const active = document.activeElement;
    if (!(active instanceof HTMLButtonElement) || !rail.contains(active)) return;
    const actions = railActions();
    const currentIndex = actions.indexOf(active);
    if (currentIndex < 0) return;

    if (event.key === "Escape") {
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

  function titleFor(placement: RailWidgetPlacement): string {
    return candidateById.get(placement.id)?.title
      ?? queryProjections.get(placement.id)?.title
      ?? viewProjections.get(placement.id)?.title
      ?? placement.fallbackTitle;
  }

  function iconFor(placement: RailWidgetPlacement): string {
    return candidateById.get(placement.id)?.icon
      ?? queryProjections.get(placement.id)?.icon
      ?? viewProjections.get(placement.id)?.icon
      ?? "search";
  }

  onMount(() => {
    document.addEventListener("tesela:focus-rail", focusRail);
    document.addEventListener("tesela:rail-widget-action", handleWidgetAction);
    return () => {
      document.removeEventListener("tesela:focus-rail", focusRail);
      document.removeEventListener("tesela:rail-widget-action", handleWidgetAction);
    };
  });
</script>

<nav class="gr-rail" aria-label="Widget rail" bind:this={rail}>
  <div
    class="gr-rail-scroll"
    role="toolbar"
    tabindex="-1"
    aria-label="Widget rail actions"
    aria-orientation="vertical"
    onfocusin={handleRailFocusIn}
    onkeydown={handleRailKeydown}
  >
    {#each layout.placements as placement, index (placement.id)}
      {@const sourceId = sourceIdFromPlacement(placement.id)}
      {@const projection = queryProjections.get(placement.id) ?? viewProjections.get(placement.id)}
      <RailWidgetFrame
        placementId={placement.id}
        title={titleFor(placement)}
        icon={iconFor(placement)}
        collapsed={placement.collapsed}
        {index}
        total={layout.placements.length}
        badge={placement.id === "builtin:agenda" ? String(taskBuckets.total) : undefined}
      >
        {#if placement.id === "builtin:quick-capture"}
          <button
            type="button"
            class="gr-capture"
            data-rail-action=""
            data-command-id="rail-quick-capture"
            onclick={() => runRailCommand("rail-quick-capture")}
          ><span class="pl">Capture a thought…</span><span class="pk">r c</span></button>
        {:else if placement.id === "builtin:favorites"}
          {#if favorites.length === 0}<div class="gr-empty">No favorite pages yet</div>{/if}
          {#each favorites as id (id)}
            <div class="gr-row-wrap">
              <GrRow icon="file-text" label={id} data-rail-action="" data-command-id="jump"
                onclick={() => runRailCommand("jump", id)} aria-label={`Open favorite page ${id}`} />
              <button type="button" class="gr-row-favorite active" aria-pressed="true"
                aria-label={`Remove ${id} from favorites`} title="Remove from favorites"
                data-rail-action="" data-command-id="rail-toggle-favorite"
                onclick={(event) => runRailCommand("rail-toggle-favorite", id, event)}><GrIcon name="star" size={13} /></button>
            </div>
          {/each}
        {:else if placement.id === "builtin:pinned"}
          {#if pinned.length === 0}<div class="gr-empty">No pinned pages yet</div>{/if}
          {#each pinned as id (id)}
            <div class="gr-row-wrap">
              <GrRow icon="file-text" label={id} data-rail-action="" data-command-id="jump"
                onclick={() => runRailCommand("jump", id)} aria-label={`Open pinned page ${id}`} />
              <button type="button" class="gr-row-favorite" class:active={isFavorite(id)}
                aria-pressed={isFavorite(id)} aria-label={isFavorite(id) ? `Remove ${id} from favorites` : `Add ${id} to favorites`}
                data-rail-action="" data-command-id="rail-toggle-favorite"
                onclick={(event) => runRailCommand("rail-toggle-favorite", id, event)}><GrIcon name="star" size={13} /></button>
            </div>
          {/each}
        {:else if placement.id === "builtin:recent"}
          {#if recents.length === 0}<div class="gr-empty">Nothing recent</div>{/if}
          {#each recents as id (id)}
            <div class="gr-row-wrap">
              <GrRow icon="circle-dot" label={id} data-rail-action="" data-command-id="jump"
                onclick={() => runRailCommand("jump", id)} aria-label={`Open recent page ${id}`} />
              <button type="button" class="gr-row-favorite" class:active={isFavorite(id)}
                aria-pressed={isFavorite(id)} aria-label={isFavorite(id) ? `Remove ${id} from favorites` : `Add ${id} to favorites`}
                data-rail-action="" data-command-id="rail-toggle-favorite"
                onclick={(event) => runRailCommand("rail-toggle-favorite", id, event)}><GrIcon name="star" size={13} /></button>
            </div>
          {/each}
        {:else if placement.id === "builtin:agenda"}
          {#if tasksQuery.isLoading}
            <div class="gr-empty">Loading agenda…</div>
          {:else if tasksQuery.isError && !tasksQuery.data}
            <div class="gr-empty error">Agenda unavailable</div>
            <button class="retry" type="button" data-rail-action="" data-command-id="rail-refresh-widget"
              onclick={() => runRailCommand("rail-refresh-widget", placement.id)}>Retry</button>
          {:else if taskBuckets.total === 0}
            <div class="gr-empty">No open tasks</div>
          {:else}
            {#if tasksQuery.isFetching}<div class="freshness">Refreshing…</div>{/if}
            {#if taskBuckets.doing.length > 0}<div class="gr-sub">Doing · {taskBuckets.doing.length}</div>{/if}
            {#each taskBuckets.doing as row (row.block_id + ":" + row.occurrence_date)}
              <GrRow icon="circle-dot" label={railTaskLabel(row)} meta={row.occurrence_date}
                data-rail-action="" data-command-id="jump" onclick={() => runRailCommand("jump", row.source_note_id)} />
            {/each}
            {#if taskBuckets.next.length > 0}<div class="gr-sub">Next · {taskBuckets.next.length}</div>{/if}
            {#each taskBuckets.next as row (row.block_id + ":" + row.occurrence_date)}
              <GrRow icon="circle-dot" label={railTaskLabel(row)} meta={row.occurrence_date}
                data-rail-action="" data-command-id="jump" onclick={() => runRailCommand("jump", row.source_note_id)} />
            {/each}
          {/if}
        {:else if placement.id === "builtin:sync-health"}
          <RailSyncHealth />
        {:else if projection}
          <RailQueryProjection {projection} />
        {:else if (placementKind(placement.id) === "query" && notesQuery.isLoading)
          || (placementKind(placement.id) === "view" && viewsQuery.isLoading)
          || (placement.id === "builtin:inbox" && viewsQuery.isLoading)}
          <div class="gr-empty">Loading widget definition…</div>
        {:else}
          <div class="gr-empty error" role="alert">Source “{sourceId}” is unavailable.</div>
          <div class="freshness">Remove it or restore the Query note/view.</div>
        {/if}
      </RailWidgetFrame>
    {/each}

    <button
      bind:this={addButton}
      type="button"
      class="gr-addw"
      data-rail-action=""
      data-command-id="rail-add-widget"
      aria-expanded={pickerOpen}
      onclick={() => runRailCommand("rail-add-widget")}
    ><GrIcon name="plus" size={14} /> Add widget</button>

    {#if pickerOpen}
      <div class="picker" role="dialog" aria-label="Add a rail widget" tabindex="-1" onkeydown={handlePickerKeydown}>
        <div class="picker-head"><strong>Add widget</strong><button type="button" class="picker-close" data-rail-action="" aria-label="Close widget picker" onclick={closePicker}><GrIcon name="x" size={13} /></button></div>
        {#if notesQuery.isLoading || viewsQuery.isLoading}<div class="gr-empty">Loading available widgets…</div>{/if}
        {#if notesQuery.isError || viewsQuery.isError}<div class="gr-empty error">Some widget sources could not be loaded.</div>{/if}
        {#if availableCandidates.length === 0 && !notesQuery.isLoading && !viewsQuery.isLoading}
          <div class="gr-empty">Every available widget is already in the rail.</div>
        {/if}
        {#each availableCandidates as candidate (candidate.id)}
          <button type="button" class="picker-item" data-rail-action="" data-command-id="rail-add-widget-by-id"
            onclick={() => runRailCommand("rail-add-widget-by-id", candidate.id)}>
            <GrIcon name={candidate.icon} size={14} /><span><strong>{candidate.title}</strong><small>{candidate.subtitle}</small></span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</nav>

<style>
  .gr-rail { width:256px; flex-shrink:0; background:var(--surface); border-right:1px solid var(--line); display:flex; flex-direction:column; min-height:0; }
  .gr-rail-scroll { flex:1; overflow:auto; padding:12px 10px; display:flex; flex-direction:column; gap:8px; position:relative; }
  .gr-capture { width:calc(100% - 8px); display:flex; align-items:center; gap:8px; margin:0 4px 4px; padding:9px 11px; border-radius:8px; background:var(--bg); border:1px solid var(--line); color:var(--subtle); font-size:12.5px; font-family:inherit; text-align:left; cursor:pointer; }
  .gr-capture .pl { flex:1; } .gr-capture .pk { font-family:var(--mono); font-size:10px; color:var(--faint); }
  .gr-row-wrap { position:relative; } .gr-row-wrap :global(.gr-row) { width:100%; padding-right:32px; text-align:left; }
  .gr-row-favorite { position:absolute; top:50%; right:5px; display:grid; place-items:center; width:22px; height:22px; transform:translateY(-50%); border:0; border-radius:5px; background:transparent; color:var(--faint); cursor:pointer; }
  .gr-row-favorite:hover, .gr-row-favorite.active { color:var(--coral); background:var(--coral-dim); }
  .gr-sub { font-family:var(--mono); font-size:9.5px; letter-spacing:.1em; text-transform:uppercase; color:var(--faint); padding:7px 8px 3px; }
  .gr-empty { padding:6px 8px; font-size:12px; color:var(--faint); overflow-wrap:anywhere; } .gr-empty.error { color:var(--coral); }
  .freshness { padding:1px 8px 4px; font:9.5px var(--mono); color:var(--faint); }
  .retry { margin:0 8px 5px; border:1px solid var(--line); border-radius:5px; background:var(--bg); color:var(--subtle); font:10px var(--mono); cursor:pointer; }
  .gr-addw { display:flex; align-items:center; justify-content:center; gap:7px; margin-top:auto; white-space:nowrap; padding:9px; border-radius:8px; border:1px dashed var(--line-2); background:transparent; color:var(--subtle); font-size:12px; font-family:inherit; cursor:pointer; }
  .gr-addw:hover { color:var(--fg2); border-color:var(--line-3); }
  .picker { position:sticky; bottom:0; z-index:4; max-height:340px; overflow:auto; padding:7px; border:1px solid var(--line-2); border-radius:10px; background:var(--raised); box-shadow:0 12px 30px rgba(0,0,0,.35); }
  .picker-head { display:flex; align-items:center; justify-content:space-between; padding:3px 4px 7px; color:var(--fg2); font-size:12px; }
  .picker-head button { display:grid; place-items:center; width:24px; height:24px; border:0; border-radius:5px; background:transparent; color:var(--faint); cursor:pointer; }
  .picker-item { width:100%; display:flex; align-items:center; gap:8px; padding:7px; border:0; border-radius:6px; background:transparent; color:var(--subtle); text-align:left; cursor:pointer; }
  .picker-item:hover, .picker-item:focus-visible { background:var(--bg); color:var(--fg2); }
  .picker-item span { display:flex; min-width:0; flex-direction:column; } .picker-item strong { font-size:11.5px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; } .picker-item small { font:9.5px var(--mono); color:var(--faint); }
  .gr-rail :global([data-rail-action]:focus-visible) { outline:2px solid var(--coral); outline-offset:1px; }
</style>
