<!-- web/src/lib/graphite/GrWidget.svelte -->
<script lang="ts">
  import type { Snippet } from 'svelte';
  import GrIcon from './GrIcon.svelte';
  let {
    title,
    icon,
    badge,
    collapsed = false,
    onToggle,
    toggleCommandId,
    controls,
    children,
  }: {
    title: string;
    icon?: string;
    badge?: string;
    collapsed?: boolean;
    onToggle?: () => void;
    toggleCommandId?: string;
    controls?: Snippet;
    children?: Snippet;
  } = $props();
</script>
<section class="gr-w">
  <header class="gr-w-head">
    {#if icon}<span class="ic"><GrIcon name={icon} size={14} /></span>{/if}
    <span class="ti">{title}</span>
    {#if badge}<span class="bd">{badge}</span>{/if}
    {#if controls}<span class="controls">{@render controls()}</span>{/if}
    {#if onToggle}
      <button
        type="button"
        class="caret"
        class:collapsed
        aria-label={`${collapsed ? 'Expand' : 'Collapse'} ${title}`}
        aria-expanded={!collapsed}
        data-rail-action=""
        data-command-id={toggleCommandId}
        onclick={onToggle}
      ><GrIcon name="chevron-down" size={14} /></button>
    {:else}
      <span class="caret"><GrIcon name="chevron-down" size={14} /></span>
    {/if}
  </header>
  {#if !collapsed}<div class="gr-w-body">{#if children}{@render children()}{/if}</div>{/if}
</section>
<style>
  .gr-w{background:var(--raised);border:1px solid var(--line);border-radius:11px;overflow:hidden;}
  .gr-w-head{display:flex;align-items:center;gap:8px;padding:9px 11px 7px;}
  .gr-w-head .ic{color:var(--subtle);display:flex;}
  .gr-w-head .ti{flex:1;font-size:11px;font-weight:600;letter-spacing:.04em;text-transform:uppercase;color:var(--fg2);}
  .gr-w-head .bd{font-family:var(--mono);font-size:10px;color:var(--subtle);background:var(--bg);
    border:1px solid var(--line);border-radius:5px;padding:1px 6px;white-space:nowrap;}
  .gr-w-head .controls{display:flex;align-items:center;gap:1px;}
  .gr-w-head .caret{color:var(--faint);margin-left:2px;display:flex;border:0;background:transparent;padding:2px;cursor:pointer;}
  .gr-w-head .caret.collapsed{transform:rotate(-90deg);}
  .gr-w-body{padding:2px 7px 9px;}
</style>
