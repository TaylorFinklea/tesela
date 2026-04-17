<script lang="ts">
  let {
    onresize,
  }: {
    onresize: (ratio: number) => void;
  } = $props();

  let dragging = $state(false);

  function handleMouseDown(e: MouseEvent) {
    e.preventDefault();
    dragging = true;

    const divider = e.currentTarget as HTMLElement;
    const container = divider.parentElement;
    if (!container) return;

    const handleMove = (ev: MouseEvent) => {
      const rect = container.getBoundingClientRect();
      const y = ev.clientY - rect.top;
      const ratio = (y / rect.height) * 100;
      onresize(Math.max(20, Math.min(80, ratio)));
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
    document.body.style.cursor = "row-resize";
    document.body.style.userSelect = "none";
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  class="split-divider shrink-0 cursor-row-resize transition-colors"
  class:is-dragging={dragging}
  onmousedown={handleMouseDown}
></div>

<style>
  .split-divider {
    height: 4px;
    background: var(--border);
  }
  .split-divider:hover {
    background: color-mix(in srgb, var(--primary) 30%, transparent);
  }
  .split-divider.is-dragging {
    background: color-mix(in srgb, var(--primary) 40%, transparent);
  }
</style>
