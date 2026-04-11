<script lang="ts">
  import { page } from "$app/state";
  import { getConnected } from "$lib/ws-client.svelte";

  let { vimMode = "NORMAL" }: { vimMode?: string } = $props();

  const wsConnected = $derived(getConnected());
  const currentPath = $derived(page.url.pathname);
  const noteName = $derived(
    currentPath.startsWith("/p/")
      ? decodeURIComponent(currentPath.slice(3))
      : currentPath === "/"
        ? "Home"
        : currentPath.slice(1),
  );
</script>

<div class="h-6 bg-surface border-t border-border flex items-center px-3 gap-4 text-[11px] font-mono shrink-0 select-none">
  <!-- Vim mode -->
  <span class="font-bold {vimMode === 'INSERT' ? 'text-emerald-400' : vimMode === 'VISUAL' ? 'text-amber-400' : 'text-primary/70'}">
    {vimMode}
  </span>

  <!-- Current note -->
  <span class="text-muted-foreground/60 truncate">{noteName}</span>

  <!-- Spacer -->
  <div class="flex-1"></div>

  <!-- Connection status -->
  <div class="flex items-center gap-1.5">
    <span class="inline-block h-1.5 w-1.5 rounded-full {wsConnected ? 'bg-emerald-500/60' : 'bg-muted-foreground/30'}"></span>
    <span class="text-muted-foreground/40">{wsConnected ? "connected" : "offline"}</span>
  </div>
</div>
