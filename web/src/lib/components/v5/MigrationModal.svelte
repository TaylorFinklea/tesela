<script lang="ts">
  /*
   * One-time modal shown on the first v5 boot after a v4→v5 state migration.
   * Surfaces dropped/converted panes so the user doesn't think they lost
   * work.
   */
  import type { MigrationReport } from "$lib/buffer/migration";

  let {
    report,
    onclose,
  }: {
    report: MigrationReport;
    onclose: () => void;
  } = $props();

  const items = $derived.by(() => {
    const out: string[] = [];
    if (report.droppedGraph > 0) {
      out.push(
        `${report.droppedGraph} graph pane${report.droppedGraph === 1 ? "" : "s"} dropped (open via ⌘G overlay).`,
      );
    }
    if (report.convertedContextDerived > 0) {
      out.push(
        `${report.convertedContextDerived} context pane${report.convertedContextDerived === 1 ? "" : "s"} converted to a backlinks derived buffer.`,
      );
    }
    if (report.convertedWidgetAmbient > 0) {
      out.push(
        `${report.convertedWidgetAmbient} widget pane${report.convertedWidgetAmbient === 1 ? "" : "s"} converted to ambient buffers.`,
      );
    }
    if (report.convertedDashboardAmbient > 0) {
      out.push(
        `${report.convertedDashboardAmbient} dashboard pane${report.convertedDashboardAmbient === 1 ? "" : "s"} converted to the workspace dashboard ambient.`,
      );
    }
    if (report.droppedExtraEditorTiles > 0) {
      out.push(
        `${report.droppedExtraEditorTiles} extra stacked tile${report.droppedExtraEditorTiles === 1 ? "" : "s"} dropped (v5 page buffers are single-tile).`,
      );
    }
    if (report.unmappedWidgets.length > 0) {
      out.push(
        `Unmapped widget${report.unmappedWidgets.length === 1 ? "" : "s"} (defaulted to workspace dashboard): ${report.unmappedWidgets.join(", ")}.`,
      );
    }
    return out;
  });
</script>

{#if items.length > 0}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="v5-modal-scrim" onclick={onclose}>
    <div class="v5-modal" onclick={(e) => e.stopPropagation()}>
      <h2>Welcome to Prism v5</h2>
      <p class="v5-modal-sub">
        Your v4 panes were migrated. A few things changed:
      </p>
      <ul>
        {#each items as item}
          <li>{item}</li>
        {/each}
      </ul>
      <p class="v5-modal-sub">All editing was preserved.</p>
      <div class="v5-modal-actions">
        <button type="button" onclick={onclose}>got it</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .v5-modal-scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    z-index: 100;
    display: flex;
    align-items: center;
    justify-content: center;
    backdrop-filter: blur(2px);
  }
  .v5-modal {
    background: var(--v4-bg);
    color: var(--v4-ink);
    border: 1px solid var(--v4-hair);
    border-radius: 8px;
    padding: 20px 24px;
    max-width: 520px;
    font-family: var(--v4-sans);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.6);
  }
  .v5-modal h2 {
    margin: 0 0 10px;
    font-size: 16px;
    color: var(--v4-ink);
  }
  .v5-modal-sub {
    color: var(--v4-ink5);
    font-size: 12px;
    margin: 0 0 12px;
  }
  .v5-modal ul {
    list-style: none;
    margin: 0 0 12px;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .v5-modal li {
    font-size: 12px;
    color: var(--v4-ink2);
    padding-left: 14px;
    position: relative;
  }
  .v5-modal li::before {
    content: "•";
    position: absolute;
    left: 0;
    color: var(--v4-accent);
  }
  .v5-modal-actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 16px;
  }
  .v5-modal-actions button {
    background: var(--v4-accent);
    color: var(--v4-bg);
    border: 0;
    border-radius: 5px;
    padding: 6px 14px;
    font-family: var(--v4-mono);
    font-size: 12px;
    cursor: pointer;
  }
</style>
