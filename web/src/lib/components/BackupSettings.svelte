<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    type BackupConfigDto,
    type BackupSummary,
    type BackupKeyStatus,
  } from "$lib/api-client";

  let backups = $state<BackupSummary[]>([]);
  let cfg = $state<BackupConfigDto | null>(null);
  let keyStatus = $state<BackupKeyStatus | null>(null);

  // UI state for "run now" form
  let destination = $state<"local" | "external" | "git">("local");
  let externalPath = $state("");
  let gitRemote = $state("");
  let gitBranch = $state("main");
  let forceEncrypt = $state(false);
  let running = $state(false);
  let runMessage = $state<string | null>(null);
  let runError = $state<string | null>(null);

  // Per-row action state
  let busyRow = $state<string | null>(null);
  let rowMessages = $state<Record<string, string>>({});

  let pickingExternal = $state(false);
  async function pickExternalFolder() {
    if (pickingExternal) return;
    pickingExternal = true;
    try {
      const res = await api.pickFolder("Pick an external backup folder");
      if (res.path) externalPath = res.path;
    } catch (e: any) {
      runError = e?.message ?? `${e}`;
    } finally {
      pickingExternal = false;
    }
  }

  async function refreshList() {
    try {
      backups = await api.listBackups();
    } catch (e) {
      runError = `Failed to list backups: ${e}`;
    }
  }
  async function refreshConfig() {
    try {
      cfg = await api.getBackupConfig();
      if (!destination || destination === "local") {
        if (cfg.external_path) {
          externalPath = cfg.external_path;
        }
        if (cfg.git_remote) {
          gitRemote = cfg.git_remote;
          gitBranch = cfg.git_branch ?? "main";
        }
      }
    } catch (e) {
      runError = `Failed to load config: ${e}`;
    }
  }
  async function refreshKey() {
    try {
      keyStatus = await api.backupKeyStatus();
    } catch {
      // ignore — keychain access can prompt; tolerate failure
    }
  }

  onMount(() => {
    void refreshList();
    void refreshConfig();
    void refreshKey();
  });

  async function runBackup() {
    if (running) return;
    running = true;
    runMessage = null;
    runError = null;
    try {
      const res = await api.runBackup({
        destination,
        external_path: destination === "external" ? externalPath || undefined : undefined,
        git_remote: destination === "git" ? gitRemote || undefined : undefined,
        git_branch: destination === "git" ? gitBranch || undefined : undefined,
        encrypt: forceEncrypt,
      });
      runMessage = `Backup complete: ${res.path} (${res.file_count} files${
        res.validated ? ", validated" : ""
      })`;
      await refreshList();
    } catch (e: any) {
      runError = e?.message ?? `${e}`;
    } finally {
      running = false;
    }
  }

  async function verifyRow(name: string) {
    busyRow = name;
    rowMessages[name] = "Verifying…";
    try {
      const v = await api.verifyBackup(name);
      rowMessages[name] = v.ok
        ? `OK (${v.elapsed_ms} ms)`
        : `FAILED: ${v.note ?? "unknown"}`;
      await refreshList();
    } catch (e: any) {
      rowMessages[name] = `Error: ${e?.message ?? e}`;
    } finally {
      busyRow = null;
    }
  }

  async function restoreRow(name: string, inPlace: boolean) {
    // In-place restore is refused while the server is running (it would rename
    // the live mosaic out from under the engine and risk re-clobbering the
    // restored files). The server returns 409; surface the guidance here
    // instead of firing a request that always fails. Use the CLI with the
    // server stopped, or restore into a sibling directory.
    if (inPlace) {
      rowMessages[name] =
        "In-place restore must be done with the server stopped: run `tesela backup restore` from the CLI, or use “Restore → sibling”.";
      return;
    }
    busyRow = name;
    rowMessages[name] = "Restoring…";
    try {
      const res = await api.restoreBackup(name, { in_place: inPlace });
      rowMessages[name] = `Restored → ${res.target}`;
      if (res.renamed_previous) {
        rowMessages[name] += ` (prior mosaic kept at ${res.renamed_previous})`;
      }
    } catch (e: any) {
      rowMessages[name] = `Error: ${e?.message ?? e}`;
    } finally {
      busyRow = null;
    }
  }

  async function pruneNow(dryRun: boolean) {
    try {
      const res = await api.pruneBackups(dryRun);
      runMessage = dryRun
        ? `Dry run: would keep ${res.kept.length}, remove ${res.removed.length}`
        : `Pruned: kept ${res.kept.length}, removed ${res.removed.length}`;
      if (!dryRun) await refreshList();
    } catch (e: any) {
      runError = e?.message ?? `${e}`;
    }
  }

  async function generateKey() {
    if (
      keyStatus?.exists &&
      !confirm(
        "An age identity already exists for this mosaic. Generating a new one will overwrite the existing entry in Keychain, and existing encrypted backups will only be decryptable while the old identity is recoverable. Continue?",
      )
    ) {
      return;
    }
    try {
      await api.backupKeygen();
      await refreshKey();
      runMessage = "Age identity stored in Keychain.";
    } catch (e: any) {
      runError = e?.message ?? `${e}`;
    }
  }

  async function saveConfig() {
    if (!cfg) return;
    try {
      cfg = await api.putBackupConfig({
        auto_on_quit: cfg.auto_on_quit,
        external_path: externalPath || null,
        git_remote: gitRemote || null,
        git_branch: gitBranch || null,
      });
      runMessage = "Settings saved.";
    } catch (e: any) {
      runError = e?.message ?? `${e}`;
    }
  }

  function fmt(iso: string): string {
    try {
      return new Date(iso).toLocaleString();
    } catch {
      return iso;
    }
  }
