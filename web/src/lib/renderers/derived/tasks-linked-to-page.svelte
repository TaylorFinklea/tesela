<script lang="ts">
  import LinkedTasksTab from "$lib/components/LinkedTasksTab.svelte";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";

  let {
    reference,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();
</script>

<LinkedTasksTab noteId={reference.path || ""} />
