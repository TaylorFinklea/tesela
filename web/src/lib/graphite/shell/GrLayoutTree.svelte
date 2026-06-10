<!-- web/src/lib/graphite/shell/GrLayoutTree.svelte -->
<script lang="ts">
  /*
   * Graphite pane-tree renderer. Walks the active tab's binary `Node` tree:
   * a leaf mounts a <GrLeaf> (which routes the buffer to a Graphite view);
   * a split renders a flex container with two children sized by the split's
   * `ratio`, plus a 1px drag handle that updates the ratio via `setRatio`.
   *
   * Structurally a Graphite-native twin of the legacy v5 LayoutTree
   * (deleted with the v5 chrome) — the same split/resizer/drag algebra —
   * but it mounts the Graphite Gr* views, so /g keeps its own presentation
   * (and tokens) while gaining vsplit/hsplit. (decisions.md 2026-06-06.)
   */
  import type { LeafId, Node, Split } from '$lib/buffer/types';
  import { setRatio } from '$lib/buffer/state.svelte';
  import GrLayoutTreeSelf from './GrLayoutTree.svelte';
  import GrLeaf from './GrLeaf.svelte';

  let {
    node,
    focusedLeafId,
    activeDragRef,
    showFocus,
  }: {
    node: Node;
    focusedLeafId: LeafId | undefined;
    activeDragRef: { value: boolean };
    showFocus: boolean;
  } = $props();

  let containerEl = $state<HTMLElement | undefined>();
  const MIN_RATIO = 0.05;

  function beginDrag(ev: PointerEvent, split: Split) {
    if (!containerEl) return;
    const rect = containerEl.getBoundingClientRect();
    const along = split.dir === 'v' ? rect.width : rect.height;
    if (along <= 0) return;
    const startCoord = split.dir === 'v' ? ev.clientX : ev.clientY;
    const startRatio = split.ratio;
    ev.preventDefault();
    activeDragRef.value = true;
    document.body.style.cursor = split.dir === 'v' ? 'col-resize' : 'row-resize';

    const onMove = (e: PointerEvent) => {
      const cur = split.dir === 'v' ? e.clientX : e.clientY;
      const dPx = cur - startCoord;
      const dR = dPx / along;
      const next = Math.max(MIN_RATIO, Math.min(1 - MIN_RATIO, startRatio + dR));
      setRatio(split.id, next);
    };
    const onUp = () => {
      activeDragRef.value = false;
      document.body.style.cursor = '';
      window.removeEventListener('pointermove', onMove);
      window.removeEventListener('pointerup', onUp);
    };
    window.addEventListener('pointermove', onMove);
    window.addEventListener('pointerup', onUp);
  }
</script>

{#if node.type === 'leaf'}
  <GrLeaf
    leafId={node.id}
    buffer={node.buffer}
    focused={node.id === focusedLeafId}
    {showFocus}
  />
{:else}
  <div
    bind:this={containerEl}
    class="gr-split"
    class:gr-split-v={node.dir === 'v'}
    class:gr-split-h={node.dir === 'h'}
  >
    <div class="gr-split-child" style="flex: {node.ratio} 1 0; min-width: 0; min-height: 0;">
      <GrLayoutTreeSelf node={node.children[0]} {focusedLeafId} {activeDragRef} {showFocus} />
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="gr-resizer"
      class:gr-resizer-col={node.dir === 'v'}
      class:gr-resizer-row={node.dir === 'h'}
      role="separator"
      aria-orientation={node.dir === 'v' ? 'vertical' : 'horizontal'}
      title="drag to resize"
      onpointerdown={(e) => beginDrag(e, node)}
    ></div>
    <div class="gr-split-child" style="flex: {1 - node.ratio} 1 0; min-width: 0; min-height: 0;">
      <GrLayoutTreeSelf node={node.children[1]} {focusedLeafId} {activeDragRef} {showFocus} />
    </div>
  </div>
{/if}

<style>
  .gr-split {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--line);
  }
  .gr-split-v {
    flex-direction: row;
  }
  .gr-split-h {
    flex-direction: column;
  }
  .gr-split-child {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* Any pane shell or nested split inside a split-child fills it. */
  .gr-split-child > :global(.gr-leaf),
  .gr-split-child > :global(.gr-split) {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  .gr-resizer {
    background: var(--line);
    position: relative;
    flex-shrink: 0;
    transition: background 140ms;
  }
  .gr-resizer::after {
    content: '';
    position: absolute;
    inset: 0;
  }
  .gr-resizer-col {
    width: 1px;
    cursor: col-resize;
  }
  .gr-resizer-col::after {
    left: -3px;
    right: -3px;
  }
  .gr-resizer-row {
    height: 1px;
    cursor: row-resize;
  }
  .gr-resizer-row::after {
    top: -3px;
    bottom: -3px;
  }
  .gr-resizer:hover {
    background: var(--coral-dim);
  }
</style>
