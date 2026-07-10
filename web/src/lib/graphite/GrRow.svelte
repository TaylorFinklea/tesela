<!-- web/src/lib/graphite/GrRow.svelte -->
<script lang="ts">
  import GrIcon from './GrIcon.svelte';
  let { icon, label, meta, urgent = false, active = false, onclick, ...rest }:
    {
      icon?: string;
      label?: string;
      meta?: string;
      urgent?: boolean;
      active?: boolean;
      onclick?: (event: MouseEvent) => void;
      [key: string]: unknown;
    } = $props();
</script>
<button type="button" class="gr-row" class:active onclick={onclick} {...rest}>
  {#if icon}<span class="ic"><GrIcon name={icon} size={15} /></span>{/if}
  <span class="lb">{label}</span>
  {#if meta}<span class="mt" class:urg={urgent}>{meta}</span>{/if}
</button>
<style>
  .gr-row{display:flex;align-items:center;gap:9px;width:100%;padding:6px 8px;border:0;border-radius:7px;
    background:transparent;text-align:left;font:inherit;cursor:pointer;color:var(--fg2);font-size:12.5px;transition:background .12s;}
  .gr-row:hover,.gr-row.active{background:var(--raised-2);}
  .gr-row.active{color:var(--fg);}
  .gr-row .ic{color:var(--subtle);display:flex;}
  .gr-row .lb{flex:1;overflow:hidden;text-overflow:ellipsis;white-space:nowrap;}
  .gr-row .mt{font-family:var(--mono);font-size:10.5px;color:var(--faint);
    font-variant-numeric:tabular-nums;white-space:nowrap;}
  .gr-row .mt.urg{color:var(--coral);}
</style>
