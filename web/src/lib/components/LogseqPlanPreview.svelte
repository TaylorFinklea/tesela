<script lang="ts">
  import {
    api,
    type LogseqPlan,
    type LogseqPlanItem,
    type LogseqDecision,
    type LogseqApplyOutcome,
  } from "$lib/api-client";

  let {
    plan,
    targetMosaic = null,
    onapplied,
    oncancel,
  }: {
    plan: LogseqPlan;
    /** Optional explicit mosaic path. Defaults to the active mosaic. */
    targetMosaic?: string | null;
    onapplied?: (outcome: LogseqApplyOutcome) => void;
    oncancel?: () => void;
  } = $props();

  let defaultPolicy = $state<"skip" | "overwrite" | "rename">("skip");
  let renameSuffix = $state("-imported");
  let perItem = $state<Record<string, LogseqDecision>>({});
  let applying = $state(false);
  let error = $state<string | null>(null);

  const counts = $derived.by(() => {
    let n = 0, u = 0, c = 0, h = 0;
    for (const it of plan.items) {
      if (it.kind === "new_import") n++;
      else if (it.kind === "unchanged") u++;
      else if (it.kind === "conflict_diff_sha" || it.kind === "conflict_foreign") c++;
      else if (it.kind === "hard_skip") h++;
    }
    return { newImports: n, unchanged: u, conflicts: c, hardSkips: h };
  });

  function decisionFor(item: LogseqPlanItem): LogseqDecision {
    if (item.kind !== "conflict_diff_sha" && item.kind !== "conflict_foreign") {
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
    if (kind === "default") delete next[rel];
    else if (kind === "rename") next[rel] = { kind: "rename", suffix: renameSuffix };
    else next[rel] = { kind };
    perItem = next;
  }

  function applyLabel(item: LogseqPlanItem): string {
    const d = decisionFor(item);
    switch (item.kind) {
      case "new_import": return "import";
      case "unchanged": return "skip (no change)";
      case "hard_skip": return "won't import";
      case "conflict_diff_sha":
      case "conflict_foreign":
        return d.kind === "rename" ? `rename → +${d.suffix}` : d.kind;
    }
  }

  async function apply() {
    if (applying) return;
    applying = true;
    error = null;
    try {
      const def: LogseqDecision =
        defaultPolicy === "rename"
          ? { kind: "rename", suffix: renameSuffix }
          : { kind: defaultPolicy };
      const outcome = await api.applyLogseq(plan, {
        per_item: perItem,
        default: def,
      }, targetMosaic ?? undefined);
      onapplied?.(outcome);
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      applying = false;
    }
  }
</script>

<div class="space-y-3">
  <div class="text-[11px] text-muted-foreground/80">
    {plan.items.length} item{plan.items.length === 1 ? "" : "s"}
    from <span class="font-mono">{plan.source}</span>
    {#if targetMosaic}
      → <span class="font-mono">{targetMosaic}</span>
    {/if}
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
            <td class="px-2 py-1 font-mono text-muted-foreground/60 break-all">{item.target_id || "—"}</td>
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
      onclick={apply}>{applying ? "Applying…" : "Apply"}</button>
    <button
      class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors"
      onclick={() => oncancel?.()}>Cancel</button>
  </div>
  {#if error}
    <p class="text-[11px] text-red-400/90">{error}</p>
  {/if}
</div>
