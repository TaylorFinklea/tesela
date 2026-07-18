<script lang="ts">
  import type { Snippet } from "svelte";
  import { commandRegistry } from "$lib/command-registry.svelte";
  import GrIcon from "$lib/graphite/GrIcon.svelte";
  import GrWidget from "$lib/graphite/GrWidget.svelte";

  let {
    placementId,
    title,
    icon,
    badge,
    collapsed,
    index,
    total,
    children,
  }: {
    placementId: string;
    title: string;
    icon?: string;
    badge?: string;
    collapsed: boolean;
    index: number;
    total: number;
    children?: Snippet;
  } = $props();

  function run(id: string) {
    void commandRegistry.get(id)?.run(placementId);
  }
</script>

{#snippet controls()}
  <button
    type="button"
    class="manage"
    aria-label={`Move ${title} up`}
    title="Move up"
    disabled={index === 0}
    data-rail-action=""
    data-command-id="rail-move-widget-up"
    onclick={() => run("rail-move-widget-up")}
  ><GrIcon name="arrow-up" size={12} /></button>
  <button
    type="button"
    class="manage"
    aria-label={`Move ${title} down`}
    title="Move down"
    disabled={index === total - 1}
    data-rail-action=""
    data-command-id="rail-move-widget-down"
    onclick={() => run("rail-move-widget-down")}
  ><GrIcon name="arrow-down" size={12} /></button>
  <button
    type="button"
    class="manage danger"
    aria-label={`Remove ${title}`}
    title="Remove widget"
    data-rail-action=""
    data-command-id="rail-remove-widget"
    onclick={() => run("rail-remove-widget")}
  ><GrIcon name="x" size={12} /></button>
{/snippet}

<GrWidget
  {title}
  {icon}
  {badge}
  {collapsed}
  onToggle={() => run("rail-toggle-widget")}
  toggleCommandId="rail-toggle-widget"
  {controls}
>
  {#if children}{@render children()}{/if}
</GrWidget>

<style>
  .manage {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--faint);
    cursor: pointer;
  }
  .manage:hover:not(:disabled), .manage:focus-visible { color: var(--fg2); background: var(--bg); }
  .manage.danger:hover:not(:disabled) { color: var(--coral); background: var(--coral-dim); }
  .manage:disabled { opacity: .25; cursor: default; }
</style>
