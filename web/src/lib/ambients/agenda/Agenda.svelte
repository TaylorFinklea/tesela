<script lang="ts">
  import type { AmbientRendererProps } from "$lib/buffer/protocol";
  let { onNavigate }: AmbientRendererProps = $props();
  // Task 6 fills this in. For now, render a placeholder so the ambient
  // renderer registers and `:agenda` opens *something*.
  void onNavigate;
</script>

<div class="p-4 text-sm text-muted-foreground">agenda — coming up</div>
