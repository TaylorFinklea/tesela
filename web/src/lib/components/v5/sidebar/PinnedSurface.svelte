<script lang="ts">
  /* Pinned surface — pages + blocks. Right-click a block to pin it. */
  import {
    getPinned,
    getPinnedBlocks,
    unpin,
    unpinBlock,
  } from "$lib/state/shared.svelte";
  import { openPageInFocused } from "$lib/buffer/state.svelte";
  import { asPageId } from "$lib/buffer/types";

  const pinned = $derived(getPinned());
  const blocks = $derived(getPinnedBlocks());
</script>

<div class="v5-side-surface">
  <header>Pinned</header>
  {#if pinned.length === 0 && blocks.length === 0}
    <p class="muted">no pinned</p>
    <p class="hint">★ a note in the tree · or right-click a block</p>
  {:else}
    {#if pinned.length > 0}
      <section>
        <h3>Pages</h3>
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
                class="x"
                title="unpin"
                onclick={() => unpin(id)}
              >×</button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
    {#if blocks.length > 0}
      <section>
        <h3>Blocks</h3>
        <ul>
          {#each blocks as b (b.pageId + ":" + b.blockId)}
            <li>
              <button
                type="button"
                class="row"
                onclick={() => openPageInFocused(asPageId(b.pageId))}
                title={`${b.pageId} · ${b.blockId}`}
              >
                <span class="preview">{b.preview}</span>
                <span class="src">{b.pageId}</span>
              </button>
              <button
                type="button"
                class="x"
                title="unpin"
                onclick={() => unpinBlock(b.pageId, b.blockId)}
              >×</button>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
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
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  header {
    color: var(--v4-ink);
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.7px;
  }
  h3 {
    margin: 0 0 6px;
    color: var(--v4-ink5);
    font-size: 9.5px;
    letter-spacing: 0.5px;
    text-transform: uppercase;
    font-weight: 500;
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
    display: flex;
    flex-direction: column;
    gap: 1px;
  }
  .row:hover {
    border-color: var(--v4-hair);
    color: var(--v4-ink);
  }
  .preview {
    color: var(--v4-ink2);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .src {
    color: var(--v4-ink6);
    font-size: 9.5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .x {
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
