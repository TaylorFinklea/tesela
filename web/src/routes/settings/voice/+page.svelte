<script lang="ts">
  /*
   * Settings → Voice — desktop counterpart of the iOS
   * `TranscriptionModelsView`. Manage on-disk transcription models
   * (Whisper + Parakeet) — list the catalog, download/delete, and
   * mark one active for the voice-capture subsystem.
   */
  import { onMount } from "svelte";
  import { apiBase } from "$lib/runtime-base";

  type ModelStatus = {
    id: string;
    family: "whisper" | "parakeet";
    display_name: string;
    short_description: string;
    size_bytes: number;
    download_url: string;
    suggested_for: string[];
    on_device: boolean;
    state: "available" | "downloading" | "downloaded" | "failed";
    on_disk_bytes: number | null;
    active: boolean;
    inference_supported: boolean;
  };

  let models = $state<ModelStatus[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let busy = $state<Record<string, "download" | "delete" | "activate" | null>>({});

  async function loadModels() {
    loading = true;
    error = null;
    try {
      const r = await fetch(`${apiBase()}/transcription/models`);
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      models = await r.json();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function downloadModel(m: ModelStatus) {
    busy[m.id] = "download";
    try {
      const r = await fetch(`${apiBase()}/transcription/models/${m.id}/download`, {
        method: "POST",
      });
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      await loadModels();
    } catch (e: unknown) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy[m.id] = null;
    }
  }

  async function deleteModel(m: ModelStatus) {
    if (!confirm(`Delete ${m.display_name}? Frees ${humanSize(m.size_bytes)}.`)) {
      return;
    }
    busy[m.id] = "delete";
    try {
      const r = await fetch(`${apiBase()}/transcription/models/${m.id}`, {
        method: "DELETE",
      });
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      await loadModels();
    } finally {
      busy[m.id] = null;
    }
  }

  async function activateModel(m: ModelStatus) {
    busy[m.id] = "activate";
    try {
      const r = await fetch(`${apiBase()}/transcription/models/${m.id}/activate`, {
        method: "POST",
      });
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      await loadModels();
    } finally {
      busy[m.id] = null;
    }
  }

  function humanSize(bytes: number): string {
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(0)} MB`;
    return `${(bytes / 1024 / 1024 / 1024).toFixed(2)} GB`;
  }

  const grouped = $derived(() => {
    const out: Record<string, ModelStatus[]> = { whisper: [], parakeet: [] };
    for (const m of models) {
      if (!out[m.family]) out[m.family] = [];
      out[m.family].push(m);
    }
    return out;
  });

  const active = $derived(models.find((m) => m.active));
  const totalUsed = $derived(
    models.reduce((acc, m) => acc + (m.on_disk_bytes ?? 0), 0)
  );

  onMount(loadModels);
</script>

<div class="voice-settings">
  <header>
    <h2>Voice · Transcription models</h2>
    <p class="lead">
      On-device transcription. Download a model to unlock voice capture in the
      Daily; mark one active to make it the default. Files live under
      <span class="mono">.tesela/models/</span> in this mosaic.
    </p>
  </header>

  {#if loading}
    <div class="state">Loading catalog…</div>
  {:else if error}
    <div class="state error">{error}</div>
  {:else}
    <section class="summary">
      <div>
        <div class="key">Active model</div>
        <div class="val">
          {active ? active.display_name : "—"}
        </div>
      </div>
      <div>
        <div class="key">Storage used</div>
        <div class="val mono">{humanSize(totalUsed)}</div>
      </div>
      <div>
        <div class="key">Models downloaded</div>
        <div class="val mono">
          {models.filter((m) => m.state === "downloaded").length} / {models.length}
        </div>
      </div>
    </section>

    {#each Object.entries(grouped()) as [family, list]}
      {#if list.length > 0}
        <section class="family">
          <h3>{family === "whisper" ? "Whisper" : "Parakeet"}</h3>
          <ul class="models">
            {#each list as m (m.id)}
              <li class:active={m.active}>
                <div class="head">
                  <div class="title">
                    {m.display_name}
                    {#if m.active}<span class="badge">active</span>{/if}
                  </div>
                  <div class="size mono">{humanSize(m.size_bytes)}</div>
                </div>
                <div class="desc">{m.short_description}</div>
                <div class="chips">
                  {#each m.suggested_for as tag}
                    <span class="chip">{tag}</span>
                  {/each}
                </div>
                <div class="actions">
                  {#if m.state === "downloaded"}
                    {#if !m.active}
                      {#if m.inference_supported}
                        <button
                          class="btn primary"
                          onclick={() => activateModel(m)}
                          disabled={busy[m.id] !== null && busy[m.id] !== undefined}
                        >
                          {busy[m.id] === "activate" ? "Activating…" : "Set active"}
                        </button>
                      {:else}
                        <span class="mono unsupported">inference not yet supported</span>
                      {/if}
                    {/if}
                    <button
                      class="btn danger"
                      onclick={() => deleteModel(m)}
                      disabled={busy[m.id] !== null && busy[m.id] !== undefined}
                    >
                      {busy[m.id] === "delete" ? "Deleting…" : "Delete"}
                    </button>
                    {#if m.on_disk_bytes}
                      <span class="mono on-disk">on disk: {humanSize(m.on_disk_bytes)}</span>
                    {/if}
                  {:else}
                    <button
                      class="btn primary"
                      onclick={() => downloadModel(m)}
                      disabled={busy[m.id] !== null && busy[m.id] !== undefined}
                    >
                      {busy[m.id] === "download" ? "Downloading…" : "Download"}
                    </button>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    {/each}
  {/if}
</div>

<style>
  .voice-settings {
    padding: 18px 22px;
    color: var(--fg-default);
    max-width: 760px;
  }
  header {
    margin-bottom: 18px;
  }
  h2 {
    margin: 0 0 6px;
    font-size: 18px;
    font-weight: 600;
  }
  .lead {
    color: var(--fg-muted);
    font-size: 13px;
    margin: 0;
    line-height: 1.5;
  }
  .mono {
    font-family: var(--font-mono, ui-monospace, "JetBrains Mono", monospace);
    font-size: 12px;
  }
  .state {
    padding: 14px;
    color: var(--fg-faint);
    font-style: italic;
  }
  .state.error {
    color: var(--type-task);
  }
  .summary {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
    margin-bottom: 18px;
    padding: 12px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 8px;
  }
  .summary .key {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--fg-faint);
    margin-bottom: 2px;
  }
  .summary .val {
    color: var(--fg-default);
    font-weight: 500;
  }
  .family {
    margin-bottom: 24px;
  }
  h3 {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--fg-faint);
    margin: 0 0 8px;
    font-weight: 500;
  }
  .models {
    list-style: none;
    padding: 0;
    margin: 0;
    display: grid;
    gap: 10px;
  }
  .models li {
    background: var(--bg-2);
    border: 1px solid var(--line);
    border-radius: 8px;
    padding: 12px 14px;
  }
  .models li.active {
    border-color: var(--accent-primary);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 4px;
  }
  .title {
    font-weight: 600;
    color: var(--fg-default);
    flex: 1;
  }
  .badge {
    display: inline-block;
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    padding: 1px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent-primary) 16%, transparent);
    color: var(--accent-primary);
    margin-left: 6px;
  }
  .size {
    color: var(--fg-faint);
  }
  .desc {
    color: var(--fg-muted);
    font-size: 12.5px;
    margin: 4px 0;
  }
  .chips {
    display: flex;
    gap: 4px;
    margin: 4px 0 8px;
  }
  .chip {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 10px;
    padding: 1px 6px;
    background: var(--bg-3);
    border-radius: 999px;
    color: var(--fg-muted);
  }
  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .btn {
    background: transparent;
    border: 1px solid var(--line);
    color: var(--fg-default);
    padding: 5px 12px;
    border-radius: 6px;
    font-family: inherit;
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
  }
  .btn:hover:not(:disabled) {
    border-color: var(--accent-primary);
  }
  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  .btn.primary {
    background: var(--accent-primary);
    color: var(--bg);
    border-color: var(--accent-primary);
  }
  .btn.danger {
    color: var(--type-task);
    border-color: color-mix(in srgb, var(--type-task) 40%, transparent);
  }
  .on-disk {
    color: var(--fg-faint);
    margin-left: 8px;
  }
  .unsupported {
    color: var(--type-note);
    font-size: 11px;
  }
</style>
