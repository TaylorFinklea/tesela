<script lang="ts">
  /*
   * Prism v4 — Journey breadcrumb bar.
   *
   * Sits just under the top bar. Renders the last 10 tile visits as
   * scannable chips with a back arrow on the left. ⌘[ also walks back
   * (wired in the v4 layout's keymap). Clicking a chip jumps directly
   * to that entry; the suppression flag in the journey store keeps the
   * resulting jumpToTile from re-recording itself.
   */
  import {
    canGoBackInJourney,
    canGoForwardInJourney,
    getJourneyCursor,
    getJourneyEntries,
    goBackInJourney,
    goForwardInJourney,
    jumpToJourneyEntry,
  } from "$lib/stores/journey.svelte";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";
  // Thin shim — Journey only ever opens pages.
  const openInEditor = (id: string, _opts?: { via?: string }) =>
    openPageInFocused(asPageId(id));

  const entries = $derived(getJourneyEntries());
  const cursor = $derived(getJourneyCursor());
  const canBack = $derived(canGoBackInJourney());
  const canFwd = $derived(canGoForwardInJourney());

  // Journey is a navigation surface: clicking a chip / walking back or
  // forward routes the target tile into the main editor pane, not
  // whatever pane currently has focus. Mirrors neovim's history-jumps.
  function walkBack() {
    const t = goBackInJourney();
    if (t) openInEditor(t, { via: "back" });
  }
  function walkForward() {
    const t = goForwardInJourney();
    if (t) openInEditor(t, { via: "forward" });
  }
  function chipClick(idx: number) {
    const t = jumpToJourneyEntry(idx);
    if (t) openInEditor(t, { via: "chip" });
  }
</script>

<div class="v4-journey">
  <span class="v4-journey-label">journey</span>
  <button
    type="button"
    class="v4-journey-nav"
    title="back · ⌘["
    disabled={!canBack}
    onclick={walkBack}
  >←</button>
  <button
    type="button"
    class="v4-journey-nav"
    title="forward · ⌘]"
    disabled={!canFwd}
    onclick={walkForward}
  >→</button>
  {#if entries.length === 0}
    <span class="v4-journey-hint">empty — every jump lands here</span>
  {:else}
    <div class="v4-journey-chips">
      {#each entries as e, i (e.ts)}
        <button
          type="button"
          class="v4-journey-chip"
          class:active={i === cursor}
          title={`${e.tileId} · via ${e.via}`}
          onclick={() => chipClick(i)}
        >{e.tileId}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .v4-journey {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 14px;
    border-bottom: 1px solid var(--v4-hair);
    overflow: hidden;
    min-height: 30px;
  }
  .v4-journey-label {
    font-family: var(--v4-mono);
    font-size: 9.5px;
    letter-spacing: 1.4px;
    text-transform: uppercase;
    color: var(--v4-ink5);
    flex-shrink: 0;
  }
  .v4-journey-nav {
    background: transparent;
    border: 0;
    color: var(--v4-ink4);
    font-family: var(--v4-mono);
    font-size: 14px;
    line-height: 1;
    padding: 2px 6px;
    border-radius: 4px;
    cursor: pointer;
    flex-shrink: 0;
  }
  .v4-journey-nav:hover:not(:disabled) {
    color: var(--v4-ink2);
    background: var(--v4-surface-lo);
  }
  .v4-journey-nav:disabled {
    color: var(--v4-ink6);
    cursor: default;
  }
  .v4-journey-hint {
    font-family: var(--v4-mono);
    font-size: 10px;
    color: var(--v4-ink6);
  }
  .v4-journey-chips {
    display: flex;
    align-items: center;
    gap: 4px;
    overflow: hidden;
    min-width: 0;
  }
  .v4-journey-chip {
    background: transparent;
    border: 1px solid var(--v4-hair);
    border-radius: 4px;
    padding: 1px 8px;
    color: var(--v4-ink4);
    font-family: var(--v4-mono);
    font-size: 10.5px;
    cursor: pointer;
    flex-shrink: 0;
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .v4-journey-chip:hover {
    color: var(--v4-ink2);
    border-color: var(--v4-hair2);
  }
  .v4-journey-chip.active {
    color: var(--v4-ink);
    border-color: var(--v4-accent-dim);
    background: color-mix(in srgb, var(--v4-accent) 10%, transparent);
  }
</style>
