<!-- web/src/lib/graphite/shell/GrStatus.svelte -->
<script lang="ts">
  /*
   * Graphite vim status line — display-only, mirrors the mockup `.gr-status`.
   * The mode pill reads the existing pane-state vim mode store (getVimMode),
   * same source v5's StatusLine uses. The breadcrumb path + contextual keys
   * are props the shell/active view supplies (static defaults this phase);
   * real per-view keys arrive with the views phase. The clock is a local
   * $state updated on a 1s interval, cleared on teardown.
   */
  import GrIcon from '$lib/graphite/GrIcon.svelte';
  import { getVimMode } from '$lib/stores/pane-state.svelte';

  let {
    path = '',
    keys = [],
  }: {
    path?: string;
    keys?: { k: string; label: string }[];
  } = $props();

  const mode = $derived((getVimMode() || 'NORMAL').toUpperCase());

  function fmtClock(): string {
    const d = new Date();
    const h = String(d.getHours()).padStart(2, '0');
    const m = String(d.getMinutes()).padStart(2, '0');
    return `${h}:${m}`;
  }

  let clock = $state(fmtClock());

  $effect(() => {
    const id = setInterval(() => {
      clock = fmtClock();
    }, 1000);
    return () => clearInterval(id);
  });
</script>

<div class="gr-status">
  <span class="mode">{mode}</span>
  {#if path}
    <span class="sep">·</span>
    <span class="path">{path}</span>
  {/if}
  <span class="keys">
    {#each keys as k}
      <span><kbd>{k.k}</kbd> {k.label}</span>
    {/each}
    <span class="clk"><GrIcon name="clock" size={12} />{clock}</span>
  </span>
</div>

<style>
  .gr-status {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 14px;
    height: 30px;
    background: var(--surface);
    white-space: nowrap;
    overflow: hidden;
    border-top: 1px solid var(--line);
    font-family: var(--mono);
    font-size: 11px;
    color: var(--subtle);
  }
  .gr-status .mode {
    color: var(--coral);
    font-weight: 700;
    letter-spacing: 0.1em;
    font-size: 10px;
  }
  .gr-status .sep {
    color: var(--faint);
  }
  .gr-status .keys {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .gr-status .keys kbd {
    color: var(--fg2);
    font-family: var(--mono);
  }
  .gr-status .clk {
    color: var(--faint);
    display: flex;
    align-items: center;
    gap: 5px;
  }
</style>
