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
    font-family: var(--v4-sans);
    color: var(--v4-ink2);
    display: flex;
    flex-direction: column;
    gap: 10px;
    align-items: flex-start;
  }
  h2 {
    margin: 0;
    font-size: 14px;
    color: var(--v4-ink);
  }
  .muted {
    color: var(--v4-ink5);
    font-size: 12px;
  }
  button {
    background: transparent;
    border: 1px solid var(--v4-hair);
    color: var(--v4-ink5);
    border-radius: 5px;
    padding: 4px 10px;
    font-family: var(--v4-mono);
    font-size: 11px;
    cursor: not-allowed;
  }
</style>
