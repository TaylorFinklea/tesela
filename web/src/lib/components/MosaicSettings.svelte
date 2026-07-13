<script lang="ts">
  import { onMount } from "svelte";
  import {
    api,
    type CurrentMosaicResponse,
    type CreateMosaicResponse,
    type DiscoveredMosaic,
    type LogseqPlan,
    type LogseqApplyOutcome,
  } from "$lib/api-client";
  import { blockMoveRecovery } from "$lib/block-move-recovery.svelte";
  import LogseqPlanPreview from "$lib/components/LogseqPlanPreview.svelte";

  let current = $state<CurrentMosaicResponse | null>(null);
  let discovered = $state<DiscoveredMosaic[]>([]);

  // Create form
  type LocationMode = "name" | "custom";
  let locationMode = $state<LocationMode>("name");
  let mode = $state<"blank" | "import">("blank");
  let newName = $state("");
  let newCustomPath = $state("");
  let importKind = $state<"obsidian" | "logseq" | "org">("logseq");
  let sourcePath = $state("");

  let creating = $state(false);
  let pickingCustom = $state(false);
  let pickingSource = $state(false);
  let result = $state<CreateMosaicResponse | null>(null);
  let error = $state<string | null>(null);

  // Logseq plan state (when create+import with Logseq lands).
  let logseqPlan = $state<LogseqPlan | null>(null);
  let logseqPlanTarget = $state<string | null>(null);
  let logseqApplyOutcome = $state<LogseqApplyOutcome | null>(null);

  // Per-row state for the Discovered list
  let switchingPath = $state<string | null>(null);
  let rowMessage = $state<Record<string, string>>({});

  // "Switch to existing mosaic" picker — for mosaics outside the
  // standard root (dev directories, etc).
  let pickingExisting = $state(false);
  async function pickAndSwitchExisting() {
    if (pickingExisting) return;
    pickingExisting = true;
    try {
      const res = await api.pickFolder("Pick a folder containing a `.tesela/` mosaic");
      if (res.path) {
        await switchAndRestart(res.path);
      }
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      pickingExisting = false;
    }
  }

  async function refresh() {
    try {
      current = await api.currentMosaic();
    } catch {/* tolerate */}
    try {
      discovered = await api.discoveredMosaics();
    } catch {/* tolerate */}
  }
  onMount(refresh);

  // Derived: preview path for Name mode.
  const resolvedNamePath = $derived.by(() => {
    if (!current?.suggested_root) return "";
    const trimmed = newName.trim();
    if (!trimmed) return "";
    return `${current.suggested_root}/${trimmed}`;
  });

  async function pickCustomPath() {
    if (pickingCustom) return;
    pickingCustom = true;
    try {
      const res = await api.pickFolder("Pick (or create) a folder for the new mosaic");
      if (res.path) newCustomPath = res.path;
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      pickingCustom = false;
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
    if (locationMode === "name" && !newName.trim()) {
      error = "Give the mosaic a name.";
      return;
    }
    if (locationMode === "custom" && !newCustomPath.trim()) {
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
    logseqPlan = null;
    logseqPlanTarget = null;
    logseqApplyOutcome = null;

    try {
      // Logseq import: split into create-blank + plan, so the user
      // gets the same conflict-resolution preview as Settings → Data.
      if (mode === "import" && importKind === "logseq") {
        // 1. Create the mosaic without an inline import.
        result = await api.createMosaic({
          name: locationMode === "name" ? newName.trim() : undefined,
          path: locationMode === "custom" ? newCustomPath : undefined,
          // intentionally no `import` here
        });
        await refresh();
        // 2. Plan the import against the freshly-created mosaic.
        logseqPlanTarget = result.path;
        logseqPlan = await api.planLogseq(sourcePath, result.path);
      } else {
        // Blank or non-Logseq import: existing path.
        result = await api.createMosaic({
          name: locationMode === "name" ? newName.trim() : undefined,
          path: locationMode === "custom" ? newCustomPath : undefined,
          import:
            mode === "import"
              ? { kind: importKind, source: sourcePath }
              : undefined,
        });
        await refresh();
      }
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      creating = false;
    }
  }

  function onPlanApplied(outcome: LogseqApplyOutcome) {
    logseqApplyOutcome = outcome;
    logseqPlan = null;
  }
  function onPlanCancel() {
    logseqPlan = null;
    logseqPlanTarget = null;
  }

  // tesela-ejn.2: the embedded desktop server always 409s
  // `/server/restart` (it can't re-exec the Tauri binary) — the switch
  // flow is a dead end there. Interim = hide/disable the controls with
  // explanatory copy; tesela-mmos (one server, N mosaics, lazy-loaded)
  // replaces the whole restart-to-switch flow, so don't build a relaunch
  // flow that would just be thrown away.
  const embedded = $derived(current?.embedded === true);

  async function switchAndRestart(path: string) {
    if (switchingPath || embedded) return;
    if (blockMoveRecovery.current()) {
      const message = "Resolve the submitted block move before switching mosaics";
      error = message;
      rowMessage[path] = message;
      return;
    }
    if (
      !confirm(
        `Switch to ${path}? The server will shut down (auto-backup runs), then a new instance will start in ~2 seconds. The page will lose its WebSocket connection during the swap.`,
      )
    ) {
      return;
    }
    switchingPath = path;
    rowMessage[path] = "Switching…";
    error = null;
    try {
      await api.switchMosaic(path);
      await api.restartServer();
      setTimeout(() => location.reload(), 4000);
    } catch (e: any) {
      rowMessage[path] = `Error: ${e?.message ?? e}`;
      switchingPath = null;
    }
  }

  function fmtRelative(iso: string | null): string {
    if (!iso) return "";
    const then = new Date(iso).getTime();
    const sec = Math.max(0, Math.round((Date.now() - then) / 1000));
    if (sec < 60) return `${sec}s ago`;
    const min = Math.round(sec / 60);
    if (min < 60) return `${min}m ago`;
    const hr = Math.round(min / 60);
    if (hr < 24) return `${hr}h ago`;
    const d = Math.round(hr / 24);
    return `${d}d ago`;
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

  {#if embedded}
    <div class="text-[11px] text-amber-400/80 mb-3 leading-relaxed border border-amber-400/20 rounded-md px-3 py-2 bg-amber-400/5">
      Switching mosaics isn't available in the desktop app yet — the embedded
      server can't restart itself. Quit and reopen Tesela with a different
      mosaic instead.
    </div>
  {/if}

  <!-- Discovered mosaics -->
  <div class="mb-3">
    <button
      class="px-2.5 py-1 rounded-md text-[11px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50"
      disabled={embedded || pickingExisting || !!switchingPath}
      onclick={pickAndSwitchExisting}
      title={embedded
        ? "Not available in the desktop app — quit and reopen with a different mosaic"
        : "Pick any folder with a `.tesela/` mosaic to switch to it"}
    >
      {pickingExisting || switchingPath ? "…" : "Switch to existing mosaic…"}
    </button>
  </div>

  {#if discovered.length > 0}
    <div class="space-y-1 mb-5">
      <p class="text-[11px] text-muted-foreground/60 mb-1.5">
        Discovered ({discovered.length})
      </p>
      {#each discovered as m}
        <div class="border border-border/40 rounded-md px-3 py-2 space-y-1
                    {m.is_current ? 'bg-primary/5 border-primary/30' : ''}">
          <div class="flex items-center gap-3">
            <span class="font-mono text-foreground/90 text-[12px]">{m.name}</span>
            {#if m.is_current}
              <span class="text-[10px] uppercase tracking-wider text-primary">current</span>
            {/if}
            <span class="ml-auto text-[10px] text-muted-foreground/60">
              {m.note_count} note{m.note_count === 1 ? "" : "s"}
              {#if m.last_modified}
                · {fmtRelative(m.last_modified)}
              {/if}
            </span>
          </div>
          <div class="text-[10px] text-muted-foreground/50 font-mono break-all">{m.path}</div>
          {#if !m.is_current}
            <div class="flex items-center gap-2 pt-1">
              <button
                class="px-2 py-0.5 rounded text-[11px] border border-border/40 hover:bg-muted/40 disabled:opacity-50"
                disabled={embedded || switchingPath === m.path}
                title={embedded ? "Not available in the desktop app — quit and reopen with a different mosaic" : undefined}
                onclick={() => switchAndRestart(m.path)}>Switch</button>
              {#if rowMessage[m.path]}
                <span class="text-[10px] text-muted-foreground/70">{rowMessage[m.path]}</span>
              {/if}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  <!-- Create new -->
  <div class="space-y-2 border-t border-border/30 pt-4">
    <p class="text-[11px] text-muted-foreground/60 mb-1.5">Create a new mosaic</p>

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

    <div class="flex items-center gap-2 pt-1">
      <span class="text-[12px] text-muted-foreground/70">Location:</span>
      {#each [
        { id: "name", label: "Just a name" },
        { id: "custom", label: "Custom folder" },
      ] as opt}
        <button
          class="px-2.5 py-1 rounded-md text-[12px] transition-all border {locationMode === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
          onclick={() => (locationMode = opt.id as LocationMode)}>{opt.label}</button>
      {/each}
    </div>

    {#if locationMode === "name"}
      <input
        type="text"
        placeholder="my-logseq-import"
        bind:value={newName}
        class="w-full text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
      />
      <p class="text-[11px] text-muted-foreground/50">
        {#if resolvedNamePath}
          Will create at <span class="font-mono">{resolvedNamePath}</span>
        {:else if current?.suggested_root}
          Will land under <span class="font-mono">{current.suggested_root}/&lt;name&gt;</span>
        {/if}
      </p>
    {:else}
      <div class="flex gap-2">
        <input
          type="text"
          placeholder={current?.suggested_root
            ? `${current.suggested_root}/my-new-mosaic`
            : "/path/to/new/mosaic"}
          bind:value={newCustomPath}
          class="flex-1 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
        />
        <button
          class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50"
          disabled={pickingCustom}
          onclick={pickCustomPath}
          title="Browse for the folder that will hold the new mosaic"
        >
          {pickingCustom ? "…" : "Browse…"}
        </button>
      </div>
    {/if}

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

    {#if result && !logseqPlan && !logseqApplyOutcome}
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
        {#if embedded}
          <p class="text-[10px] text-amber-400/80">
            Created, but switching to it isn't available in the desktop app —
            quit and reopen Tesela with this mosaic instead.
          </p>
        {:else}
          <button
            class="px-2.5 py-1 rounded-md text-[12px] border border-primary/30 bg-primary/10 text-primary hover:bg-primary/20 transition-colors disabled:opacity-50"
            disabled={!!switchingPath}
            onclick={() => switchAndRestart(result!.path)}
          >
            {switchingPath ? "Switching…" : "Switch to this mosaic"}
          </button>
          <p class="text-[10px] text-muted-foreground/50">
            Server will gracefully shut down (auto-backup runs), then a fresh instance binds in ~2s. Page reloads automatically.
          </p>
        {/if}
      </div>
    {/if}

    <!-- Logseq plan preview (when create+import landed) -->
    {#if logseqPlan && logseqPlanTarget && !logseqApplyOutcome}
      <div class="mt-2 border border-border/40 rounded-md px-3 py-2 space-y-2">
        <div class="text-[11px] text-foreground/80">
          Created blank mosaic at <span class="font-mono">{logseqPlanTarget}</span>.
          Review the Logseq import below before applying.
        </div>
        <LogseqPlanPreview
          plan={logseqPlan}
          targetMosaic={logseqPlanTarget}
          onapplied={onPlanApplied}
          oncancel={onPlanCancel}
        />
      </div>
    {/if}

    <!-- After plan applied -->
    {#if logseqApplyOutcome && logseqPlanTarget}
      <div class="mt-2 border border-emerald-400/30 bg-emerald-400/5 rounded-md px-3 py-2 space-y-1.5 text-[12px]">
        <div class="text-emerald-300/90 mb-1">Import applied to {logseqPlanTarget}</div>
        <div class="grid grid-cols-3 gap-2 text-[11px] text-foreground/80">
          <div>Imported: <span class="font-mono">{logseqApplyOutcome.imported}</span></div>
          <div>Overwritten: <span class="font-mono">{logseqApplyOutcome.overwritten}</span></div>
          <div>Renamed: <span class="font-mono">{logseqApplyOutcome.renamed}</span></div>
          <div>Skipped: <span class="font-mono">{logseqApplyOutcome.skipped}</span></div>
          <div>Unchanged: <span class="font-mono">{logseqApplyOutcome.unchanged}</span></div>
          <div>Assets: <span class="font-mono">{logseqApplyOutcome.assets_copied}</span></div>
        </div>
        {#if logseqApplyOutcome.errors.length > 0}
          <details class="text-[11px] text-red-300/80 mt-1">
            <summary class="cursor-pointer">{logseqApplyOutcome.errors.length} error{logseqApplyOutcome.errors.length === 1 ? "" : "s"}</summary>
            <ul class="ml-3 mt-1 list-disc">
              {#each logseqApplyOutcome.errors as err}<li class="font-mono">{err}</li>{/each}
            </ul>
          </details>
        {/if}
        {#if embedded}
          <p class="text-[10px] text-amber-400/80">
            Applied, but switching to it isn't available in the desktop app —
            quit and reopen Tesela with this mosaic instead.
          </p>
        {:else}
          <button
            class="px-2.5 py-1 rounded-md text-[12px] border border-primary/30 bg-primary/10 text-primary hover:bg-primary/20 transition-colors disabled:opacity-50"
            disabled={!!switchingPath}
            onclick={() => switchAndRestart(logseqPlanTarget!)}
          >
            {switchingPath ? "Switching…" : "Switch to this mosaic"}
          </button>
        {/if}
      </div>
    {/if}
    {#if error}
      <p class="text-[11px] text-red-400/90">{error}</p>
    {/if}
  </div>
</section>
