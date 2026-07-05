<script lang="ts">
  /* Placeholder per spec — "coming in a later phase". */
  import type { AmbientRendererProps } from "$lib/buffer/protocol";

  let {}: AmbientRendererProps = $props();
</script>

<div class="v5-ai">
  <h2>AI workspace</h2>
  <p class="muted">Coming in a later phase.</p>
  <button type="button" disabled>chat about focused page</button>
</div>

<style>
  .v5-ai {
    flex: 1;
    min-height: 0;
    padding: 20px 22px;
    font-family: var(--theme-font-sans);
    color: var(--fg-muted);
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
  }
  h2 {
    margin: 0;
    font-size: 14px;
    color: var(--fg-default);
  }
  .muted {
    color: var(--fg-faint);
    font-size: 12px;
  }
  button {
    background: transparent;
    border: 1px solid var(--line-soft);
    color: var(--fg-faint);
    border-radius: 5px;
    padding: 4px 10px;
    font-family: var(--theme-font-mono);
    font-size: 11px;
    cursor: not-allowed;
  }
</style>
