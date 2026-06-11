<!-- web/src/lib/graphite/GrChip.svelte -->
<script lang="ts">
  import type { HTMLButtonAttributes } from 'svelte/elements';
  let { active = false, count, children, ...rest }:
    { active?: boolean; count?: number; children?: any } & HTMLButtonAttributes = $props();
</script>

<button class="gr-chip" class:active {...rest}>
  {#if children}{@render children()}{/if}
  {#if count !== undefined}<span class="n">{count}</span>{/if}
</button>

<style>
  .gr-chip{display:inline-flex;align-items:center;gap:6px;height:26px;padding:0 11px;border-radius:8px;
    cursor:pointer;background:var(--raised);border:1px solid var(--line);color:var(--fg2);
    font-size:12px;font-family:var(--sans);transition:all .14s;}
  .gr-chip:hover{border-color:var(--line-2);}
  .gr-chip.active{background:var(--coral-dim);border-color:var(--coral-line);color:var(--coral);}
  .gr-chip .n{font-family:var(--mono);font-size:10px;color:var(--faint);}
  .gr-chip.active .n{color:var(--coral);}
</style>
