<script lang="ts">
  let activeOption = $state(0);
  const options = ["Frosted Glass", "Glass Accents", "Depth Layers", "Neon Glow", "Warm Mosaic", "Tile Grid", "Terracotta", "Woven"];
</script>

<div class="flex-1 flex flex-col bg-[#08080c] overflow-hidden">
  <!-- Tab bar -->
  <div class="flex items-center gap-1 px-4 py-2.5 border-b border-white/5 flex-wrap">
    <span class="text-[12px] font-semibold text-white/60 mr-3">Design</span>
    {#each options as opt, i}
      <button
        class="px-3 py-1.5 rounded-md text-[12px] transition-all {activeOption === i ? 'bg-amber-500/20 text-amber-300 border border-amber-500/30' : 'text-white/40 hover:text-white/70 hover:bg-white/5'}"
        onclick={() => (activeOption = i)}
      >{opt}</button>
    {/each}
  </div>

  <div class="flex-1 relative overflow-hidden">
    <!-- Colored background blobs for glass effects -->
    <div class="absolute inset-0 bg-gradient-to-br from-amber-950/40 via-[#08080c] to-violet-950/30"></div>
    <div class="absolute top-10 left-60 w-80 h-80 bg-amber-500/8 rounded-full blur-[80px]"></div>
    <div class="absolute bottom-10 right-20 w-60 h-60 bg-violet-500/8 rounded-full blur-[80px]"></div>
    <div class="absolute top-40 right-80 w-40 h-40 bg-amber-500/5 rounded-full blur-[60px]"></div>

    <div class="relative flex h-full">

      {#if activeOption === 0}
        <!-- ═══ FROSTED GLASS ═══ -->
        <div class="w-56 backdrop-blur-xl bg-white/[0.03] border-r border-white/[0.06] flex flex-col">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.06]">
            <span class="text-[14px] font-bold text-white/90 tracking-tight">Tesela</span>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-white/[0.05] backdrop-blur rounded-lg px-3 py-2 text-[12px] text-white/30 border border-white/[0.08]">🔍 Search notes...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-[12px] text-white/60 hover:bg-white/[0.05] transition-colors">
              <span class="text-amber-400 text-[13px]">☀</span> <span>Today</span>
            </div>
            <div class="flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-[12px] text-white/60 hover:bg-white/[0.05] transition-colors">
              <span class="text-blue-400 text-[13px]">📅</span> <span>Timeline</span>
            </div>
            <div class="flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-[12px] text-white/60 hover:bg-white/[0.05] transition-colors">
              <span class="text-violet-400 text-[13px]">◉</span> <span>Graph</span>
            </div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-white/25 uppercase tracking-[0.15em] px-2.5 mb-2">Favorites</div>
            <div class="space-y-0.5">
              <div class="flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-[12px] text-amber-300/70">
                <span class="text-[9px]">⭐</span> Daily Notes
              </div>
            </div>
          </div>
          <div class="px-3 mt-3">
            <div class="text-[10px] font-semibold text-white/25 uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-0.5">
              <div class="px-2.5 py-2 rounded-lg text-[12px] bg-amber-400/[0.12] text-amber-300 border border-amber-400/[0.15] font-medium backdrop-blur-sm">Test Outliner</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/45 hover:text-white/70 hover:bg-white/[0.04]">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/45 hover:text-white/70 hover:bg-white/[0.04]">Task</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/45 hover:text-white/70 hover:bg-white/[0.04]">Project Ideas</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/45 hover:text-white/70 hover:bg-white/[0.04]">Daily Notes</div>
            </div>
          </div>
          <div class="mt-auto px-4 py-2.5 border-t border-white/[0.05]">
            <div class="text-[10px] text-white/20">23 notes</div>
          </div>
        </div>

        <div class="flex-1 flex flex-col">
          <div class="h-12 px-6 flex items-center gap-3 border-b border-white/[0.06]">
            <span class="text-[12px] text-white/25">←</span>
            <span class="text-[14px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-400/15 text-amber-300 border border-amber-400/20 font-medium">Project</span>
          </div>
          <div class="flex-1 px-10 py-8 overflow-y-auto">
            <div class="space-y-1.5 max-w-2xl">
              <div class="border-l-2 border-amber-400/40 pl-4 py-2">
                <div class="text-[14px] text-white/90 font-medium">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-500/15 text-blue-300 border border-blue-500/15 ml-2">#Task</span>
                </div>
                <div class="flex gap-3 mt-1.5">
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/10 text-amber-300 border border-amber-500/10">⏳ doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-red-500/10 text-red-300 border border-red-500/10">🔴 high</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-white/5 text-white/40">📅 2026-04-15</span>
                </div>
              </div>
              <div class="pl-10 space-y-1">
                <div class="border-l border-white/[0.06] pl-4 py-1.5">
                  <div class="text-[13px] text-white/60">Write the getting started guide</div>
                </div>
                <div class="border-l border-white/[0.06] pl-4 py-1.5">
                  <div class="text-[13px] text-white/60">Review with <span class="text-amber-400/80 underline underline-offset-2 decoration-amber-400/30 cursor-pointer">Person</span> team</div>
                </div>
              </div>
              <div class="border-l-2 border-transparent pl-4 py-2">
                <div class="text-[14px] text-white/90 font-medium">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-500/15 text-blue-300 border border-blue-500/15 ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-red-500/15 text-red-300 border border-red-500/15 ml-1">#urgent</span>
                </div>
                <div class="flex gap-3 mt-1.5">
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-white/5 text-white/40">📋 todo</span>
                </div>
              </div>
              <div class="pl-10 space-y-1">
                <div class="border-l border-white/[0.06] pl-4 py-1.5">
                  <div class="text-[13px] text-white/60">Reproduce on staging</div>
                </div>
                <div class="border-l border-white/[0.06] pl-4 py-1.5">
                  <div class="text-[13px] text-white/60">Check auth middleware</div>
                </div>
              </div>
              <div class="border-l-2 border-transparent pl-4 py-2">
                <div class="text-[14px] text-white/90 font-medium">Weekly sync with <span class="text-amber-400/80 underline underline-offset-2 decoration-amber-400/30 cursor-pointer">Domain</span> leads</div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-white/[0.02] border-t border-white/[0.05] flex items-center px-4 gap-4">
            <span class="text-[11px] font-mono font-bold text-amber-400">NORMAL</span>
            <span class="text-[11px] font-mono text-white/25">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400/70 mr-1"></span>
            <span class="text-[11px] font-mono text-white/20">connected</span>
          </div>
        </div>

        <div class="w-52 backdrop-blur-xl bg-white/[0.02] border-l border-white/[0.06] flex flex-col">
          <div class="h-12 px-4 flex items-center border-b border-white/[0.06]">
            <span class="text-[10px] font-semibold text-white/25 uppercase tracking-[0.15em]">Links</span>
          </div>
          <div class="px-4 py-3">
            <div class="text-[10px] text-white/20 uppercase tracking-wider mb-2">Backlinks (1)</div>
            <div class="text-[12px] text-amber-400/60 hover:text-amber-300 cursor-pointer px-1 py-0.5">test-outliner</div>
          </div>
          <div class="px-4 py-3 border-t border-white/[0.04]">
            <div class="text-[10px] text-white/20 uppercase tracking-wider mb-2">Forward (3)</div>
            <div class="space-y-0.5">
              <div class="text-[12px] text-amber-400/60 hover:text-amber-300 cursor-pointer px-1 py-0.5">Person</div>
              <div class="text-[12px] text-amber-400/60 hover:text-amber-300 cursor-pointer px-1 py-0.5">Domain</div>
              <div class="text-[12px] text-violet-400/60 hover:text-violet-300 cursor-pointer px-1 py-0.5">LifeProject</div>
            </div>
          </div>
        </div>

      {:else if activeOption === 1}
        <!-- ═══ GLASS ACCENTS ═══ -->
        <div class="w-56 bg-[#0c0c14] border-r border-white/[0.04] flex flex-col">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.04]">
            <span class="text-[14px] font-bold text-white/85">Tesela</span>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-white/[0.03] rounded-lg px-3 py-2 text-[12px] text-white/25 border border-white/[0.05]">🔍 Search...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-amber-400">☀</span> Today</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-blue-400">📅</span> Timeline</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-violet-400">◉</span> Graph</div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-white/20 uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-0.5">
              <div class="px-2.5 py-2 rounded-lg text-[12px] font-medium text-amber-200 bg-gradient-to-r from-amber-500/[0.15] via-amber-500/[0.08] to-transparent border border-amber-400/[0.15] shadow-[0_0_20px_-5px] shadow-amber-500/15">Test Outliner</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Task</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Project Ideas</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Daily Notes</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#09090f]">
          <div class="h-12 px-6 flex items-center gap-3 border-b border-white/[0.04]">
            <span class="text-[14px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full bg-gradient-to-r from-amber-400/15 to-amber-400/5 text-amber-300 border border-amber-400/15">Project</span>
          </div>
          <div class="flex-1 px-10 py-8">
            <div class="space-y-1.5 max-w-2xl">
              <div class="border-l-2 border-amber-400/40 pl-4 py-2 bg-gradient-to-r from-amber-500/[0.05] to-transparent rounded-r-lg">
                <div class="text-[14px] text-white/90 font-medium">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-400/10 text-blue-300 border border-blue-400/15 ml-2">#Task</span>
                </div>
                <div class="flex gap-2 mt-1.5">
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-400/10 text-amber-300/80">⏳ doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-red-400/10 text-red-300/80">🔴 high</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-white/[0.04] text-white/35">📅 2026-04-15</span>
                </div>
              </div>
              <div class="pl-10 space-y-0.5">
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.05]">Write the getting started guide</div>
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.05]">Review with <span class="text-amber-400/70">Person</span> team</div>
              </div>
              <div class="border-l-2 border-transparent pl-4 py-2">
                <div class="text-[14px] text-white/85">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-400/10 text-blue-300 ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-red-400/10 text-red-300 ml-1">#urgent</span>
                </div>
                <div class="mt-1.5"><span class="text-[10px] px-2 py-0.5 rounded-full bg-white/[0.04] text-white/35">📋 todo</span></div>
              </div>
              <div class="pl-10 space-y-0.5">
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.05]">Reproduce on staging</div>
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.05]">Check auth middleware</div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-[#070710] border-t border-white/[0.04] flex items-center px-4">
            <span class="text-[11px] font-mono font-bold text-amber-400">NORMAL</span>
            <span class="text-[11px] font-mono text-white/20 ml-4">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400/70 mr-1"></span>
            <span class="text-[11px] font-mono text-white/20">connected</span>
          </div>
        </div>

      {:else if activeOption === 2}
        <!-- ═══ DEPTH LAYERS ═══ -->
        <div class="w-56 bg-[#0e0e18] border-r border-white/[0.05] flex flex-col shadow-[6px_0_30px_-5px] shadow-black/50">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.05]">
            <span class="text-[14px] font-bold text-white/85">Tesela</span>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-[#12121f] rounded-lg px-3 py-2 text-[12px] text-white/25 shadow-inner shadow-black/30 border border-white/[0.03]">🔍 Search...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-amber-400">☀</span> Today</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-blue-400">📅</span> Timeline</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-violet-400">◉</span> Graph</div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-white/20 uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-1">
              <div class="px-2.5 py-2 rounded-lg text-[12px] font-medium text-amber-200 bg-[#151528] shadow-lg shadow-black/40 border border-amber-500/[0.12]">Test Outliner</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Task</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Project Ideas</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#0a0a12]">
          <div class="h-12 px-6 flex items-center gap-3 border-b border-white/[0.05]">
            <span class="text-[14px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded-md bg-[#16162a] text-amber-300 shadow-sm border border-amber-500/10">Project</span>
          </div>
          <div class="flex-1 px-10 py-8">
            <div class="space-y-2 max-w-2xl">
              <div class="border-l-2 border-amber-400/30 pl-4 py-2.5 bg-[#0e0e1a] rounded-r-lg shadow-md shadow-black/25">
                <div class="text-[14px] text-white/90 font-medium">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded bg-[#1a1a30] text-blue-300 shadow-sm ml-2">#Task</span>
                </div>
                <div class="flex gap-2 mt-2">
                  <span class="text-[10px] px-2 py-0.5 rounded bg-[#1a1a28] text-amber-300 shadow-sm">⏳ doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded bg-[#1a1a28] text-red-300 shadow-sm">🔴 high</span>
                  <span class="text-[10px] px-2 py-0.5 rounded bg-[#14141e] text-white/35 shadow-sm">📅 2026-04-15</span>
                </div>
              </div>
              <div class="pl-10 space-y-1">
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.05]">Write the getting started guide</div>
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.05]">Review with <span class="text-amber-400/60">Person</span> team</div>
              </div>
              <div class="pl-4 py-2.5">
                <div class="text-[14px] text-white/85">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded bg-[#1a1a30] text-blue-300 ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded bg-[#2a1a1a] text-red-300 ml-1">#urgent</span>
                </div>
                <div class="mt-2"><span class="text-[10px] px-2 py-0.5 rounded bg-[#14141e] text-white/35">📋 todo</span></div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-[#080810] border-t border-white/[0.05] flex items-center px-4 shadow-[0_-4px_12px_-2px] shadow-black/30">
            <span class="text-[11px] font-mono font-bold text-amber-400">NORMAL</span>
            <span class="text-[11px] font-mono text-white/20 ml-4">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-1.5 w-1.5 rounded-full bg-emerald-400/70 mr-1"></span>
            <span class="text-[11px] font-mono text-white/20">connected</span>
          </div>
        </div>

      {:else if activeOption === 3}
        <!-- ═══ NEON GLOW ═══ -->
        <div class="w-56 bg-[#0a0a0f] border-r border-white/[0.04] flex flex-col">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.04]">
            <span class="text-[14px] font-bold text-white/85">Tesela</span>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-white/[0.03] rounded-lg px-3 py-2 text-[12px] text-white/25 border border-white/[0.05]">🔍 Search...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-amber-400">☀</span> Today</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-blue-400">📅</span> Timeline</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-violet-400">◉</span> Graph</div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-white/20 uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-0.5">
              <div class="px-2.5 py-2 rounded-lg text-[12px] font-medium text-amber-200 bg-amber-500/[0.08] border border-amber-400/[0.2] shadow-[0_0_20px_-3px] shadow-amber-500/25 relative">
                <div class="absolute left-0 top-1.5 bottom-1.5 w-[3px] bg-amber-400 rounded-full shadow-[0_0_8px_2px] shadow-amber-400/50"></div>
                <span class="ml-2">Test Outliner</span>
              </div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Task</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Project Ideas</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-white/40">Daily Notes</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#0a0a0f]">
          <div class="h-12 px-6 flex items-center gap-3 border-b border-white/[0.04]">
            <span class="text-[14px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-400/10 text-amber-300 border border-amber-400/20 shadow-[0_0_8px_-2px] shadow-amber-500/20">Project</span>
          </div>
          <div class="flex-1 px-10 py-8">
            <div class="space-y-1.5 max-w-2xl">
              <div class="relative border-l-[3px] border-amber-400 shadow-[inset_3px_0_12px_-3px] shadow-amber-500/25 pl-4 py-2">
                <div class="text-[14px] text-white/90 font-medium">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-500/15 text-blue-300 border border-blue-400/15 shadow-[0_0_6px_-1px] shadow-blue-500/15 ml-2">#Task</span>
                </div>
                <div class="flex gap-2 mt-1.5">
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/10 text-amber-300 border border-amber-400/15 shadow-[0_0_6px_-1px] shadow-amber-500/10">⏳ doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-red-500/10 text-red-300 border border-red-400/15 shadow-[0_0_6px_-1px] shadow-red-500/10">🔴 high</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-white/[0.04] text-white/35 border border-white/[0.06]">📅 2026-04-15</span>
                </div>
              </div>
              <div class="pl-10 space-y-0.5">
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.08]">Write the getting started guide</div>
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.08]">Review with <span class="text-amber-400/80 drop-shadow-[0_0_3px_rgba(20,184,166,0.3)]">Person</span> team</div>
              </div>
              <div class="border-l-[3px] border-transparent pl-4 py-2">
                <div class="text-[14px] text-white/85">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-blue-500/15 text-blue-300 border border-blue-400/15 shadow-[0_0_6px_-1px] shadow-blue-500/15 ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-red-500/15 text-red-300 border border-red-400/15 shadow-[0_0_6px_-1px] shadow-red-500/15 ml-1">#urgent</span>
                </div>
                <div class="mt-1.5"><span class="text-[10px] px-2 py-0.5 rounded-full bg-white/[0.04] text-white/35 border border-white/[0.06]">📋 todo</span></div>
              </div>
              <div class="pl-10 space-y-0.5">
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.08]">Reproduce on staging</div>
                <div class="pl-4 py-1.5 text-[13px] text-white/55 border-l border-white/[0.08]">Check auth middleware</div>
              </div>
              <div class="border-l-[3px] border-transparent pl-4 py-2">
                <div class="text-[14px] text-white/85">Weekly sync with <span class="text-amber-400/80 drop-shadow-[0_0_3px_rgba(20,184,166,0.3)]">Domain</span> leads</div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-[#060608] border-t border-amber-500/[0.08] flex items-center px-4 shadow-[0_-2px_10px_-2px] shadow-amber-500/10">
            <span class="text-[11px] font-mono font-bold text-amber-400 drop-shadow-[0_0_6px_rgba(20,184,166,0.4)]">NORMAL</span>
            <span class="text-[11px] font-mono text-white/20 ml-4">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-1.5 w-1.5 rounded-full bg-amber-400 shadow-[0_0_8px_2px] shadow-amber-400/40 mr-1.5"></span>
            <span class="text-[11px] font-mono text-white/20">connected</span>
          </div>
        </div>
      {:else if activeOption === 4}
        <!-- ═══ WARM MOSAIC ═══ -->
        <!-- Warm earthy tones, subtle tile-like card sections, terracotta accents -->
        <div class="w-56 bg-[#1a1410] border-r border-[#2a2018]/60 flex flex-col">
          <div class="px-4 h-[52px] flex items-center border-b border-[#2a2018]/60">
            <span class="text-[15px] font-bold text-[#e8d5b8] tracking-tight">Tesela</span>
            <span class="text-[9px] ml-1.5 text-[#c4956a]/40 tracking-widest">MOSAIC</span>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-[#221c14] rounded-lg px-3 py-2 text-[12px] text-[#8a7a68] border border-[#2a2018]/50">Search...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-[#c4a882] flex items-center gap-2.5"><span class="text-amber-500">☀</span> Today</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-[#8a7a68] flex items-center gap-2.5"><span class="text-[#7a9eb8]">📅</span> Timeline</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-[#8a7a68] flex items-center gap-2.5"><span class="text-[#b87a9e]">◉</span> Graph</div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-[#5a4a38] uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-1">
              <!-- Tile-like selected item with warm border -->
              <div class="px-2.5 py-2 rounded-lg text-[12px] font-medium text-[#e8d5b8] bg-[#2a2018] border border-[#c4956a]/20 shadow-sm">Test Outliner</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-[#7a6a58]">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-[#7a6a58]">Task</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-[#7a6a58]">Project Ideas</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#141210]">
          <div class="h-[52px] px-6 flex items-center gap-3 border-b border-[#2a2018]/40">
            <span class="text-[15px] font-bold text-[#e8d5b8]">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full bg-[#c4956a]/15 text-[#c4956a] border border-[#c4956a]/20">Project</span>
          </div>
          <div class="flex-1 px-10 py-8">
            <div class="space-y-3 max-w-2xl">
              <!-- Warm tile card for focused block -->
              <div class="bg-[#1c1814] rounded-xl p-4 border border-[#2a2018]/40 shadow-sm">
                <div class="text-[14px] text-[#e8d5b8] font-medium">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-[#7a9eb8]/15 text-[#7a9eb8] ml-2">#Task</span>
                </div>
                <div class="flex gap-2 mt-2">
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-600/10 text-amber-400/80 border border-amber-600/10">doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-red-600/10 text-red-400/80 border border-red-600/10">high</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-[#2a2018] text-[#8a7a68]">📅 2026-04-15</span>
                </div>
              </div>
              <div class="pl-6 space-y-1">
                <div class="pl-3 py-1.5 text-[13px] text-[#a09080] border-l border-[#2a2018]">Write the getting started guide</div>
                <div class="pl-3 py-1.5 text-[13px] text-[#a09080] border-l border-[#2a2018]">Review with <span class="text-[#c4956a]">Person</span> team</div>
              </div>
              <div class="bg-[#181614]/60 rounded-xl p-4 border border-[#2a2018]/20">
                <div class="text-[14px] text-[#d8c5a8]">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-[#7a9eb8]/15 text-[#7a9eb8] ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-red-500/10 text-red-400/70 ml-1">#urgent</span>
                </div>
                <div class="mt-2"><span class="text-[10px] px-2 py-0.5 rounded-full bg-[#2a2018] text-[#8a7a68]">todo</span></div>
              </div>
              <div class="pl-6 space-y-1">
                <div class="pl-3 py-1.5 text-[13px] text-[#a09080] border-l border-[#2a2018]">Reproduce on staging</div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-[#12100e] border-t border-[#2a2018]/40 flex items-center px-4">
            <span class="text-[11px] font-mono font-bold text-[#c4956a]">NORMAL</span>
            <span class="text-[11px] font-mono text-[#5a4a38] ml-4">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-[5px] w-[5px] rounded-full bg-emerald-500/50 mr-1.5"></span>
            <span class="text-[11px] font-mono text-[#5a4a38]">connected</span>
          </div>
        </div>

      {:else if activeOption === 5}
        <!-- ═══ TILE GRID ═══ -->
        <!-- Blocks as distinct tile cards in a grid-like layout, geometric feel -->
        <div class="w-56 bg-[#0e0e14] border-r border-white/[0.04] flex flex-col">
          <div class="px-4 h-[52px] flex items-center border-b border-white/[0.04]">
            <div class="flex items-center gap-2">
              <div class="grid grid-cols-2 gap-0.5 w-3.5 h-3.5">
                <div class="bg-amber-400/60 rounded-[1px]"></div>
                <div class="bg-amber-400/30 rounded-[1px]"></div>
                <div class="bg-amber-400/30 rounded-[1px]"></div>
                <div class="bg-amber-400/60 rounded-[1px]"></div>
              </div>
              <span class="text-[15px] font-bold text-white/85">Tesela</span>
            </div>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-white/[0.03] rounded-md px-3 py-2 text-[12px] text-white/25 border border-white/[0.05]">Search...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2.5 py-2 rounded-md text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-amber-400">☀</span> Today</div>
            <div class="px-2.5 py-2 rounded-md text-[12px] text-white/50 flex items-center gap-2.5"><span class="text-blue-400">📅</span> Timeline</div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-white/20 uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-0.5">
              <div class="px-2.5 py-2 rounded-md text-[12px] font-medium text-amber-200 bg-amber-500/[0.08] border-l-2 border-amber-400">Test Outliner</div>
              <div class="px-2.5 py-1.5 rounded-md text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-md text-[12px] text-white/40">Task</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#0a0a0f]">
          <div class="h-[52px] px-6 flex items-center gap-3 border-b border-white/[0.04]">
            <span class="text-[15px] font-bold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/15">Project</span>
          </div>
          <div class="flex-1 px-8 py-6">
            <!-- Grid of tile cards -->
            <div class="grid gap-2 max-w-2xl">
              <div class="bg-[#12121a] rounded-lg p-4 border border-white/[0.05] hover:border-amber-500/20 transition-colors">
                <div class="text-[14px] text-white/90 font-medium mb-2">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-300 ml-2">#Task</span>
                </div>
                <div class="flex gap-2">
                  <span class="text-[10px] px-2 py-0.5 rounded bg-amber-500/10 text-amber-300/80">doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded bg-red-500/10 text-red-300/80">high</span>
                </div>
                <div class="mt-2 pl-3 space-y-0.5 border-l border-white/[0.06]">
                  <div class="text-[12px] text-white/45">Write the getting started guide</div>
                  <div class="text-[12px] text-white/45">Review with <span class="text-amber-400/60">Person</span> team</div>
                </div>
              </div>
              <div class="bg-[#12121a] rounded-lg p-4 border border-white/[0.05] hover:border-amber-500/20 transition-colors">
                <div class="text-[14px] text-white/85 font-medium mb-2">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded bg-blue-500/10 text-blue-300 ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded bg-red-500/10 text-red-300 ml-1">#urgent</span>
                </div>
                <div class="flex gap-2">
                  <span class="text-[10px] px-2 py-0.5 rounded bg-white/[0.04] text-white/35">todo</span>
                </div>
                <div class="mt-2 pl-3 space-y-0.5 border-l border-white/[0.06]">
                  <div class="text-[12px] text-white/45">Reproduce on staging</div>
                  <div class="text-[12px] text-white/45">Check auth middleware</div>
                </div>
              </div>
              <div class="bg-[#12121a] rounded-lg p-4 border border-white/[0.05] hover:border-amber-500/20 transition-colors">
                <div class="text-[14px] text-white/85 font-medium">Weekly sync with <span class="text-amber-400/70">Domain</span> leads</div>
                <div class="mt-2 pl-3 space-y-0.5 border-l border-white/[0.06]">
                  <div class="text-[12px] text-white/45">Discuss Q2 roadmap</div>
                  <div class="text-[12px] text-white/45">Review <span class="text-amber-400/60">LifeProject</span> priorities</div>
                </div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-[#08080e] border-t border-white/[0.04] flex items-center px-4">
            <span class="text-[11px] font-mono font-bold text-amber-400">NORMAL</span>
            <span class="text-[11px] font-mono text-white/20 ml-4">test-outliner</span>
          </div>
        </div>

      {:else if activeOption === 6}
        <!-- ═══ TERRACOTTA ═══ -->
        <!-- Warm clay/earth, rounded shapes, Mediterranean/artisan feel -->
        <div class="w-56 bg-[#1e1612] border-r border-[#3a2820]/50 flex flex-col">
          <div class="px-4 h-[52px] flex items-center border-b border-[#3a2820]/50">
            <span class="text-[15px] font-bold text-[#f0d8c0] tracking-tight">Tesela</span>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-[#2a1e18] rounded-xl px-3 py-2 text-[12px] text-[#8a6a50] border border-[#3a2820]/30">Search...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2.5 py-2 rounded-xl text-[12px] text-[#c0a088] flex items-center gap-2.5"><span class="text-orange-400">☀</span> Today</div>
            <div class="px-2.5 py-2 rounded-xl text-[12px] text-[#8a7060] flex items-center gap-2.5"><span class="text-sky-400">📅</span> Timeline</div>
            <div class="px-2.5 py-2 rounded-xl text-[12px] text-[#8a7060] flex items-center gap-2.5"><span class="text-rose-400">◉</span> Graph</div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-[#5a4030] uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-1">
              <!-- Terracotta selected tile -->
              <div class="px-2.5 py-2 rounded-xl text-[12px] font-medium text-[#f0d8c0] bg-gradient-to-r from-[#c4704a]/15 to-transparent border border-[#c4704a]/20">Test Outliner</div>
              <div class="px-2.5 py-1.5 rounded-xl text-[12px] text-[#7a6050]">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-xl text-[12px] text-[#7a6050]">Task</div>
              <div class="px-2.5 py-1.5 rounded-xl text-[12px] text-[#7a6050]">Project Ideas</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#161210]">
          <div class="h-[52px] px-6 flex items-center gap-3 border-b border-[#3a2820]/30">
            <span class="text-[15px] font-bold text-[#f0d8c0]">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full bg-[#c4704a]/15 text-[#c4704a] border border-[#c4704a]/15">Project</span>
          </div>
          <div class="flex-1 px-10 py-8">
            <div class="space-y-2 max-w-2xl">
              <div class="border-l-[3px] border-[#c4704a]/50 pl-4 py-2 bg-[#1e1814]/60 rounded-r-xl">
                <div class="text-[14px] text-[#f0d8c0] font-medium">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-sky-500/10 text-sky-300 ml-2">#Task</span>
                </div>
                <div class="flex gap-2 mt-1.5">
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-orange-500/10 text-orange-300 border border-orange-500/10">doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-rose-500/10 text-rose-300 border border-rose-500/10">high</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-[#2a2018] text-[#8a7060]">📅 2026-04-15</span>
                </div>
              </div>
              <div class="pl-10 space-y-1">
                <div class="pl-3 py-1.5 text-[13px] text-[#a08868] border-l border-[#3a2820]/40">Write the getting started guide</div>
                <div class="pl-3 py-1.5 text-[13px] text-[#a08868] border-l border-[#3a2820]/40">Review with <span class="text-[#c4704a]">Person</span> team</div>
              </div>
              <div class="border-l-[3px] border-transparent pl-4 py-2">
                <div class="text-[14px] text-[#d8c0a8]">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-sky-500/10 text-sky-300 ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-rose-500/10 text-rose-300 ml-1">#urgent</span>
                </div>
                <div class="mt-1.5"><span class="text-[10px] px-2 py-0.5 rounded-full bg-[#2a2018] text-[#8a7060]">todo</span></div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-[#12100e] border-t border-[#3a2820]/30 flex items-center px-4">
            <span class="text-[11px] font-mono font-bold text-[#c4704a]">NORMAL</span>
            <span class="text-[11px] font-mono text-[#5a4030] ml-4">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-[5px] w-[5px] rounded-full bg-emerald-400/60 mr-1.5"></span>
            <span class="text-[11px] font-mono text-[#5a4030]">connected</span>
          </div>
        </div>

      {:else if activeOption === 7}
        <!-- ═══ WOVEN ═══ -->
        <!-- Textile/fabric-inspired, fine borders as threads, warm muted palette -->
        <div class="w-56 bg-[#141216] border-r border-[#2a2530]/80 flex flex-col">
          <div class="px-4 h-[52px] flex items-center border-b border-[#2a2530]/80">
            <span class="text-[15px] font-bold text-[#d8c8e0] tracking-tight">Tesela</span>
            <span class="text-[9px] ml-2 text-[#8a78a0]/40 font-mono">///</span>
          </div>
          <div class="px-3 py-2.5">
            <div class="bg-[#1a1620] rounded-lg px-3 py-2 text-[12px] text-[#6a5a7a] border border-[#2a2530]/60">Search...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-[#a898b8] flex items-center gap-2.5"><span class="text-amber-400">☀</span> Today</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-[#7a6a90] flex items-center gap-2.5"><span class="text-indigo-400">📅</span> Timeline</div>
            <div class="px-2.5 py-2 rounded-lg text-[12px] text-[#7a6a90] flex items-center gap-2.5"><span class="text-pink-400">◉</span> Graph</div>
          </div>
          <div class="px-3 mt-4">
            <div class="text-[10px] font-semibold text-[#4a3a58] uppercase tracking-[0.15em] px-2.5 mb-2">Pages</div>
            <div class="space-y-0.5">
              <!-- Woven thread-like selection -->
              <div class="px-2.5 py-2 rounded-lg text-[12px] font-medium text-amber-200 bg-amber-500/[0.06] border-l-2 border-amber-400/40 border-t border-b border-r border-[#2a2530]/40">Test Outliner</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-[#7a6a90] border-l-2 border-transparent">Weekly Review</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-[#7a6a90] border-l-2 border-transparent">Task</div>
              <div class="px-2.5 py-1.5 rounded-lg text-[12px] text-[#7a6a90] border-l-2 border-transparent">Project Ideas</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#0e0c10]">
          <div class="h-[52px] px-6 flex items-center gap-3 border-b border-[#2a2530]/40">
            <span class="text-[15px] font-bold text-[#d8c8e0]">Test Outliner</span>
            <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-400/10 text-amber-300 border border-amber-400/15">Project</span>
          </div>
          <div class="flex-1 px-10 py-8">
            <div class="space-y-1 max-w-2xl">
              <!-- Woven block with top/bottom thread borders -->
              <div class="border-l-2 border-amber-400/30 border-t border-t-[#2a2530]/30 border-b border-b-[#2a2530]/30 pl-4 py-3 rounded-r">
                <div class="text-[14px] text-[#d8c8e0] font-medium">Ship the docs refresh
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-indigo-500/10 text-indigo-300 ml-2">#Task</span>
                </div>
                <div class="flex gap-2 mt-1.5">
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-amber-500/10 text-amber-300/80 border border-amber-500/10">doing</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-rose-500/10 text-rose-300/80 border border-rose-500/10">high</span>
                  <span class="text-[10px] px-2 py-0.5 rounded-full bg-[#1a1620] text-[#7a6a90] border border-[#2a2530]/40">📅 2026-04-15</span>
                </div>
              </div>
              <div class="pl-10 space-y-0">
                <div class="pl-3 py-2 text-[13px] text-[#9a8aaa] border-l border-[#2a2530]/50 border-b border-b-[#2a2530]/20">Write the getting started guide</div>
                <div class="pl-3 py-2 text-[13px] text-[#9a8aaa] border-l border-[#2a2530]/50">Review with <span class="text-amber-400/70">Person</span> team</div>
              </div>
              <div class="border-l-2 border-transparent border-t border-t-[#2a2530]/20 pl-4 py-3">
                <div class="text-[14px] text-[#c8b8d0]">Fix the login bug
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-indigo-500/10 text-indigo-300 ml-2">#Task</span>
                  <span class="text-[10px] px-1.5 py-0.5 rounded-full bg-rose-500/10 text-rose-300 ml-1">#urgent</span>
                </div>
                <div class="mt-1.5"><span class="text-[10px] px-2 py-0.5 rounded-full bg-[#1a1620] text-[#7a6a90] border border-[#2a2530]/40">todo</span></div>
              </div>
              <div class="pl-10 space-y-0">
                <div class="pl-3 py-2 text-[13px] text-[#9a8aaa] border-l border-[#2a2530]/50 border-b border-b-[#2a2530]/20">Reproduce on staging</div>
                <div class="pl-3 py-2 text-[13px] text-[#9a8aaa] border-l border-[#2a2530]/50">Check auth middleware</div>
              </div>
            </div>
          </div>
          <div class="h-7 bg-[#0a080c] border-t border-[#2a2530]/40 flex items-center px-4">
            <span class="text-[11px] font-mono font-bold text-amber-400">NORMAL</span>
            <span class="text-[11px] font-mono text-[#4a3a58] ml-4">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-[5px] w-[5px] rounded-full bg-emerald-400/60 mr-1.5"></span>
            <span class="text-[11px] font-mono text-[#4a3a58]">connected</span>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
