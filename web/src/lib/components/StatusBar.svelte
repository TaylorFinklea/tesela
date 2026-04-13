<script lang="ts">
  import { page } from "$app/state";
  import { getConnected } from "$lib/ws-client.svelte";
  import { getSaveStatus } from "$lib/stores/save-state.svelte";

  let { vimMode = "NORMAL" }: { vimMode?: string } = $props();

  const wsConnected = $derived(getConnected());
  const saveStatus = $derived(getSaveStatus());
  const currentPath = $derived(page.url.pathname);
  const noteName = $derived(
    currentPath.startsWith("/p/") ? decodeURIComponent(currentPath.slice(3))
      : currentPath === "/" ? "Home" : currentPath.slice(1),
  );
</script>

<div class="h-7 bg-surface border-t border-border flex items-center px-5 gap-5 text-[11px] font-mono shrink-0 select-none">
  <span class="font-bold {vimMode === 'INSERT' ? 'text-emerald-600 dark:text-emerald-400' : vimMode === 'VISUAL' ? 'text-violet-600 dark:text-violet-400' : 'text-primary'}">
    {vimMode}
  </span>
  <span class="text-muted-foreground truncate">{noteName}</span>

  {#if saveStatus === "saving"}
    <span class="text-muted-foreground">saving…</span>
  {:else if saveStatus === "saved"}
    <span class="text-emerald-600 dark:text-emerald-400">saved ✓</span>
  {:else if saveStatus === "error"}
    <span class="text-destructive">save failed</span>
  {/if}

  <div class="flex-1"></div>
  <div class="flex items-center gap-2">
    <span class="inline-block h-[6px] w-[6px] rounded-full {wsConnected ? 'bg-emerald-500' : 'bg-destructive'}"></span>
    <span class="text-muted-foreground">{wsConnected ? "connected" : "offline"}</span>
  </div>
</div>
