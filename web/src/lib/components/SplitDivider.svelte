<script lang="ts">
  /**
   * Drag-resizable divider between two panes. Supports horizontal (top/bottom
   * pane stack — used by the kanban split) and vertical (left/right pane stack
   * — used by Phase 9.5 focus-pane vsplit) orientations.
   *
   * pixelMode = true: onresize receives absolute pixels instead of a 0-100 ratio.
   *   horizontal → height from bottom edge (window.innerHeight - clientY)
   *   vertical   → width from right edge  (window.innerWidth  - clientX)
   */
  type Orientation = "horizontal" | "vertical";

  let {
    onresize,
    orientation = "horizontal",
    pixelMode = false,
  }: {
    onresize: (value: number) => void;
    orientation?: Orientation;
    pixelMode?: boolean;
  } = $props();

  let dragging = $state(false);

  function handleMouseDown(e: MouseEvent) {
    e.preventDefault();
    dragging = true;

    const divider = e.currentTarget as HTMLElement;
    const container = divider.parentElement;
    if (!container) return;
    const isVertical = orientation === "vertical";

    const handleMove = (ev: MouseEvent) => {
      if (pixelMode) {
        if (isVertical) {
          // Width from right edge of viewport.
          onresize(window.innerWidth - ev.clientX);
        } else {
          // Height from bottom edge of viewport.
          onresize(window.innerHeight - ev.clientY);
        }
      } else {
        const rect = container.getBoundingClientRect();
        const ratio = isVertical
          ? ((ev.clientX - rect.left) / rect.width) * 100
          : ((ev.clientY - rect.top) / rect.height) * 100;
        onresize(Math.max(20, Math.min(80, ratio)));
      }
    };

    const handleUp = () => {
      dragging = false;
      document.removeEventListener("mousemove", handleMove);
      document.removeEventListener("mouseup", handleUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };

    document.addEventListener("mousemove", handleMove);
    document.addEventListener("mouseup", handleUp);
    document.body.style.cursor = isVertical ? "col-resize" : "row-resize";
    document.body.style.userSelect = "none";
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="split-divider shrink-0 transition-colors {orientation === 'vertical' ? 'vertical' : 'horizontal'}"
  class:is-dragging={dragging}
  onmousedown={handleMouseDown}
></div>

<style>
  .split-divider {
    background: var(--border);
  }
  .split-divider.horizontal {
    height: 4px;
    cursor: row-resize;
  }
  .split-divider.vertical {
    width: 4px;
    cursor: col-resize;
  }
  .split-divider:hover {
    background: color-mix(in srgb, var(--primary) 30%, transparent);
  }
  .split-divider.is-dragging {
    background: color-mix(in srgb, var(--primary) 40%, transparent);
  }
</style>
