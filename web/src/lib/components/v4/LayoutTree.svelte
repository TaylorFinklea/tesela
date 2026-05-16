<script lang="ts">
  /*
   * Prism v4 — recursive layout renderer.
   *
   * Walks the binary-tree `LayoutNode` and emits a flex DOM that mirrors
   * its structure: leaves render a `<PaneShell>`, splits render a flex
   * container whose direction matches the split's axis, with 1px drag
   * handles between adjacent children. The drag handler rewrites that
   * split's `sizes` array in-place via `setSplitSizes`.
   *
   * Recursion: a `<LayoutTree>` for a split renders one nested
   * `<LayoutTree>` per child, so the tree depth is unbounded.
   */
  import type { LayoutNode, SplitNode } from "$lib/stores/pane-tree";
  import { MIN_PANE_WEIGHT } from "$lib/stores/pane-tree";
  import { setSplitSizes } from "$lib/stores/pane-tree.svelte";
  import PaneShell from "$lib/components/v4/PaneShell.svelte";
  import LayoutTreeSelf from "$lib/components/v4/LayoutTree.svelte";

  let { node, focusedPaneId, activeDragRef }: {
    node: LayoutNode;
    focusedPaneId: string | undefined;
    /** Reactive `[boolean]` cell that the root layout uses to disable
     *  pointer events on descendants while a drag is active. Passed by
     *  reference so descendants can flip it during their own drags. */
    activeDragRef: { value: boolean };
  } = $props();

  let containerEl = $state<HTMLElement | undefined>();

  function beginDrag(ev: PointerEvent, split: SplitNode, leftIdx: number) {
    if (!containerEl) return;
    const rect = containerEl.getBoundingClientRect();
    const along = split.dir === "vertical" ? rect.width : rect.height;
    if (along <= 0) return;
    const startCoord = split.dir === "vertical" ? ev.clientX : ev.clientY;
    const sizesStart = split.sizes.slice();
    const totalW = sizesStart.reduce((s, w) => s + w, 0);
    const sL = sizesStart[leftIdx];
    const sR = sizesStart[leftIdx + 1];
    const sum = sL + sR;
    ev.preventDefault();
    activeDragRef.value = true;
    document.body.style.cursor = split.dir === "vertical" ? "col-resize" : "row-resize";
    const onMove = (e: PointerEvent) => {
      const cur = split.dir === "vertical" ? e.clientX : e.clientY;
      const dPx = cur - startCoord;
      const dW = (dPx / along) * totalW;
      let newL = sL + dW;
      newL = Math.max(MIN_PANE_WEIGHT, Math.min(sum - MIN_PANE_WEIGHT, newL));
      const next = sizesStart.slice();
      next[leftIdx] = newL;
      next[leftIdx + 1] = sum - newL;
      setSplitSizes(split.id, next);
    };
    const onUp = () => {
      activeDragRef.value = false;
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  }
</script>

{#if node.kind === "leaf"}
  <PaneShell pane={node.pane} focused={node.pane.id === focusedPaneId} />
{:else}
  <div
    bind:this={containerEl}
    class="v4-split"
    class:v4-split-v={node.dir === "vertical"}
    class:v4-split-h={node.dir === "horizontal"}
  >
    {#each node.children as child, i (child.kind === "leaf" ? child.pane.id : child.id)}
      <div class="v4-split-child" style="flex: {node.sizes[i]} 1 0; min-width: 0; min-height: 0;">
        <LayoutTreeSelf node={child} {focusedPaneId} {activeDragRef} />
      </div>
      {#if i < node.children.length - 1}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="v4-resizer"
          class:v4-resizer-col={node.dir === "vertical"}
          class:v4-resizer-row={node.dir === "horizontal"}
          role="separator"
          aria-orientation={node.dir === "vertical" ? "vertical" : "horizontal"}
          title="drag to resize"
          onpointerdown={(e) => beginDrag(e, node, i)}
        ></div>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .v4-split {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--v4-hair);
  }
  .v4-split-v { flex-direction: row; }
  .v4-split-h { flex-direction: column; }
  .v4-split-child {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* Any pane shell or nested split inside a split-child fills it. */
  .v4-split-child > :global(.v4-pane),
  .v4-split-child > :global(.v4-split) {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  .v4-resizer {
    background: var(--v4-hair);
    position: relative;
    flex-shrink: 0;
    transition: background 140ms;
  }
  .v4-resizer::after {
    content: "";
    position: absolute;
    inset: 0;
  }
  .v4-resizer-col {
    width: 1px;
    cursor: col-resize;
  }
  .v4-resizer-col::after { left: -3px; right: -3px; }
  .v4-resizer-row {
    height: 1px;
    cursor: row-resize;
  }
  .v4-resizer-row::after { top: -3px; bottom: -3px; }
  .v4-resizer:hover { background: var(--v4-accent-dim); }
</style>
