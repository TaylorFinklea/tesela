<script lang="ts">
  /* Derived renderer wrapper: backlinks-of-page. Resolves the reference's
   * page path and mounts the existing v4 BacklinksTab. */
  import BacklinksTab from "$lib/components/v4/BacklinksTab.svelte";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";

  let {
    reference,
    onNavigate,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();
</script>

<BacklinksTab
  noteId={reference.path || undefined}
  onOpenNote={(id) =>
    onNavigate({ kind: "open-page", path: id, how: "replace" })}
/>
