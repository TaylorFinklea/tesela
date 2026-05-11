<script lang="ts">
  import { api } from "$lib/api-client";

  let outPath = $state("");
  let mode = $state<"full" | "portable">("full");
  let includeAttachments = $state(false);
  let running = $state(false);
  let picking = $state(false);
  let message = $state<string | null>(null);
  let error = $state<string | null>(null);

  async function pickFolder() {
    if (picking) return;
    picking = true;
    error = null;
    try {
      const res = await api.pickFolder("Pick a folder to export into");
      if (res.path) outPath = res.path;
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      picking = false;
    }
  }

  async function runExport() {
    if (!outPath.trim()) {
      error = "Out path required.";
      return;
    }
    running = true;
    message = null;
    error = null;
    try {
      const res = await api.runExport({
        out_path: outPath,
        mode,
        include_attachments: includeAttachments,
      });
      message = `Exported ${res.note_count} notes${
        res.attachment_count > 0 ? ` + ${res.attachment_count} attachments` : ""
      } (${mode} mode) → ${res.out_path}${
        mode === "portable"
          ? ` · stripped ${res.stripped_property_count} internal properties`
          : ""
      }`;
    } catch (e: any) {
      error = e?.message ?? `${e}`;
    } finally {
      running = false;
    }
  }
</script>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Export markdown
  </h2>
  <div class="space-y-2">
    <div class="flex gap-2">
      <input
        type="text"
        placeholder="/tmp/my-mosaic-export (absolute path)"
        bind:value={outPath}
        class="flex-1 text-[12px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30"
      />
      <button
        class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50"
        disabled={picking}
        onclick={pickFolder}
        title="Browse for a destination folder using Finder"
      >
        {picking ? "…" : "Browse…"}
      </button>
    </div>
    <div class="flex items-center gap-2">
      <span class="text-[12px] text-muted-foreground/70">Mode:</span>
      {#each [
        { id: "full", label: "Full (round-trippable)" },
        { id: "portable", label: "Portable (Obsidian/Logseq-friendly)" },
      ] as opt}
        <button
          class="px-2.5 py-1 rounded-md text-[12px] transition-all border {mode === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
          onclick={() => (mode = opt.id as typeof mode)}>{opt.label}</button>
      {/each}
    </div>
    <label class="flex items-center gap-2 cursor-pointer text-[12px]">
      <input type="checkbox" bind:checked={includeAttachments} class="accent-primary" />
      <span class="text-muted-foreground/80">Include `attachments/` directory</span>
    </label>
    <button
      class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-progress"
      disabled={running}
      onclick={runExport}
    >
      {running ? "Exporting…" : "Run export"}
    </button>
    {#if message}
      <p class="text-[11px] text-emerald-400/80">{message}</p>
    {/if}
    {#if error}
      <p class="text-[11px] text-red-400/90">{error}</p>
    {/if}
  </div>
  <p class="text-[11px] text-muted-foreground/40 mt-2">
    Full mode is byte-exact (re-importable). Portable strips Tesela-internal properties (Apple
    Reminders sync state, frontmatter timestamps, etc.) and writes a README explaining the diff.
  </p>
</section>
