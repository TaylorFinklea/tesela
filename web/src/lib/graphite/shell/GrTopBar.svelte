<!-- web/src/lib/graphite/shell/GrTopBar.svelte -->
<script lang="ts">
  /*
   * Graphite top bar — new presentation bound to the existing v4/v5
   * behavior. Mirrors web/src/routes/v4/+layout.svelte's top-bar wiring:
   *   - workspace tabs come from the buffer state store (getWorkspace /
   *     switchTab), the active tab gets the coral `.kdot`
   *   - the command bar opens the Command Station (openStation), same as
   *     v4's `.v4-command-bar`
   *   - the connection dot reflects ws-client's getConnected()
   *   - graph / settings icons open the existing fullscreen overlays
   * Only the markup + CSS (the mockup's `.gr-top`) is new.
   */
  import GrIcon from '$lib/graphite/GrIcon.svelte';
  import { getWorkspace, switchTab, newTab } from '$lib/buffer/state.svelte';
  import { openStation } from '$lib/stores/station.svelte';
  import {
    openFullscreenGraph,
    openSettingsOverlay,
  } from '$lib/stores/fullscreen-overlay.svelte';
  import { getConnected } from '$lib/ws-client.svelte';

  const workspace = $derived(getWorkspace());
</script>

<div class="gr-top">
  <div class="gr-top-left">
    <div class="gr-brand">
      <span class="mark" aria-hidden="true"></span>
      <span class="nm">tesela</span>
    </div>
    <div class="gr-tabs">
      {#each workspace.tabs as t (t.id)}
        <button
          type="button"
          class="gr-tab"
          class:active={t.id === workspace.activeTabId}
          onclick={() => switchTab(t.id)}
          title="switch tab"
        >
          {#if t.id === workspace.activeTabId}<span class="kdot"></span>{/if}
          <span class="nm">{t.name}</span>
        </button>
      {/each}
      <button
        type="button"
        class="gr-ic gr-tab-add"
        onclick={() => newTab()}
        title="new tab · ⌘T"
        aria-label="New tab"
      >
        <GrIcon name="plus" size={15} />
      </button>
    </div>
  </div>

  <button
    type="button"
    class="gr-cmd"
    onclick={() => openStation()}
    title="Search or run a command · ⌘K"
  >
    <GrIcon name="search" size={15} />
    <span class="ph">Search or run a command…</span>
    <kbd>⌘K</kbd>
  </button>

  <div class="gr-icons">
    <button type="button" class="gr-ic" title="Voice capture" aria-label="Voice capture">
      <GrIcon name="microphone" size={16} />
    </button>
    <button
      type="button"
      class="gr-conn"
      class:offline={!getConnected()}
      onclick={() => openSettingsOverlay('sync')}
      title={getConnected() ? 'Connected — live sync running. Click for Sync settings.' : 'Disconnected from server. Click for Sync settings.'}
      aria-label={getConnected() ? 'Connected' : 'Disconnected'}
    >
      <i></i>
    </button>
    <button
      type="button"
      class="gr-ic"
      onclick={() => openFullscreenGraph()}
      title="Fullscreen graph · ⌘G"
      aria-label="Graph"
    >
      <GrIcon name="graph" size={16} />
    </button>
    <button
      type="button"
      class="gr-ic"
      onclick={() => openSettingsOverlay('general')}
      title="Settings"
      aria-label="Settings"
    >
      <GrIcon name="settings" size={16} />
    </button>
  </div>
</div>

<style>
  .gr-top {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 18px;
    padding: 0 16px;
    border-bottom: 1px solid var(--line);
    background: var(--surface);
    height: 48px;
    z-index: 5;
  }
  .gr-top-left {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  .gr-brand {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .gr-brand .mark {
    width: 18px;
    height: 18px;
    border-radius: 5px;
    flex-shrink: 0;
    background: linear-gradient(135deg, #8693B2 0%, var(--coral) 100%);
  }
  .gr-brand .nm {
    font-size: 13.5px;
    font-weight: 600;
    color: var(--fg);
    letter-spacing: -0.01em;
  }
  .gr-tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: 6px;
    min-width: 0;
    overflow: hidden;
  }
  .gr-tab {
    height: 30px;
    padding: 0 11px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    color: var(--subtle);
    cursor: pointer;
    white-space: nowrap;
    background: transparent;
    border: 1px solid transparent;
    font-family: var(--sans);
    transition: all 0.14s;
  }
  .gr-tab:hover {
    color: var(--fg2);
    background: var(--raised);
  }
  .gr-tab.active {
    color: var(--fg);
    background: var(--raised);
    border: 1px solid var(--line-2);
  }
  .gr-tab .kdot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--coral);
  }
  .gr-tab.active .nm {
    font-weight: 550;
  }
  .gr-cmd {
    justify-self: center;
    width: min(440px, 100%);
    height: 32px;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 0 11px;
    border-radius: 9px;
    background: var(--bg);
    border: 1px solid var(--line-2);
    color: var(--subtle);
    cursor: pointer;
    font-size: 12.5px;
    font-family: var(--sans);
    transition: border-color 0.14s;
  }
  .gr-cmd:hover {
    border-color: var(--line-3);
  }
  .gr-cmd .ph {
    flex: 1;
    text-align: left;
  }
  .gr-cmd kbd {
    font-family: var(--mono);
    font-size: 10.5px;
    color: var(--subtle);
    background: var(--raised);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 2px 6px;
    line-height: 1;
  }
  .gr-icons {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .gr-ic {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 8px;
    color: var(--subtle);
    cursor: pointer;
    background: transparent;
    border: none;
    transition: all 0.14s;
  }
  .gr-ic:hover {
    color: var(--fg);
    background: var(--raised);
  }
  .gr-tab-add {
    width: 26px;
    height: 26px;
    flex-shrink: 0;
  }
  .gr-conn {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    background: transparent;
    border: none;
    cursor: pointer;
  }
  .gr-conn i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--query);
    box-shadow: 0 0 0 3px rgba(133, 188, 99, 0.16);
  }
  .gr-conn.offline i {
    background: var(--faint);
    box-shadow: none;
  }
</style>
