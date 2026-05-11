<script lang="ts">
  import {
    api,
    type ImportResponse,
    type LogseqPlan,
    type LogseqApplyOutcome,
  } from "$lib/api-client";
  import LogseqPlanPreview from "$lib/components/LogseqPlanPreview.svelte";

  type Kind = "obsidian" | "logseq" | "org";

  let kind = $state<Kind>("obsidian");
  let source = $state("");
  let dryRun = $state(true);
  let running = $state(false);
  let picking = $state(false);
  // Obsidian/Org result (shell-out path)
  let result = $state<ImportResponse | null>(null);
  let error = $state<string | null>(null);

  // Logseq plan flow
  let plan = $state<LogseqPlan | null>(null);
  let applyOutcome = $state<LogseqApplyOutcome | null>(null);

  async function pickFolder() {
    if (picking) return;
    picking = true;
    error = null;
    try {
      const label =
        kind === "obsidian"
          ? "Pick an Obsidian vault to import"
          : kind === "logseq"
            ? "Pick a Logseq graph to import"
            : "Pick a folder of .org files to import";
      const res = await api.pickFolder(label);
      if (res.path) source = res.path;
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      picking = false;
    }
  }

  async function runImport() {
    if (!source.trim()) {
      error = "Source path required.";
      return;
    }
    error = null;
    result = null;
    plan = null;
    applyOutcome = null;

    if (kind === "logseq") {
      running = true;
      try {
        plan = await api.planLogseq(source);
      } catch (e: any) {
        error = e?.message ?? `${e}`;
      } finally {
        running = false;
      }
      return;
    }

    // Obsidian + Org still use the shell-out path for now.
    running = true;
    try {
      const fn = kind === "obsidian" ? api.importObsidian : api.importOrg;
      result = await fn(source, dryRun);
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      running = false;
    }
  }

  function onApplied(outcome: LogseqApplyOutcome) {
    applyOutcome = outcome;
    plan = null;
  }
  function onCancel() {
    plan = null;
    applyOutcome = null;
  }
</script>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Import from another tool
  </h2>

  {#if !plan && !applyOutcome}
    <div class="space-y-2">
      <div class="flex items-center gap-2">
        <span class="text-[12px] text-muted-foreground/70">Source:</span>
        {#each [
          { id: "obsidian", label: "Obsidian vault" },
          { id: "logseq", label: "Logseq graph" },
          { id: "org", label: "Org files" },
        ] as opt}
          <button
            class="px-2.5 py-1 rounded-md text-[12px] transition-all border {kind === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
            onclick={() => (kind = opt.id as Kind)}>{opt.label}</button>
        {/each}
      </div>
      <div class="flex gap-2">
        <input
          type="text"
          placeholder={kind === "obsidian"
            ? "/Users/you/Documents/MyVault"
            : kind === "logseq"
              ? "/Users/you/Documents/Logseq-Graph"
              : "/Users/you/org-roam"}
          bind:value={source}
          class="flex-1 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
        />
        <button
          class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50"
          disabled={picking}
          onclick={pickFolder}>{picking ? "…" : "Browse…"}</button>
      </div>

      {#if kind !== "logseq"}
        <label class="flex items-center gap-2 cursor-pointer text-[12px]">
          <input type="checkbox" bind:checked={dryRun} class="accent-primary" />
          <span class="text-muted-foreground/80">Dry run (don't write files)</span>
        </label>
      {/if}

      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-progress"
        disabled={running}
        onclick={runImport}
      >
        {running ? "Planning…" : kind === "logseq" ? "Plan import" : dryRun ? "Dry run" : "Run import"}
      </button>

      {#if result}
        <div class="mt-2 border border-border/40 rounded-md px-3 py-2">
          <div class="text-[11px] text-muted-foreground/70 mb-1">
            {result.success ? "Success" : "Errors during import"} — kind: {result.kind}
          </div>
          {#if result.stdout}
            <pre class="text-[11px] font-mono whitespace-pre-wrap text-foreground/80">{result.stdout}</pre>
          {/if}
          {#if result.stderr}
            <pre class="text-[11px] font-mono whitespace-pre-wrap text-red-300/80 mt-1">{result.stderr}</pre>
          {/if}
        </div>
      {/if}
      {#if error}
        <p class="text-[11px] text-red-400/90">{error}</p>
      {/if}
    </div>
    <p class="text-[11px] text-muted-foreground/40 mt-2">
      All importers are idempotent (re-running on unchanged sources is a no-op).
      Logseq's plan view lets you resolve conflicts per-item before applying.
    </p>
  {/if}

  {#if plan && !applyOutcome}
    <LogseqPlanPreview {plan} onapplied={onApplied} oncancel={onCancel} />
  {/if}

  {#if applyOutcome}
    <div class="space-y-2">
      <div class="border border-emerald-400/30 bg-emerald-400/5 rounded-md px-3 py-2 space-y-1 text-[12px]">
        <div class="text-emerald-300/90 mb-1">Import complete</div>
        <div class="grid grid-cols-3 gap-2 text-[11px] text-foreground/80">
          <div>Imported: <span class="font-mono">{applyOutcome.imported}</span></div>
          <div>Overwritten: <span class="font-mono">{applyOutcome.overwritten}</span></div>
          <div>Renamed: <span class="font-mono">{applyOutcome.renamed}</span></div>
          <div>Skipped: <span class="font-mono">{applyOutcome.skipped}</span></div>
          <div>Unchanged: <span class="font-mono">{applyOutcome.unchanged}</span></div>
          <div>Assets: <span class="font-mono">{applyOutcome.assets_copied}</span></div>
        </div>
        {#if applyOutcome.errors.length > 0}
          <details class="text-[11px] text-red-300/80 mt-1">
            <summary class="cursor-pointer">{applyOutcome.errors.length} error{applyOutcome.errors.length === 1 ? "" : "s"}</summary>
            <ul class="ml-3 mt-1 list-disc">
              {#each applyOutcome.errors as err}<li class="font-mono">{err}</li>{/each}
            </ul>
          </details>
        {/if}
      </div>
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors"
        onclick={onCancel}>Done</button>
    </div>
  {/if}
</section>
