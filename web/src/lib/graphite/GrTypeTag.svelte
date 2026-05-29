<!-- web/src/lib/graphite/GrTypeTag.svelte -->
<script lang="ts">
  let { type = 'project', children }: { type?: string; children?: any } = $props();
</script>
<span class="gr-typetag" style="color:var(--{type});">
  <span class="sw" style="background:var(--{type});"></span>{#if children}{@render children()}{/if}
</span>
<style>
  .gr-typetag{display:inline-flex;align-items:center;gap:6px;height:21px;padding:0 9px;border-radius:6px;
    font-family:var(--mono);font-size:10.5px;letter-spacing:.02em;background:var(--raised);
    border:1px solid var(--line-2);}
  .gr-typetag .sw{width:6px;height:6px;border-radius:2px;flex-shrink:0;}
</style>
