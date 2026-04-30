<script lang="ts">
  import { page } from "$app/state";
  import { getConnected } from "$lib/ws-client.svelte";
  import { getSaveStatus } from "$lib/stores/save-state.svelte";
  import {
    isSplitOpen,
    getActivePane,
    isCtrlWPending,
    getVimMode,
    isVimEnabled,
    isBottomDrawerOpen,
    toggleBottomDrawer,
  } from "$lib/stores/pane-state.svelte";

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
  const vimMode = $derived(getVimMode());
  const vimOn = $derived(isVimEnabled());
  const drawerOpen = $derived(isBottomDrawerOpen());
</script>

<div class="v9-status">
  {#if vimOn}
    <span class="mode">{vimMode}</span>
  {/if}
  {#if ctrlWPending}
    <span style="color: var(--v9-amber); font-weight: 700;">^W</span>
  {/if}
  {#if splitOpen}
    <span style="color: var(--v9-ink-faint); font-weight: 700;">
      {activePane === "outliner" ? "⬆ OUTLINER" : "⬇ KANBAN"}
    </span>
  {/if}
  <span style="color: var(--v9-ink-3); overflow: hidden; text-overflow: ellipsis; white-space: nowrap;">{noteName}</span>

  {#if saveStatus === "saving"}
    <span style="color: var(--v9-ink-faint);">saving…</span>
  {:else if saveStatus === "saved"}
    <span style="color: var(--v9-sage);">saved</span>
  {:else if saveStatus === "error"}
    <span style="color: var(--v9-rose);">save failed</span>
  {/if}

  <div class="keys">
    <span class="toggle {drawerOpen ? 'on' : ''}" role="button" tabindex="0"
      onclick={() => toggleBottomDrawer()}
      onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); toggleBottomDrawer(); } }}
    >[{drawerOpen ? "×" : " "}] bottom</span>
    <span style="display: inline-flex; align-items: center; gap: 4px;">
      <span style="display: inline-block; height: 5px; width: 5px; border-radius: 50%; background: {wsConnected ? 'var(--v9-sage)' : 'var(--v9-rose)'};"></span>
      <span>{wsConnected ? "connected" : "offline"}</span>
    </span>
  </div>
</div>
