<script lang="ts">
  /*
   * Prism v4 — the pane-kind chip on the right of a pane header. Shows
   * an icon + label for the current kind; an invisible <select> sits on
   * top so clicking the chip opens the native picker. Swapping kind
   * replaces the pane's body via the store's `swapKind` (the pane id is
   * preserved).
   */
  import type { Pane, PaneKind } from "$lib/stores/pane-tree";

  let {
    pane,
    onSwapKind,
  }: {
    pane: Pane;
    onSwapKind: (kind: PaneKind) => void;
  } = $props();

  const KIND_META: Record<PaneKind, { icon: string; label: string }> = {
    editor: { icon: "≡", label: "editor" },
    widget: { icon: "◐", label: "widget" },
    context: { icon: "◑", label: "context" },
    graph: { icon: "✦", label: "graph" },
    dashboard: { icon: "✶", label: "dashboard" },
  };

  const meta = $derived(KIND_META[pane.kind]);
</script>

<div class="v4-kind-menu">
  <span class="v4-kind-chip">
    <span class="v4-kind-icon">{meta.icon}</span>
    <span>{meta.label}</span>
  </span>
  <select
    class="v4-kind-select"
    value={pane.kind}
    onclick={(e) => e.stopPropagation()}
    onchange={(e) => onSwapKind(e.currentTarget.value as PaneKind)}
    title="change pane kind"
  >
    {#each Object.entries(KIND_META) as [kind, m] (kind)}
      <option value={kind}>{m.icon}  {m.label}</option>
    {/each}
  </select>
</div>

<style>
  .v4-kind-menu {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .v4-kind-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 2px 8px 2px 7px;
    border-radius: 5px;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink3);
    font-family: var(--v4-mono);
    font-size: 10.5px;
    letter-spacing: 0.4px;
    pointer-events: none;
  }
  .v4-kind-icon {
    color: var(--v4-accent);
    font-size: 11px;
  }
  .v4-kind-select {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0;
    cursor: pointer;
    appearance: none;
  }
</style>
