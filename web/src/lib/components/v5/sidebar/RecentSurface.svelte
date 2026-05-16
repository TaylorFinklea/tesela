<script lang="ts">
  /* Recent surface — reads the shared LRU populated by the focusPane
   * chokepoint. */
  import { getRecent } from "$lib/state/shared.svelte";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  const recent = $derived(getRecent());
</script>

<div class="v5-side-surface">
  <header>Recent</header>
  {#if recent.length === 0}
    <p class="muted">no recent</p>
  {:else}
    <ul>
      {#each recent as id (id)}
        <li>
          <button
            type="button"
            onclick={() => openPageInFocused(asPageId(id))}
          >{id}</button>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .v5-side-surface {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 10px 12px;
    font-family: var(--v4-mono);
    font-size: 11px;
    color: var(--v4-ink2);
  }
  header {
    color: var(--v4-ink);
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.7px;
    margin-bottom: 8px;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  li button {
    background: transparent;
    border: 1px solid transparent;
    color: var(--v4-ink2);
    text-align: left;
    padding: 3px 6px;
    border-radius: 4px;
    cursor: pointer;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--v4-mono);
    font-size: 11px;
  }
  li button:hover {
    border-color: var(--v4-hair);
    color: var(--v4-ink);
  }
  .muted {
    color: var(--v4-ink5);
  }
</style>
