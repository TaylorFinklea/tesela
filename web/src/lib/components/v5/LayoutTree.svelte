<script lang="ts">
  /*
   * Prism v5 — recursive layout renderer for the binary pane tree.
   *
   * Walks a `Node`. Leaves mount a `<BufferShell>`. Splits render a flex
   * container with two children sized via the split's `ratio`, plus a 1px
   * drag handle that updates the ratio via `setRatio`.
   */
  import type { LeafId, Node, Split } from "$lib/buffer/types";
  import { setRatio } from "$lib/buffer/state.svelte";
  import BufferShell from "./BufferShell.svelte";
  import LayoutTreeSelf from "./LayoutTree.svelte";

  let {
    node,
    focusedLeafId,
    activeDragRef,
  }: {
    node: Node;
    focusedLeafId: LeafId | undefined;
    activeDragRef: { value: boolean };
  } = $props();

  let containerEl = $state<HTMLElement | undefined>();
  const MIN_RATIO = 0.05;

  function beginDrag(ev: PointerEvent, split: Split) {
    if (!containerEl) return;
    const rect = containerEl.getBoundingClientRect();
    const along = split.dir === "v" ? rect.width : rect.height;
    if (along <= 0) return;
    const startCoord = split.dir === "v" ? ev.clientX : ev.clientY;
    const startRatio = split.ratio;
    ev.preventDefault();
    activeDragRef.value = true;
    document.body.style.cursor =
      split.dir === "v" ? "col-resize" : "row-resize";

    const onMove = (e: PointerEvent) => {
      const cur = split.dir === "v" ? e.clientX : e.clientY;
      const dPx = cur - startCoord;
      const dR = dPx / along;
      const next = Math.max(MIN_RATIO, Math.min(1 - MIN_RATIO, startRatio + dR));
      setRatio(split.id, next);
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

{#if node.type === "leaf"}
  <BufferShell
    leafId={node.id}
    buffer={node.buffer}
    focused={node.id === focusedLeafId}
  />
{:else}
  <div
    bind:this={containerEl}
    class="v5-split"
    class:v5-split-v={node.dir === "v"}
    class:v5-split-h={node.dir === "h"}
  >
    <div class="v5-split-child" style="flex: {node.ratio} 1 0; min-width: 0; min-height: 0;">
      <LayoutTreeSelf
        node={node.children[0]}
        {focusedLeafId}
        {activeDragRef}
      />
    </div>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="v5-resizer"
      class:v5-resizer-col={node.dir === "v"}
      class:v5-resizer-row={node.dir === "h"}
      role="separator"
      aria-orientation={node.dir === "v" ? "vertical" : "horizontal"}
      title="drag to resize"
      onpointerdown={(e) => beginDrag(e, node)}
    ></div>
    <div class="v5-split-child" style="flex: {1 - node.ratio} 1 0; min-width: 0; min-height: 0;">
      <LayoutTreeSelf
        node={node.children[1]}
        {focusedLeafId}
        {activeDragRef}
      />
    </div>
  </div>
{/if}

<style>
  .v5-split {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--v4-hair);
  }
  .v5-split-v {
    flex-direction: row;
  }
  .v5-split-h {
    flex-direction: column;
  }
  .v5-split-child {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow: hidden;
  }
  /* Any buffer shell or nested split inside a split-child fills it. */
  .v5-split-child > :global(.v5-buffer),
  .v5-split-child > :global(.v5-split) {
    flex: 1;
    min-height: 0;
    min-width: 0;
  }

  .v5-resizer {
    background: var(--v4-hair);
    position: relative;
    flex-shrink: 0;
    transition: background 140ms;
  }
  .v5-resizer::after {
    content: "";
    position: absolute;
    inset: 0;
  }
  .v5-resizer-col {
    width: 1px;
    cursor: col-resize;
  }
  .v5-resizer-col::after {
    left: -3px;
    right: -3px;
  }
  .v5-resizer-row {
    height: 1px;
    cursor: row-resize;
  }
  .v5-resizer-row::after {
    top: -3px;
    bottom: -3px;
  }
  .v5-resizer:hover {
    background: var(--v4-accent-dim);
  }
</style>
