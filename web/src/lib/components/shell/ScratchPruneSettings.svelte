<script lang="ts">
  /*
   * Settings panel for the scratch-prune sweep. Lives under the "data"
   * settings group since it's about persistent storage cleanup.
   *
   * The actual sweep runs from `web/src/lib/state/scratch-prune.ts` —
   * this is just the toggle/threshold UI.
   */
  import {
    getScratchPruneAfterDays,
    setScratchPruneAfterDays,
  } from "$lib/buffer/state.svelte";
  import { runScratchPrune } from "$lib/state/scratch-prune";
  import { getAppQueryClient } from "$lib/app-query-client.svelte";

  const current = $derived(getScratchPruneAfterDays());
  const enabled = $derived(!!current && current > 0);

  // Local form state — only commits to the workspace on Save / preset
  // click so a fat-fingered draft doesn't kick off a sweep.
  let draft = $state<number>(current && current > 0 ? current : 30);
  let running = $state(false);
  let lastResult = $state<string | null>(null);

  function commit(days: number | undefined) {
    setScratchPruneAfterDays(days);
  }

  async function pruneNow() {
    running = true;
    lastResult = null;
    try {
      const days = enabled ? current : draft;
      const result = await runScratchPrune(days);
      if (!result) {
        lastResult = "no threshold set";
      } else {
        lastResult = `scanned ${result.scanned} · pruned ${result.pruned.length}`;
        const qc = getAppQueryClient();
        if (qc) qc.invalidateQueries({ queryKey: ["notes"] });
      }
    } catch (e) {
      lastResult = e instanceof Error ? e.message : String(e);
    } finally {
      running = false;
    }
  }
</script>

<section class="scratch-prune-settings">
  <header>
    <h3>Scratch prune</h3>
    <p class="hint">
      Scratch pages (created via <code>:scratch</code> or <code>Space n s</code>)
      that aren't promoted accumulate. The prune sweep deletes scratches
      with no edits past a threshold. Disabled by default.
    </p>
  </header>

  <div class="row toggle">
    <span>
      Current:
      <strong>
        {#if enabled}
          delete unedited scratches older than {current} days
        {:else}
          off
        {/if}
      </strong>
    </span>
  </div>

  <div class="row presets">
    <button type="button" onclick={() => commit(undefined)} class:active={!enabled}
      >Off</button
    >
    <button
      type="button"
      onclick={() => commit(7)}
      class:active={current === 7}>7 days</button
    >
    <button
      type="button"
      onclick={() => commit(30)}
      class:active={current === 30}>30 days</button
    >
    <button
      type="button"
      onclick={() => commit(90)}
      class:active={current === 90}>90 days</button
    >
  </div>

  <div class="row custom">
    <label>
      Custom days
      <input
        type="number"
        min="1"
        max="365"
        bind:value={draft}
        onchange={() => {
          if (draft > 0) commit(draft);
        }}
      />
    </label>
    <button type="button" onclick={pruneNow} disabled={running}>
      {running ? "Sweeping…" : "Prune now"}
    </button>
    {#if lastResult}
      <span class="result">{lastResult}</span>
    {/if}
  </div>
</section>

<style>
  .scratch-prune-settings {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px 0;
    border-top: 1px solid var(--v9-line, var(--line-soft));
  }
  header {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  h3 {
    margin: 0;
    font-size: 14px;
    color: var(--v9-ink, var(--fg-default));
  }
  .hint {
    color: var(--v9-ink-muted, var(--fg-faint));
    font-size: 12px;
    margin: 0;
    max-width: 64ch;
  }
  code {
    font-family: var(--theme-font-mono);
    color: var(--primary);
    background: transparent;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    font-size: 12px;
    color: var(--v9-ink-muted, var(--fg-subtle));
  }
  .row strong {
    color: var(--v9-ink, var(--fg-default));
    font-weight: 500;
  }
  .presets button,
  .custom button {
    background: transparent;
    border: 1px solid var(--v9-line, var(--line-soft));
    color: var(--v9-ink-muted, var(--fg-subtle));
    border-radius: 5px;
    padding: 4px 12px;
    cursor: pointer;
    font-family: var(--theme-font-mono);
    font-size: 11px;
  }
  .presets button:hover,
  .custom button:hover:not(:disabled) {
    border-color: var(--v9-line-soft, var(--line));
    color: var(--v9-ink, var(--fg-default));
  }
  .presets button.active {
    border-color: var(--primary);
    color: var(--primary);
  }
  label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  input[type="number"] {
    width: 80px;
    background: transparent;
    border: 1px solid var(--v9-line, var(--line-soft));
    border-radius: 5px;
    padding: 4px 8px;
    color: var(--v9-ink, var(--fg-default));
    font-family: var(--theme-font-mono);
    font-size: 12px;
  }
  input[type="number"]:focus {
    border-color: var(--primary);
    outline: none;
  }
  .result {
    color: var(--v9-ink-muted, var(--fg-faint));
    font-family: var(--theme-font-mono);
    font-size: 11px;
  }
</style>
