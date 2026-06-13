<script lang="ts">
  import { onMount } from "svelte";

  let {
    input,
    match,
    position,
    onuseexisting,
    oncreatenew,
  }: {
    input: string;
    match: string;
    position: { x: number; y: number };
    onuseexisting: () => void;
    oncreatenew: () => void;
  } = $props();

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      e.stopPropagation();
      onuseexisting();
    } else if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      oncreatenew();
    } else {
      e.preventDefault();
      e.stopPropagation();
    }
  }

  onMount(() => {
    const mouseHandler = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      if (!target.closest(".new-entity-guard")) oncreatenew();
    };
    document.addEventListener("keydown", handleKeydown, true);
    document.addEventListener("mousedown", mouseHandler);
    return () => {
      document.removeEventListener("keydown", handleKeydown, true);
      document.removeEventListener("mousedown", mouseHandler);
    };
  });
</script>

<div
  class="new-entity-guard fixed z-50 rounded-md border border-border bg-popover text-popover-foreground shadow-lg w-64 p-2"
  style="left: {position.x}px; top: {position.y}px"
  role="dialog"
  aria-label="Confirm new entity"
>
  <div class="text-[12px] leading-snug mb-2">
    Did you mean <strong class="text-primary font-semibold">{match}</strong>?
  </div>
  <div class="text-[10px] text-muted-foreground/60 mb-2 truncate">Typed: {input}</div>
  <div class="flex items-center gap-1.5">
    <button
      type="button"
      class="guard-btn guard-btn-primary"
      onclick={onuseexisting}
    >Use existing</button>
    <button
      type="button"
      class="guard-btn"
      onclick={oncreatenew}
    >Create new</button>
  </div>
  <div class="guard-hint"><kbd>↵</kbd> use existing <span>·</span> <kbd>Esc</kbd> create new</div>
</div>

<style>
  .guard-btn {
    border: 1px solid var(--border, var(--v9-line));
    border-radius: 4px;
    padding: 3px 7px;
    font-size: 11px;
    color: var(--foreground);
    background: color-mix(in srgb, var(--foreground) 6%, transparent);
  }
  .guard-btn:hover { background: color-mix(in srgb, var(--primary) 12%, transparent); }
  .guard-btn-primary {
    color: var(--primary);
    border-color: color-mix(in srgb, var(--primary) 45%, transparent);
    background: color-mix(in srgb, var(--primary) 10%, transparent);
  }
  .guard-hint {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: 7px;
    font-size: 9.5px;
    color: var(--v9-ink-faint, var(--muted-foreground));
    font-family: var(--v9-mono, ui-monospace, monospace);
  }
  .guard-hint kbd {
    border: 1px solid var(--border, var(--v9-line));
    border-radius: 3px;
    padding: 0 3px;
    color: var(--primary);
    background: color-mix(in srgb, var(--foreground) 7%, transparent);
  }
  .guard-hint span { opacity: 0.45; }
</style>
