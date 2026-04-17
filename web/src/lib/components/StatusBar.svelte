<script lang="ts">
  import { page } from "$app/state";
  import { getConnected } from "$lib/ws-client.svelte";
  import { getSaveStatus } from "$lib/stores/save-state.svelte";
  import { isSplitOpen, getActivePane, isCtrlWPending } from "$lib/stores/pane-state.svelte";

  let { vimMode = "NORMAL" }: { vimMode?: string } = $props();

  const wsConnected = $derived(getConnected());
  const saveStatus = $derived(getSaveStatus());
  const currentPath = $derived(page.url.pathname);
  const noteName = $derived(
    currentPath.startsWith("/p/")
      ? decodeURIComponent(currentPath.slice(3))
      : currentPath === "/"
        ? "Home"
        : currentPath.slice(1),
  );
  const splitOpen = $derived(isSplitOpen());
  const activePane = $derived(getActivePane());
  const ctrlWPending = $derived(isCtrlWPending());
</script>

<div class="h-7 bg-surface border-t border-border flex items-center px-4 gap-4 text-[11px] font-mono shrink-0 select-none">
  <span class="font-bold {vimMode === 'INSERT' ? 'text-emerald-400' : vimMode === 'VISUAL' ? 'text-violet-400' : 'text-primary'}">
    {vimMode}
  </span>
  {#if ctrlWPending}
    <span class="text-amber-400 font-bold animate-pulse">^W</span>
  {/if}
  {#if splitOpen}
    <span class="text-muted-foreground/50 font-bold">
      {activePane === "outliner" ? "⬆ OUTLINER" : "⬇ KANBAN"}
    </span>
  {/if}
  <span class="text-muted-foreground/60 truncate">{noteName}</span>

  <!-- Save indicator -->
  {#if saveStatus === "saving"}
    <span class="text-muted-foreground/50">saving…</span>
  {:else if saveStatus === "saved"}
    <span class="text-emerald-400/60">saved</span>
  {:else if saveStatus === "error"}
    <span class="text-destructive/80">save failed</span>
  {/if}

  <div class="flex-1"></div>
  <div class="flex items-center gap-1.5">
    <span class="inline-block h-[5px] w-[5px] rounded-full {wsConnected ? 'bg-emerald-400/60' : 'bg-destructive/60'}"></span>
    <span class="text-muted-foreground/40">{wsConnected ? "connected" : "offline"}</span>
  </div>
</div>
