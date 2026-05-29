<!-- web/src/lib/graphite/GrButton.svelte -->
<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  import GrIcon from './GrIcon.svelte';
  let { variant = 'ghost', icon, children, ...rest }:
    { variant?: 'ghost' | 'cta'; icon?: string; children?: any } & HTMLButtonAttributes = $props();
  const iconOnly = $derived(icon && !children);
</script>

<button class="gr-btn" class:cta={variant === 'cta'} class:ic={iconOnly} {...rest}>
  {#if icon}<GrIcon name={icon} size={iconOnly ? 17 : 15} />{/if}
  {#if children}{@render children()}{/if}
</button>

<style>
  .gr-btn{height:28px;padding:0 11px;border-radius:8px;font-size:12px;display:inline-flex;
    align-items:center;gap:6px;white-space:nowrap;cursor:pointer;font-family:var(--sans);
    background:var(--raised);border:1px solid var(--line-2);color:var(--fg2);transition:all .14s;}
  .gr-btn:hover{background:var(--raised-2);color:var(--fg);}
  .gr-btn.cta{background:var(--coral);color:#10110f;border-color:transparent;font-weight:600;}
  .gr-btn.ic{width:30px;height:30px;padding:0;justify-content:center;background:transparent;
    border-color:transparent;color:var(--subtle);}
  .gr-btn.ic:hover{background:var(--raised);color:var(--fg);}
</style>
