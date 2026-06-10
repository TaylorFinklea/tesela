<script lang="ts">
  import OutlineTab from "$lib/components/shell/OutlineTab.svelte";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";

  let {
    reference,
    onNavigate,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();
</script>

<OutlineTab
  noteId={reference.path || undefined}
  onOpenNote={(id) =>
    onNavigate({ kind: "open-page", path: id, how: "replace" })}
/>
