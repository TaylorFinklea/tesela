<script lang="ts">
  import { onMount } from "svelte";
  import { api, type CurrentMosaicResponse, type CreateMosaicResponse } from "$lib/api-client";

  let current = $state<CurrentMosaicResponse | null>(null);
  let mode = $state<"blank" | "import">("blank");
  let importKind = $state<"obsidian" | "logseq" | "org">("logseq");
  let newPath = $state("");
  let sourcePath = $state("");
  let creating = $state(false);
  let pickingNew = $state(false);
  let pickingSource = $state(false);
  let result = $state<CreateMosaicResponse | null>(null);
  let error = $state<string | null>(null);
  let switching = $state(false);

  async function refresh() {
    try {
      current = await api.currentMosaic();
    } catch (e) {
      // server may be restarting; tolerate.
    }
  }
  onMount(refresh);

  async function pickNewPath() {
    if (pickingNew) return;
    pickingNew = true;
    try {
      const res = await api.pickFolder(
        "Pick (or create) a folder for the new mosaic",
      );
      if (res.path) newPath = res.path;
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      pickingNew = false;
    }
  }
  async function pickSourcePath() {
    if (pickingSource) return;
    pickingSource = true;
    try {
      const label =
        importKind === "obsidian"
          ? "Pick an Obsidian vault to import"
          : importKind === "logseq"
            ? "Pick a Logseq graph to import"
            : "Pick a folder of .org files to import";
      const res = await api.pickFolder(label);
      if (res.path) sourcePath = res.path;
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      pickingSource = false;
    }
  }

  async function create() {
    if (creating) return;
    if (!newPath.trim()) {
      error = "Pick a target folder for the new mosaic.";
      return;
    }
    if (mode === "import" && !sourcePath.trim()) {
      error = "Pick the source folder to import from.";
      return;
    }
    creating = true;
    error = null;
    result = null;
    try {
      result = await api.createMosaic({
        path: newPath,
        import:
          mode === "import"
            ? { kind: importKind, source: sourcePath }
            : undefined,
      });
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      creating = false;
    }
  }

  async function switchAndRestart() {
    if (!result || switching) return;
    if (
      !confirm(
        "Switch to this mosaic? The server will shut down (auto-backup runs), then a new instance will start on the new mosaic in ~2 seconds. The page will lose its WebSocket connection during the swap.",
      )
    ) {
      return;
    }
    switching = true;
    error = null;
    try {
      await api.switchMosaic(result.path);
      const r = await api.restartServer();
      if (r.respawn_used) {
        // Reload after ~4s to give the new server time to bind + index.
        setTimeout(() => location.reload(), 4000);
      } else {
        // launchd managed — same wait.
        setTimeout(() => location.reload(), 4000);
      }
    } catch (e: any) {
      error = e?.message ?? `${e}`;
      switching = false;
    }
  }
</script>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Mosaic
  </h2>

  {#if current}
    <div class="text-[11px] text-muted-foreground/70 mb-3 leading-relaxed">
      Current: <span class="text-foreground/90 font-mono">{current.path}</span>
      {#if current.config_default_mosaic && current.config_default_mosaic !== current.path}
        <span class="text-amber-400/80"
          >· config says default = {current.config_default_mosaic}; restart to apply</span
        >
      {/if}
    </div>
  {/if}

  <div class="space-y-2">
    <div class="flex items-center gap-2">
      <span class="text-[12px] text-muted-foreground/70">Create:</span>
      {#each [
        { id: "blank", label: "Blank mosaic" },
        { id: "import", label: "From another tool" },
      ] as opt}
        <button
          class="px-2.5 py-1 rounded-md text-[12px] transition-all border {mode === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
          onclick={() => (mode = opt.id as typeof mode)}>{opt.label}</button>
      {/each}
    </div>

    <div class="flex gap-2">
      <input
        type="text"
        placeholder={current?.suggested_root
          ? `${current.suggested_root}/my-new-mosaic`
          : "/path/to/new/mosaic"}
        bind:value={newPath}
        class="flex-1 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
      />
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50"
        disabled={pickingNew}
        onclick={pickNewPath}
        title="Browse for the folder that will hold the new mosaic"
      >
        {pickingNew ? "…" : "Browse…"}
      </button>
    </div>

    {#if mode === "import"}
      <div class="flex items-center gap-2 pt-1">
        <span class="text-[12px] text-muted-foreground/70">From:</span>
        {#each [
          { id: "obsidian", label: "Obsidian" },
          { id: "logseq", label: "Logseq" },
          { id: "org", label: "Org files" },
        ] as opt}
          <button
            class="px-2.5 py-1 rounded-md text-[12px] transition-all border {importKind === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
            onclick={() => (importKind = opt.id as typeof importKind)}>{opt.label}</button>
        {/each}
      </div>
      <div class="flex gap-2">
        <input
          type="text"
          placeholder="Source folder to import"
          bind:value={sourcePath}
          class="flex-1 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
        />
        <button
          class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50"
          disabled={pickingSource}
          onclick={pickSourcePath}
          title="Browse for the source folder to import"
        >
          {pickingSource ? "…" : "Browse…"}
        </button>
      </div>
    {/if}

    <div class="pt-1">
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-progress"
        disabled={creating}
        onclick={create}
      >
        {creating ? "Creating…" : mode === "blank" ? "Create blank mosaic" : "Create + import"}
      </button>
    </div>

    {#if result}
      <div class="mt-2 border border-border/40 rounded-md px-3 py-2 space-y-1.5">
        <div class="text-[11px] text-foreground/80">
          Created at <span class="font-mono">{result.path}</span>
        </div>
        {#if result.import_success === true}
          <details class="text-[11px] text-muted-foreground/70">
            <summary class="cursor-pointer">Import output (success)</summary>
            <pre class="font-mono whitespace-pre-wrap mt-1 text-foreground/70">{result.import_stdout ?? ""}</pre>
          </details>
        {:else if result.import_success === false}
          <div class="text-[11px] text-red-300/90">Import returned errors:</div>
          <pre class="text-[11px] font-mono whitespace-pre-wrap text-red-300/80">{result.import_stderr ?? result.import_stdout ?? ""}</pre>
        {/if}
        <button
          class="px-2.5 py-1 rounded-md text-[12px] border border-primary/30 bg-primary/10 text-primary hover:bg-primary/20 transition-colors disabled:opacity-50"
          disabled={switching}
          onclick={switchAndRestart}
        >
          {switching ? "Switching…" : "Switch to this mosaic"}
        </button>
        <p class="text-[10px] text-muted-foreground/50">
          Server will gracefully shut down (auto-backup runs), then a fresh instance binds in ~2s. Page reloads automatically.
        </p>
      </div>
    {/if}
    {#if error}
      <p class="text-[11px] text-red-400/90">{error}</p>
    {/if}
  </div>
</section>
