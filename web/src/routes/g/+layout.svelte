<!-- web/src/routes/g/+layout.svelte -->
<script lang="ts">
  import '$lib/graphite/tokens.css';
  let { children } = $props();
</script>

<div class="gr-root">
  {@render children()}
</div>

<style>
  .gr-root { min-height: 100vh; }
</style>
