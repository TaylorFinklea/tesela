<script lang="ts">
  import { api, type ImportResponse } from "$lib/api-client";

  type Kind = "obsidian" | "logseq" | "org";

  let kind = $state<Kind>("obsidian");
  let source = $state("");
  let dryRun = $state(true);
  let running = $state(false);
  let result = $state<ImportResponse | null>(null);
  let error = $state<string | null>(null);

  async function runImport() {
    if (!source.trim()) {
      error = "Source path required.";
      return;
    }
    running = true;
    error = null;
    result = null;
    try {
      const fn =
        kind === "obsidian"
          ? api.importObsidian
          : kind === "logseq"
            ? api.importLogseq
            : api.importOrg;
      result = await fn(source, dryRun);
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      running = false;
    }
  }
</script>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Import from another tool
  </h2>
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
    <input
      type="text"
      placeholder={kind === "obsidian"
        ? "/Users/you/Documents/MyVault"
        : kind === "logseq"
          ? "/Users/you/Documents/Logseq-Graph"
          : "/Users/you/org-roam"}
      bind:value={source}
      class="w-full text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
    />
    <label class="flex items-center gap-2 cursor-pointer text-[12px]">
      <input type="checkbox" bind:checked={dryRun} class="accent-primary" />
      <span class="text-muted-foreground/80">Dry run (don't write files)</span>
    </label>
    <button
      class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-progress"
      disabled={running}
      onclick={runImport}
    >
      {running ? "Importing…" : dryRun ? "Dry run" : "Run import"}
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
    All importers are idempotent (re-running on unchanged sources is a no-op). Conflicts route to
    `&lt;mosaic&gt;/_import-skipped.log` so notes are never silently overwritten.
  </p>
</section>