</script>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Backups
  </h2>

  <!-- Run now -->
  <div class="space-y-2 mb-4">
    <div class="flex items-center gap-2">
      <span class="text-[12px] text-muted-foreground/70">Destination:</span>
      {#each [
        { id: "local", label: "Local" },
        { id: "external", label: "External path" },
        { id: "git", label: "Git remote" },
      ] as opt}
        <button
          class="px-2.5 py-1 rounded-md text-[12px] transition-all border {destination === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
          onclick={() => (destination = opt.id as typeof destination)}>{opt.label}</button>
      {/each}
    </div>

    {#if destination === "external"}
      <div class="flex gap-2">
        <input
          type="text"
          placeholder="/Users/you/Library/Mobile Documents/com~apple~CloudDocs/TeselaBackups"
          bind:value={externalPath}
          class="flex-1 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
        />
        <button
          class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50"
          disabled={pickingExternal}
          onclick={pickExternalFolder}
          title="Browse for a backup destination folder using Finder"
        >
          {pickingExternal ? "…" : "Browse…"}
        </button>
      </div>
    {:else if destination === "git"}
      <div class="flex gap-2">
        <input
          type="text"
          placeholder="git@github.com:you/tesela-backups.git"
          bind:value={gitRemote}
          class="flex-1 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
        />
        <input
          type="text"
          placeholder="main"
          bind:value={gitBranch}
          class="w-24 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
        />
      </div>
    {/if}

    <label class="flex items-center gap-2 cursor-pointer text-[12px]">
      <input type="checkbox" bind:checked={forceEncrypt} class="accent-primary" />
      <span class="text-muted-foreground/80"
        >Force encryption (auto-on for external/git; requires a generated keypair)</span
      >
    </label>

    <div class="flex items-center gap-2 pt-1">
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-progress"
        disabled={running}
        onclick={runBackup}
      >
        {running ? "Running…" : "Run backup now"}
      </button>
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors"
        onclick={() => pruneNow(true)}>Prune (dry run)</button
      >
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors"
        onclick={() => pruneNow(false)}>Prune now</button
      >
    </div>

    {#if runMessage}
      <p class="text-[11px] text-emerald-400/80">{runMessage}</p>
    {/if}
    {#if runError}
      <p class="text-[11px] text-red-400/90">{runError}</p>
    {/if}
  </div>

  <!-- Existing backups -->
  {#if backups.length === 0}
    <p class="text-[11px] text-muted-foreground/40 mt-1.5">
      No backups yet. Click "Run backup now" above to create the first one.
    </p>
  {:else}
    <div class="space-y-1 text-[12px]">
      {#each backups as b}
        <div class="border border-border/40 rounded-md px-3 py-2 space-y-1">
          <div class="flex items-center gap-3">
            <span class="font-mono text-foreground/90">{b.name}</span>
            <span class="text-muted-foreground/50">{fmt(b.created_at)}</span>
            <span class="ml-auto text-[10px] text-muted-foreground/60 uppercase tracking-wider"
              >{b.destination_kind} · {b.encryption_kind} · {b.file_count} files</span
            >
            <span
              class="text-[10px] font-mono px-1.5 py-0.5 rounded {b.validated === true
                ? 'bg-emerald-500/10 text-emerald-300'
                : b.validated === false
                  ? 'bg-red-500/10 text-red-300'
                  : 'bg-muted text-muted-foreground/60'}"
              >{b.validated === true ? "OK" : b.validated === false ? "FAIL" : "—"}</span
            >
          </div>
          <div class="flex items-center gap-2">
            <button
              class="px-2 py-0.5 rounded text-[11px] border border-border/40 hover:bg-muted/40 disabled:opacity-50"
              disabled={busyRow === b.name}
              onclick={() => verifyRow(b.name)}>Verify</button
            >
            <button
              class="px-2 py-0.5 rounded text-[11px] border border-border/40 hover:bg-muted/40 disabled:opacity-50"
              disabled={busyRow === b.name}
              onclick={() => restoreRow(b.name, false)}>Restore → sibling</button
            >
            <button
              class="px-2 py-0.5 rounded text-[11px] border border-border/40 text-muted-foreground/70 hover:bg-muted/40 disabled:opacity-50"
              disabled={busyRow === b.name}
              title="In-place restore requires the server stopped — use the tesela CLI, or restore to a sibling directory."
              onclick={() => restoreRow(b.name, true)}>Restore in-place (CLI)</button
            >
            {#if rowMessages[b.name]}
              <span class="text-[10px] text-muted-foreground/70 ml-2">{rowMessages[b.name]}</span>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Encryption keypair -->
  <div class="mt-5 border-t border-border/30 pt-3">
    <p class="text-[11px] text-muted-foreground/60 mb-1.5">Encryption identity (macOS Keychain)</p>
    {#if keyStatus?.exists}
      <p class="text-[11px] text-foreground/80 font-mono break-all">{keyStatus.recipient}</p>
      <button
        class="mt-1 px-2 py-0.5 rounded text-[11px] border border-border/40 hover:bg-muted/40"
        onclick={generateKey}>Rotate keypair</button
      >
    {:else}
      <p class="text-[11px] text-muted-foreground/60 mb-1">
        No identity in Keychain. Required for encrypted (external/git) backups.
      </p>
      <button
        class="px-2 py-0.5 rounded text-[11px] border border-border/40 hover:bg-muted/40"
        onclick={generateKey}>Generate keypair</button
      >
    {/if}
  </div>

  <!-- Config form -->
  {#if cfg}
    <div class="mt-5 border-t border-border/30 pt-3 space-y-2">
      <p class="text-[11px] text-muted-foreground/60 mb-1.5">Persistent settings (config.toml)</p>
      <label class="flex items-center gap-3 cursor-pointer">
        <button
          class="w-9 h-5 rounded-full transition-colors {cfg.auto_on_quit
            ? 'bg-primary'
            : 'bg-muted'}"
          onclick={() => cfg && (cfg = { ...cfg, auto_on_quit: !cfg.auto_on_quit })}
        >
          <span
            class="block w-3.5 h-3.5 rounded-full bg-background transition-transform {cfg.auto_on_quit
              ? 'translate-x-4.5'
              : 'translate-x-0.5'}"
          ></span>
        </button>
        <span class="text-[12px]">Auto-backup on server shutdown</span>
      </label>
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors"
        onclick={saveConfig}>Save settings</button
      >
      <p class="text-[11px] text-muted-foreground/40">
        Destination + git/external paths shown above are also persisted when saved here.
      </p>
    </div>
  {/if}
</section>
