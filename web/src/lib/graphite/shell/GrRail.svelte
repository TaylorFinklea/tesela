<!-- web/src/lib/graphite/shell/GrRail.svelte -->
<script lang="ts">
  /*
   * Graphite widget rail — the left-hand widget host. New presentation
   * over existing stores:
   *   - Quick capture → openColonMode() (same verb the `:` keypress and
   *     v4's capture affordance use)
   *   - Pinned → getFavorites() (page-id strings persisted in localStorage)
   *   - Today → getRecents() (page-id strings), badge = count
   *   - Tasks → static placeholder rows this phase (real data = views phase)
   *   - Add widget → stub (configurability deferred)
   * The widget set is fixed for parity; configurability is the iterate phase.
   */
  import GrWidget from '$lib/graphite/GrWidget.svelte';
  import GrRow from '$lib/graphite/GrRow.svelte';
  import GrIcon from '$lib/graphite/GrIcon.svelte';
  import { openColonMode } from '$lib/stores/colon-mode.svelte';
  import { getFavorites } from '$lib/stores/favorites.svelte';
  import { getRecents } from '$lib/stores/recents.svelte';

  const favorites = $derived(getFavorites());
  const recents = $derived(getRecents());

  function addWidget() {
    // Configurable widgets are deferred to a later phase.
    console.log('graphite: add widget (deferred)');
  }
</script>

<div class="gr-rail">
  <div class="gr-rail-scroll">
    <GrWidget title="Quick capture" icon="bolt">
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div class="gr-capture" onclick={() => openColonMode()}>
        <span class="pl">Capture a thought…</span>
        <span class="pk">C</span>
      </div>
    </GrWidget>

    <GrWidget title="Pinned" icon="pin">
      {#if favorites.length === 0}
        <div class="gr-empty">No pinned pages yet</div>
      {:else}
        {#each favorites as id (id)}
          <GrRow icon="file-text" label={id} />
        {/each}
      {/if}
    </GrWidget>

    <GrWidget title="Today" icon="sun" badge={String(recents.length)}>
      {#if recents.length === 0}
        <div class="gr-empty">Nothing recent</div>
      {:else}
        {#each recents as id (id)}
          <GrRow icon="circle-dot" label={id} />
        {/each}
      {/if}
    </GrWidget>

    <GrWidget title="Tasks" icon="square-check">
      <div class="gr-sub">Doing</div>
      <GrRow icon="circle-dot" label="Placeholder task (views phase)" />
      <div class="gr-sub">Next</div>
      <GrRow icon="circle-dot" label="Placeholder task (views phase)" />
    </GrWidget>

    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="gr-addw" onclick={addWidget}>
      <GrIcon name="plus" size={14} /> Add widget
    </div>
  </div>
</div>

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
    color: var(--subtle);
    font-size: 12px;
    cursor: pointer;
  }
  .gr-addw:hover {
    color: var(--fg2);
    border-color: var(--line-3);
  }
</style>
