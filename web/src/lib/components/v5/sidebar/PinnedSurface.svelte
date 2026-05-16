<script lang="ts">
  /* Pinned surface — user-curated. Read from shared workspace state. */
  import { getPinned, unpin } from "$lib/state/shared.svelte";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  const pinned = $derived(getPinned());
</script>

<div class="v5-side-surface">
  <header>Pinned</header>
  {#if pinned.length === 0}
    <p class="muted">no pinned</p>
    <p class="hint">★ in the notes tree to pin</p>
  {:else}
    <ul>
      {#each pinned as id (id)}
        <li>
          <button
            type="button"
            class="row"
            onclick={() => openPageInFocused(asPageId(id))}
          >{id}</button>
          <button
            type="button"
            class="unpin"
            title="unpin"
            onclick={() => unpin(id)}
          >×</button>
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
  li {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 4px;
  }
  .row {
    background: transparent;
    border: 1px solid transparent;
    color: var(--v4-ink2);
    text-align: left;
    padding: 3px 6px;
    border-radius: 4px;
    cursor: pointer;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--v4-mono);
    font-size: 11px;
  }
  .row:hover {
    border-color: var(--v4-hair);
    color: var(--v4-ink);
  }
  .unpin {
    background: transparent;
    border: 0;
    color: var(--v4-ink6);
    cursor: pointer;
    font-size: 12px;
    padding: 0 4px;
  }
  .muted {
    color: var(--v4-ink5);
  }
  .hint {
    color: var(--v4-ink6);
    font-size: 10px;
  }
</style>
