<script lang="ts">
  /*
   * Prism v4 — inline settings overlay.
   *
   * Replaces the legacy "navigate to /settings/<slug>" flow. The same
   * routes still exist as bookmark fallbacks; this overlay imports
   * each settings page as a regular Svelte component and renders it
   * inside the v4 shell, with a left nav that matches the legacy
   * Settings layout but styled in v4 tokens.
   *
   * Esc closes. Clicking a row in the left nav rewrites the active
   * slug without unmounting the overlay.
   */
  import {
    closeOverlay,
    getSettingsSlug,
    setSettingsSlug,
    type SettingsSlug,
  } from "$lib/stores/fullscreen-overlay.svelte";

  // Import the existing settings page components directly. They expose
  // no props and rely only on the shared QueryClient + WS context (both
  // mounted by the root layout), so they work outside their route.
  import GeneralPage from "../../../routes/settings/general/+page.svelte";
  import MosaicPage from "../../../routes/settings/mosaic/+page.svelte";
  import DataPage from "../../../routes/settings/data/+page.svelte";
  import SyncPage from "../../../routes/settings/sync/+page.svelte";
  import DevicesPage from "../../../routes/settings/devices/+page.svelte";
  import VoicePage from "../../../routes/settings/voice/+page.svelte";

  type Tab = { slug: SettingsSlug; label: string; hint: string };
  const TABS: Tab[] = [
    { slug: "general", label: "General", hint: "Editor, fonts, server" },
    { slug: "mosaic",  label: "Mosaic",  hint: "Create or switch mosaic" },
    { slug: "data",    label: "Data",    hint: "Backups, export, import" },
    { slug: "sync",    label: "Sync",    hint: "Notifications" },
    { slug: "devices", label: "Devices", hint: "LAN peers, device sync" },
    { slug: "voice",   label: "Voice",   hint: "Transcription models" },
  ];

  const slug = $derived(getSettingsSlug());
</script>

<div class="settings-overlay" role="dialog" aria-label="Settings">
  <header class="settings-head">
    <span class="settings-label">settings</span>
    <span class="settings-hint">esc closes</span>
    <button class="settings-close" type="button" onclick={closeOverlay} title="close · Esc">×</button>
  </header>

  <div class="settings-body">
    <nav class="settings-nav" aria-label="Settings sections">
      {#each TABS as t (t.slug)}
        <button
          type="button"
          class="settings-nav-row"
          class:active={t.slug === slug}
          onclick={() => setSettingsSlug(t.slug)}
        >
          <span class="settings-nav-label">{t.label}</span>
          <span class="settings-nav-hint">{t.hint}</span>
        </button>
      {/each}
    </nav>

    <div class="settings-content">
      {#if slug === "general"}
        <GeneralPage />
      {:else if slug === "mosaic"}
        <MosaicPage />
      {:else if slug === "data"}
        <DataPage />
      {:else if slug === "sync"}
        <SyncPage />
      {:else if slug === "devices"}
        <DevicesPage />
      {:else if slug === "voice"}
        <VoicePage />
      {/if}
    </div>
  </div>
</div>

<style>
  .settings-overlay {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: var(--v4-bg);
  }
  .settings-head {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--v4-hair);
    flex-shrink: 0;
  }
  .settings-label {
    font-family: var(--v4-mono);
    font-size: 10px;
    letter-spacing: 1.4px;
    text-transform: uppercase;
    color: var(--v4-accent);
  }
  .settings-hint {
    flex: 1;
    font-family: var(--v4-mono);
    font-size: 10.5px;
    color: var(--v4-ink5);
  }
  .settings-close {
    background: transparent;
    border: 0;
    color: var(--v4-ink4);
    font-size: 16px;
    line-height: 1;
    padding: 2px 8px;
    border-radius: 5px;
    cursor: pointer;
  }
  .settings-close:hover { color: var(--v4-ink2); background: var(--v4-surface-lo); }

  .settings-body {
    flex: 1;
    min-height: 0;
    display: flex;
  }
  .settings-nav {
    width: 200px;
    flex-shrink: 0;
    border-right: 1px solid var(--v4-hair);
    padding: 10px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow-y: auto;
  }
  .settings-nav-row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 7px 10px;
    background: transparent;
    border: 0;
    border-radius: 6px;
    cursor: pointer;
    color: var(--v4-ink4);
    text-align: left;
    transition: background var(--v4-dur-fast, 140ms), color var(--v4-dur-fast, 140ms);
  }
  .settings-nav-row:hover {
    background: var(--v4-surface-lo);
    color: var(--v4-ink2);
  }
  .settings-nav-row.active {
    background: color-mix(in srgb, var(--v4-accent) 12%, transparent);
    color: var(--v4-accent);
  }
  .settings-nav-label {
    font-family: var(--v4-sans);
    font-size: 12px;
    font-weight: 500;
  }
  .settings-nav-hint {
    font-family: var(--v4-mono);
    font-size: 9.5px;
    opacity: 0.7;
  }

  .settings-content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 28px 32px;
  }
</style>
