<script lang="ts">
  import PropertiesView from "$lib/components/v4/PropertiesView.svelte";
  import type { DerivedRendererProps } from "$lib/buffer/protocol";

  let {
    reference,
  }: DerivedRendererProps<{ kind: "page"; path: string }> = $props();
</script>

<PropertiesView noteId={reference.path || undefined} focusedBlock={null} />
