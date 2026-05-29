<!-- web/src/lib/graphite/views/GrDaily.svelte — Part A, Task A2.
     Daily journal in the pane. REUSES JournalView (the Logseq-style
     continuous day stack: lazy-create, cross-day j/k nav, infinite scroll),
     wrapped in a `.gr-outline` scroll container. The Graphite block look
     comes from A1's variable remap + decoration overrides (graphite-editor.css)
     — JournalView and BlockOutliner are imported READ-ONLY. -->
<script lang="ts">
  import JournalView from "$lib/components/JournalView.svelte";

  let { anchorDate }: { anchorDate?: string } = $props();

  // Default the anchor to the user's LOCAL today (same rule JournalView uses
  // internally so we land on the freshly-seeded trailing block).
  const todayStr = (() => {
    const d = new Date();
    const y = d.getFullYear();
    const m = String(d.getMonth() + 1).padStart(2, "0");
    const day = String(d.getDate()).padStart(2, "0");
    return `${y}-${m}-${day}`;
  })();

  const anchor = $derived(anchorDate || todayStr);
</script>

<div class="gr-outline">
  <JournalView anchorDate={anchor} />
</div>

<style>
  .gr-outline {
    flex: 1;
    overflow: auto;
    padding: 14px 18px;
    min-height: 0;
  }
</style>
