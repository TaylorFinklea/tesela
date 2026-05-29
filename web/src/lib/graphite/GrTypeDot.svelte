<!-- web/src/lib/graphite/GrTypeDot.svelte -->
<script lang="ts">
  let { type = 'note', size = 6 }: { type?: string; size?: number } = $props();
</script>
<span class="gr-dot" style="--d:{size}px;background:var(--{type});"></span>
<style>.gr-dot{width:var(--d);height:var(--d);border-radius:50%;flex-shrink:0;display:inline-block;}</style>
