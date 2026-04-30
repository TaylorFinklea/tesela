<script lang="ts">
  /**
   * Side-by-side diff modal for a historical note version (Phase 9.3).
   *
   * Props:
   *   - prev   — historical content (from `note_versions.content`)
   *   - next   — current content
   *   - title  — page title for the header
   *   - timestamp — relative time of the historical version
   *   - onclose — modal-dismiss callback
   *   - onrestore — restore callback (parent issues PUT)
   */
  import { lineDiff, relativeTime, type DiffRow } from "$lib/line-diff";

  type Props = {
    prev: string;
    next: string;
    title: string;
    versionTimestamp: string;
    onclose: () => void;
    onrestore: () => void;
  };
  let { prev, next, title, versionTimestamp, onclose, onrestore }: Props = $props();

  const diff = $derived(lineDiff(prev, next));
  const relLabel = $derived(relativeTime(versionTimestamp));

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }

  function rowClass(r: DiffRow): string {
    if (r.kind === "same") return "diff-same";
    if (r.kind === "added") return "diff-added";
    return "diff-removed";
  }
</script>

<svelte:window onkeydown={handleKey} />

<div
  class="modal-backdrop"
  role="dialog"
  aria-modal="true"
  tabindex="-1"
  onclick={(e) => { if (e.target === e.currentTarget) onclose(); }}
  onkeydown={(e) => { if (e.key === "Enter" && e.target === e.currentTarget) onclose(); }}
>
  <div class="modal">
    <div class="modal-head">
      <span class="t">{title}</span>
      <span class="s">restored from {relLabel} · +{diff.added} −{diff.removed}</span>
      <span class="sp"></span>
      <button type="button" class="restore" onclick={onrestore}>Restore this version</button>
      <button type="button" class="close" onclick={onclose} aria-label="Close">×</button>
    </div>
    <div class="diff-body">
      {#each diff.rows as r}
        <div class={`diff-row ${rowClass(r)}`}>
          <span class="ln">{r.kind === "added" ? "" : r.prevLine}</span>
          <span class="ln">{r.kind === "removed" ? "" : r.nextLine}</span>
          <span class="marker">{r.kind === "added" ? "+" : r.kind === "removed" ? "−" : " "}</span>
          <span class="text">{r.text}</span>
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed; inset: 0; background: rgba(0,0,0,0.55);
    display: grid; place-items: center; z-index: 200;
  }
  .modal {
    background: var(--v9-bg-2);
    border: 1px solid var(--v9-line);
    border-radius: 6px;
    width: min(900px, 90vw);
    max-height: 80vh;
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .modal-head {
    display: flex; align-items: center; gap: 10px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--v9-line);
    background: var(--v9-bg-3);
    font-family: var(--v9-mono);
    font-size: 11px;
  }
  .modal-head .t { color: var(--v9-amber); font-weight: 600; }
  .modal-head .s { color: var(--v9-ink-3); }
  .modal-head .sp { flex: 1; }
  .modal-head .restore {
    background: var(--v9-amber); color: var(--v9-bg);
    border: none; padding: 4px 10px; border-radius: 3px;
    font-family: var(--v9-mono); font-size: 11px; font-weight: 700;
    cursor: pointer;
  }
  .modal-head .close {
    background: transparent; color: var(--v9-ink-3);
    border: none; cursor: pointer; font-size: 18px; line-height: 1;
    padding: 0 6px;
  }
  .diff-body {
    flex: 1; overflow-y: auto;
    font-family: var(--v9-mono); font-size: 11.5px; line-height: 1.5;
    padding: 6px 0;
  }
  .diff-row {
    display: grid;
    grid-template-columns: 36px 36px 16px 1fr;
    gap: 4px;
    padding: 0 14px;
    white-space: pre;
  }
  .diff-row.diff-added { background: color-mix(in srgb, var(--v9-sage) 12%, transparent); }
  .diff-row.diff-removed { background: color-mix(in srgb, var(--v9-rose) 12%, transparent); }
  .diff-row .ln {
    color: var(--v9-ink-faint);
    text-align: right;
    font-size: 10px;
  }
  .diff-row .marker {
    text-align: center;
    color: var(--v9-ink-3);
  }
  .diff-row.diff-added .marker { color: var(--v9-sage); }
  .diff-row.diff-removed .marker { color: var(--v9-rose); }
  .diff-row .text {
    color: var(--v9-ink);
    overflow-x: auto;
  }
</style>
