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

<div class="h-7 bg-surface border-t border-border flex items-center px-4 gap-4 text-[11px] font-mono shrink-0 select-none">
  <span class="font-bold {vimMode === 'INSERT' ? 'text-emerald-400' : vimMode === 'VISUAL' ? 'text-violet-400' : 'text-primary'}">
    {vimMode}
  </span>
  <span class="text-muted-foreground/40 truncate">{noteName}</span>
  <div class="flex-1"></div>
  <div class="flex items-center gap-1.5">
    <span class="inline-block h-[5px] w-[5px] rounded-full {wsConnected ? 'bg-emerald-400/60' : 'bg-muted-foreground/20'}"></span>
    <span class="text-muted-foreground/25">{wsConnected ? "connected" : "offline"}</span>
  </div>
</div>
