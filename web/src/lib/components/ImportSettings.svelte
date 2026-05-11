<script lang="ts">
  import {
    api,
    type ImportResponse,
    type LogseqPlan,
    type LogseqPlanItem,
    type LogseqDecision,
    type LogseqApplyOutcome,
  } from "$lib/api-client";

  type Kind = "obsidian" | "logseq" | "org";

  let kind = $state<Kind>("obsidian");
  let source = $state("");
  let dryRun = $state(true);
  let running = $state(false);
  let picking = $state(false);
  // Generic (Obsidian/Org) result
  let result = $state<ImportResponse | null>(null);
  let error = $state<string | null>(null);

  // Logseq plan + decision state
  let plan = $state<LogseqPlan | null>(null);
  let defaultPolicy = $state<"skip" | "overwrite" | "rename">("skip");
  let renameSuffix = $state("-imported");
  let perItem = $state<Record<string, LogseqDecision>>({});
  let applying = $state(false);
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
    perItem = {};

    if (kind === "logseq") {
      // Logseq uses the structured plan/apply flow.
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

  function decisionFor(item: LogseqPlanItem): LogseqDecision {
    if (item.kind !== "conflict_diff_sha" && item.kind !== "conflict_foreign") {
      // Non-conflict items aren't user-decisioned (NewImport always
      // proceeds, Unchanged is a no-op, HardSkip is informational).
      return { kind: "skip" };
    }
    const override = perItem[item.source_rel];
    if (override) return override;
    return defaultPolicy === "rename"
      ? { kind: "rename", suffix: renameSuffix }
      : { kind: defaultPolicy };
  }

  function setItemDecision(rel: string, kind: "default" | "skip" | "overwrite" | "rename") {
    const next = { ...perItem };
    if (kind === "default") {
      delete next[rel];
    } else if (kind === "rename") {
      next[rel] = { kind: "rename", suffix: renameSuffix };
    } else {
      next[rel] = { kind };
    }
    perItem = next;
  }

  function applyLabel(item: LogseqPlanItem): string {
    const d = decisionFor(item);
    switch (item.kind) {
      case "new_import":
        return "import";
      case "unchanged":
        return "skip (no change)";
      case "hard_skip":
        return "won't import";
      case "conflict_diff_sha":
      case "conflict_foreign":
        return d.kind === "rename" ? `rename → +${d.suffix}` : d.kind;
    }
  }

  async function applyPlan() {
    if (!plan || applying) return;
    applying = true;
    error = null;
    try {
      const def: LogseqDecision =
        defaultPolicy === "rename"
          ? { kind: "rename", suffix: renameSuffix }
          : { kind: defaultPolicy };
      applyOutcome = await api.applyLogseq(plan, {
        per_item: perItem,
        default: def,
      });
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      applying = false;
    }
  }

  function cancelPlan() {
    plan = null;
    perItem = {};
    applyOutcome = null;
  }

  const counts = $derived.by(() => {
    if (!plan) return null;
    let n = 0, u = 0, c = 0, h = 0;
    for (const it of plan.items) {
      if (it.kind === "new_import") n++;
      else if (it.kind === "unchanged") u++;
      else if (it.kind === "conflict_diff_sha" || it.kind === "conflict_foreign") c++;
      else if (it.kind === "hard_skip") h++;
    }
    return { newImports: n, unchanged: u, conflicts: c, hardSkips: h };
  });
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

  {#if plan && counts && !applyOutcome}
    <!-- Plan preview -->
    <div class="space-y-3">
      <div class="text-[11px] text-muted-foreground/80">
        Reviewing import from <span class="font-mono">{plan.source}</span>
      </div>

      <div class="grid grid-cols-4 gap-2 text-[12px]">
        <div class="border border-border/40 rounded-md p-2">
          <div class="text-[10px] text-muted-foreground/60 uppercase tracking-wider">New</div>
          <div class="text-foreground/90 text-[15px] font-medium">{counts.newImports}</div>
        </div>
        <div class="border border-border/40 rounded-md p-2">
          <div class="text-[10px] text-muted-foreground/60 uppercase tracking-wider">Unchanged</div>
          <div class="text-foreground/90 text-[15px] font-medium">{counts.unchanged}</div>
        </div>
        <div class="border {counts.conflicts > 0 ? 'border-amber-400/40' : 'border-border/40'} rounded-md p-2">
          <div class="text-[10px] text-muted-foreground/60 uppercase tracking-wider">Conflicts</div>
          <div class="text-foreground/90 text-[15px] font-medium {counts.conflicts > 0 ? 'text-amber-400/90' : ''}">{counts.conflicts}</div>
        </div>
        <div class="border border-border/40 rounded-md p-2">
          <div class="text-[10px] text-muted-foreground/60 uppercase tracking-wider">Skipped</div>
          <div class="text-foreground/90 text-[15px] font-medium">{counts.hardSkips}</div>
        </div>
      </div>

      {#if counts.conflicts > 0}
        <div class="border border-amber-400/30 bg-amber-400/5 rounded-md px-3 py-2 space-y-2">
          <div class="text-[11px] text-amber-300/90">
            {counts.conflicts} conflict{counts.conflicts === 1 ? "" : "s"} — pick a default action, then override per row if needed.
          </div>
          <div class="flex items-center gap-2">
            <span class="text-[11px] text-muted-foreground/70">Default action:</span>
            {#each [
              { id: "skip", label: "Skip" },
              { id: "overwrite", label: "Overwrite" },
              { id: "rename", label: "Rename" },
            ] as opt}
              <button
                class="px-2 py-0.5 rounded text-[11px] border transition-all {defaultPolicy === opt.id ? 'bg-primary/10 text-primary border-primary/30' : 'text-muted-foreground border-border/40 hover:bg-muted/40 hover:text-foreground'}"
                onclick={() => (defaultPolicy = opt.id as typeof defaultPolicy)}>{opt.label}</button>
            {/each}
            {#if defaultPolicy === "rename"}
              <span class="text-[11px] text-muted-foreground/70 ml-2">Suffix:</span>
              <input
                type="text"
                bind:value={renameSuffix}
                class="text-[11px] bg-muted/50 rounded px-2 py-0.5 w-28 font-mono outline-none border border-transparent focus:border-ring/30"
              />
            {/if}
          </div>
        </div>
      {/if}

      <!-- Plan items table -->
      <div class="border border-border/40 rounded-md max-h-96 overflow-y-auto">
        <table class="w-full text-[11px]">
          <thead class="bg-muted/40 text-muted-foreground/70 sticky top-0">
            <tr>
              <th class="text-left px-2 py-1.5 font-medium">Source</th>
              <th class="text-left px-2 py-1.5 font-medium">Target</th>
              <th class="text-left px-2 py-1.5 font-medium">Status</th>
              <th class="text-left px-2 py-1.5 font-medium">Action</th>
            </tr>
          </thead>
          <tbody>
            {#each plan.items as item}
              <tr class="border-t border-border/30
                  {item.kind === 'conflict_diff_sha' || item.kind === 'conflict_foreign' ? 'bg-amber-400/5' : ''}
                  {item.kind === 'hard_skip' ? 'opacity-60' : ''}
                ">
                <td class="px-2 py-1 font-mono text-foreground/80 break-all">{item.source_rel}</td>
                <td class="px-2 py-1 font-mono text-muted-foreground/60 break-all">
                  {item.target_id || "—"}
                </td>
                <td class="px-2 py-1">
                  {#if item.kind === "new_import"}<span class="text-emerald-300/80">new</span>{/if}
                  {#if item.kind === "unchanged"}<span class="text-muted-foreground/60">unchanged</span>{/if}
                  {#if item.kind === "conflict_diff_sha"}<span class="text-amber-300/90">edited</span>{/if}
                  {#if item.kind === "conflict_foreign"}<span class="text-amber-300/90">foreign</span>{/if}
                  {#if item.kind === "hard_skip"}<span class="text-muted-foreground/40">can't import</span>{/if}
                </td>
                <td class="px-2 py-1">
                  {#if item.kind === "conflict_diff_sha" || item.kind === "conflict_foreign"}
                    {@const override = perItem[item.source_rel]}
                    <div class="flex items-center gap-1">
                      {#each [
                        { id: "default", label: "Default" },
                        { id: "skip", label: "Skip" },
                        { id: "overwrite", label: "Overwrite" },
                        { id: "rename", label: "Rename" },
                      ] as opt}
                        {@const active = opt.id === "default" ? !override
                                       : opt.id === "rename" ? override?.kind === "rename"
                                       : override?.kind === opt.id}
                        <button
                          class="px-1.5 py-0.5 rounded text-[10px] border transition-all {active ? 'bg-primary/10 text-primary border-primary/30' : 'text-muted-foreground border-border/40 hover:bg-muted/40 hover:text-foreground'}"
                          onclick={() => setItemDecision(item.source_rel, opt.id as any)}>{opt.label}</button>
                      {/each}
                      <span class="ml-2 text-[10px] text-muted-foreground/50">→ {applyLabel(item)}</span>
                    </div>
                    {#if item.reason}
                      <div class="text-[10px] text-muted-foreground/50 mt-0.5">{item.reason}</div>
                    {/if}
                  {:else}
                    <span class="text-[10px] text-muted-foreground/50">{applyLabel(item)}</span>
                    {#if item.reason}
                      <div class="text-[10px] text-muted-foreground/50 mt-0.5">{item.reason}</div>
                    {/if}
                  {/if}
                </td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>

      <div class="flex items-center gap-2">
        <button
          class="px-3 py-1.5 rounded-md text-[12px] border border-primary/30 bg-primary/10 text-primary hover:bg-primary/20 transition-colors disabled:opacity-50"
          disabled={applying}
          onclick={applyPlan}>{applying ? "Applying…" : "Apply"}</button>
        <button
          class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors"
          onclick={cancelPlan}>Cancel</button>
      </div>
      {#if error}
        <p class="text-[11px] text-red-400/90">{error}</p>
      {/if}
    </div>
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
        onclick={cancelPlan}>Done</button>
    </div>
  {/if}
</section>
