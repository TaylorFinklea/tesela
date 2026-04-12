<script lang="ts">
  let activeOption = $state(0);
  const options = ["Frosted Glass", "Glass Accents", "Depth Layers", "Neon Glow"];
</script>

<div class="flex-1 flex flex-col bg-[#0a0a0f] overflow-hidden">
  <!-- Tab bar -->
  <div class="flex items-center gap-1 px-4 py-3 border-b border-white/5">
    <span class="text-[13px] font-semibold text-white/60 mr-4">Design Preview</span>
    {#each options as opt, i}
      <button
        class="px-3 py-1.5 rounded-md text-[12px] transition-all {activeOption === i ? 'bg-teal-500/20 text-teal-300 border border-teal-500/30' : 'text-white/40 hover:text-white/70 hover:bg-white/5'}"
        onclick={() => (activeOption = i)}
      >{opt}</button>
    {/each}
  </div>

  <!-- Preview area -->
  <div class="flex-1 relative overflow-hidden">
    <!-- Subtle gradient background for glass effects -->
    <div class="absolute inset-0 bg-gradient-to-br from-teal-900/20 via-[#0a0a0f] to-purple-900/10"></div>
    <div class="absolute top-20 left-40 w-64 h-64 bg-teal-500/5 rounded-full blur-3xl"></div>
    <div class="absolute bottom-20 right-40 w-48 h-48 bg-purple-500/5 rounded-full blur-3xl"></div>

    <div class="relative flex h-full">
      {#if activeOption === 0}
        <!-- FROSTED GLASS PANELS -->
        <!-- Sidebar -->
        <div class="w-56 backdrop-blur-xl bg-white/[0.03] border-r border-white/[0.06] flex flex-col">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.06]">
            <span class="text-[13px] font-semibold text-white/80 tracking-tight">Tesela</span>
          </div>
          <div class="px-3 py-2">
            <div class="bg-white/[0.04] rounded-lg px-3 py-1.5 text-[12px] text-white/30 border border-white/[0.06]">Filter...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="flex items-center gap-2 px-2 py-1.5 rounded-lg text-[12px] text-white/50 hover:bg-white/[0.04] transition-colors">
              <span class="text-[11px]">☀</span> Today
            </div>
            <div class="flex items-center gap-2 px-2 py-1.5 rounded-lg text-[12px] text-white/50 hover:bg-white/[0.04] transition-colors">
              <span class="text-[11px]">📅</span> Timeline
            </div>
            <div class="flex items-center gap-2 px-2 py-1.5 rounded-lg text-[12px] text-white/50 hover:bg-white/[0.04] transition-colors">
              <span class="text-[11px]">◉</span> Graph
            </div>
          </div>
          <div class="px-3 mt-3">
            <div class="text-[10px] font-medium text-white/20 uppercase tracking-widest px-2 mb-1">Pages</div>
            <div class="space-y-px">
              <div class="px-2 py-1.5 rounded-lg text-[12px] bg-teal-500/10 text-teal-300 border border-teal-500/10 font-medium">Test Outliner</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Task</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Daily Notes</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Project Ideas</div>
            </div>
          </div>
          <div class="mt-auto px-4 py-2 border-t border-white/[0.04]">
            <div class="text-[10px] text-white/15">23 notes · j/k nav</div>
          </div>
        </div>

        <!-- Main content -->
        <div class="flex-1 flex flex-col">
          <div class="h-12 px-5 flex items-center gap-3 border-b border-white/[0.06]">
            <span class="text-[11px] text-white/30">←</span>
            <span class="text-[13px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-1.5 py-0.5 rounded-md bg-teal-500/10 text-teal-400/80 border border-teal-500/10">Project</span>
          </div>
          <div class="flex-1 px-8 py-6 overflow-y-auto">
            <div class="space-y-1 max-w-2xl">
              <div class="flex items-start gap-0 border-l-2 border-teal-500/30 pl-3 py-1">
                <div>
                  <div class="text-[13px] text-white/85 font-medium">Ship the docs refresh <span class="text-[11px] px-1.5 py-0.5 rounded bg-white/[0.06] text-white/50 ml-1">#Task</span></div>
                  <div class="flex gap-3 mt-1">
                    <span class="text-[11px] text-white/30"><span class="text-white/20">status</span> doing</span>
                    <span class="text-[11px] text-white/30"><span class="text-white/20">priority</span> high</span>
                  </div>
                </div>
              </div>
              <div class="pl-8 space-y-1">
                <div class="border-l-2 border-transparent pl-3 py-1">
                  <div class="text-[13px] text-white/60">Write the getting started guide</div>
                </div>
                <div class="border-l-2 border-transparent pl-3 py-1">
                  <div class="text-[13px] text-white/60">Review with <span class="text-teal-400/70 underline underline-offset-2 decoration-teal-400/20">Person</span> team</div>
                </div>
              </div>
              <div class="flex items-start gap-0 border-l-2 border-transparent pl-3 py-1">
                <div>
                  <div class="text-[13px] text-white/85 font-medium">Fix the login bug <span class="text-[11px] px-1.5 py-0.5 rounded bg-white/[0.06] text-white/50 ml-1">#Task</span> <span class="text-[11px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400/70 ml-0.5">#urgent</span></div>
                  <div class="flex gap-3 mt-1">
                    <span class="text-[11px] text-white/30"><span class="text-white/20">status</span> todo</span>
                  </div>
                </div>
              </div>
              <div class="pl-8 space-y-1">
                <div class="border-l-2 border-transparent pl-3 py-1">
                  <div class="text-[13px] text-white/60">Reproduce on staging</div>
                </div>
                <div class="border-l-2 border-transparent pl-3 py-1">
                  <div class="text-[13px] text-white/60">Check auth middleware</div>
                </div>
              </div>
              <div class="flex items-start gap-0 border-l-2 border-transparent pl-3 py-1">
                <div>
                  <div class="text-[13px] text-white/85 font-medium">Weekly sync with <span class="text-teal-400/70 underline underline-offset-2 decoration-teal-400/20">Domain</span> leads</div>
                </div>
              </div>
            </div>
          </div>
          <!-- Status bar -->
          <div class="h-6 bg-white/[0.02] border-t border-white/[0.04] flex items-center px-4 gap-4">
            <span class="text-[11px] font-mono font-bold text-teal-400">NORMAL</span>
            <span class="text-[11px] font-mono text-white/25">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-1.5 w-1.5 rounded-full bg-teal-500/60 mr-1"></span>
            <span class="text-[11px] font-mono text-white/20">connected</span>
          </div>
        </div>

        <!-- Right sidebar -->
        <div class="w-52 backdrop-blur-xl bg-white/[0.02] border-l border-white/[0.06] flex flex-col">
          <div class="h-12 px-3 flex items-center border-b border-white/[0.06]">
            <span class="text-[10px] font-medium text-white/20 uppercase tracking-widest">Links</span>
          </div>
          <div class="px-3 py-3">
            <div class="text-[10px] text-white/20 uppercase tracking-widest mb-2">Backlinks (1)</div>
            <div class="text-[12px] text-white/40 px-1">test-outliner</div>
          </div>
          <div class="px-3 py-3 border-t border-white/[0.04]">
            <div class="text-[10px] text-white/20 uppercase tracking-widest mb-2">Forward (3)</div>
            <div class="space-y-0.5">
              <div class="text-[12px] text-teal-400/60 px-1">Person</div>
              <div class="text-[12px] text-teal-400/60 px-1">Domain</div>
              <div class="text-[12px] text-teal-400/60 px-1">LifeProject</div>
            </div>
          </div>
        </div>

      {:else if activeOption === 1}
        <!-- GLASSMORPHISM ACCENTS -->
        <div class="w-56 bg-[#0d0d14] border-r border-white/[0.04] flex flex-col">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.04]">
            <span class="text-[13px] font-semibold text-white/80">Tesela</span>
          </div>
          <div class="px-3 py-2">
            <div class="bg-white/[0.03] rounded-lg px-3 py-1.5 text-[12px] text-white/30">Filter...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/50">☀ Today</div>
            <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/50">📅 Timeline</div>
          </div>
          <div class="px-3 mt-3">
            <div class="text-[10px] text-white/20 uppercase tracking-widest px-2 mb-1">Pages</div>
            <div class="space-y-px">
              <!-- Glass accent on selected item -->
              <div class="px-2 py-1.5 rounded-lg text-[12px] font-medium text-teal-300 bg-gradient-to-r from-teal-500/15 to-transparent border border-teal-500/20 shadow-[0_0_12px_-3px] shadow-teal-500/10">Test Outliner</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Task</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Daily Notes</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#0a0a0f]">
          <div class="h-12 px-5 flex items-center gap-3 border-b border-white/[0.04]">
            <span class="text-[13px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-1.5 py-0.5 rounded-md bg-teal-500/10 text-teal-400/80">Project</span>
          </div>
          <div class="flex-1 px-8 py-6">
            <div class="space-y-1 max-w-2xl">
              <!-- Glass accent on focused block -->
              <div class="border-l-2 border-teal-500/40 pl-3 py-1.5 bg-gradient-to-r from-teal-500/[0.04] to-transparent rounded-r-lg">
                <div class="text-[13px] text-white/85">Ship the docs refresh <span class="text-[11px] px-1.5 py-0.5 rounded bg-white/[0.06] text-white/50">#Task</span></div>
                <div class="text-[11px] text-white/25 mt-0.5">status doing · priority high</div>
              </div>
              <div class="pl-8 space-y-0.5">
                <div class="pl-3 py-1 text-[13px] text-white/55">Write the getting started guide</div>
                <div class="pl-3 py-1 text-[13px] text-white/55">Review with <span class="text-teal-400/70">Person</span> team</div>
              </div>
              <div class="border-l-2 border-transparent pl-3 py-1.5">
                <div class="text-[13px] text-white/85">Fix the login bug <span class="text-[11px] px-1.5 py-0.5 rounded bg-white/[0.06] text-white/50">#Task</span> <span class="text-[11px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400/70">#urgent</span></div>
                <div class="text-[11px] text-white/25 mt-0.5">status todo</div>
              </div>
            </div>
          </div>
          <div class="h-6 bg-[#080810] border-t border-white/[0.04] flex items-center px-4">
            <span class="text-[11px] font-mono font-bold text-teal-400">NORMAL</span>
            <span class="text-[11px] font-mono text-white/20 ml-4">test-outliner</span>
          </div>
        </div>

      {:else if activeOption === 2}
        <!-- DEPTH LAYERS -->
        <div class="w-56 bg-[#0e0e16] border-r border-white/[0.05] flex flex-col shadow-[4px_0_24px_-4px] shadow-black/40">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.05]">
            <span class="text-[13px] font-semibold text-white/80">Tesela</span>
          </div>
          <div class="px-3 py-2">
            <div class="bg-[#12121e] rounded-lg px-3 py-1.5 text-[12px] text-white/30 shadow-inner shadow-black/20">Filter...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/50">☀ Today</div>
            <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/50">📅 Timeline</div>
          </div>
          <div class="px-3 mt-3">
            <div class="text-[10px] text-white/20 uppercase tracking-widest px-2 mb-1">Pages</div>
            <div class="space-y-1">
              <!-- Elevated card for selected -->
              <div class="px-2 py-1.5 rounded-lg text-[12px] font-medium text-teal-300 bg-[#14142a] shadow-lg shadow-black/30 border border-teal-500/10">Test Outliner</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Task</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#0a0a12]">
          <div class="h-12 px-5 flex items-center gap-3 border-b border-white/[0.05]">
            <span class="text-[13px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-1.5 py-0.5 rounded-md bg-[#14142a] text-teal-400/80 shadow-sm border border-teal-500/10">Project</span>
          </div>
          <div class="flex-1 px-8 py-6">
            <div class="space-y-2 max-w-2xl">
              <!-- Elevated focused block -->
              <div class="border-l-2 border-teal-500/30 pl-3 py-2 bg-[#0e0e18] rounded-r-lg shadow-md shadow-black/20">
                <div class="text-[13px] text-white/85">Ship the docs refresh <span class="text-[11px] px-1.5 py-0.5 rounded bg-[#18182a] text-white/50">#Task</span></div>
                <div class="text-[11px] text-white/25 mt-1">status doing · priority high</div>
              </div>
              <div class="pl-8 space-y-1">
                <div class="pl-3 py-1 text-[13px] text-white/55">Write the getting started guide</div>
                <div class="pl-3 py-1 text-[13px] text-white/55">Review with <span class="text-teal-400/60">Person</span> team</div>
              </div>
              <div class="pl-3 py-2">
                <div class="text-[13px] text-white/85">Fix the login bug <span class="text-[11px] px-1.5 py-0.5 rounded bg-[#18182a] text-white/50">#Task</span></div>
                <div class="text-[11px] text-white/25 mt-1">status todo</div>
              </div>
            </div>
          </div>
          <div class="h-6 bg-[#08080e] border-t border-white/[0.05] flex items-center px-4 shadow-[0_-2px_8px] shadow-black/20">
            <span class="text-[11px] font-mono font-bold text-teal-400">NORMAL</span>
            <span class="text-[11px] font-mono text-white/20 ml-4">test-outliner</span>
          </div>
        </div>

      {:else}
        <!-- NEON GLOW -->
        <div class="w-56 bg-[#0a0a0f] border-r border-white/[0.04] flex flex-col">
          <div class="px-4 h-12 flex items-center border-b border-white/[0.04]">
            <span class="text-[13px] font-semibold text-white/80">Tesela</span>
          </div>
          <div class="px-3 py-2">
            <div class="bg-white/[0.03] rounded-lg px-3 py-1.5 text-[12px] text-white/30 border border-white/[0.04] focus-within:border-teal-500/30 focus-within:shadow-[0_0_8px] focus-within:shadow-teal-500/10">Filter...</div>
          </div>
          <div class="px-3 space-y-0.5">
            <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/50">☀ Today</div>
            <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/50">📅 Timeline</div>
          </div>
          <div class="px-3 mt-3">
            <div class="text-[10px] text-white/20 uppercase tracking-widest px-2 mb-1">Pages</div>
            <div class="space-y-px">
              <!-- Neon glow on selected -->
              <div class="px-2 py-1.5 rounded-lg text-[12px] font-medium text-teal-300 bg-teal-500/[0.08] border border-teal-500/20 shadow-[0_0_15px_-3px] shadow-teal-500/20 relative">
                <div class="absolute left-0 top-1 bottom-1 w-0.5 bg-teal-400 rounded-full shadow-[0_0_6px] shadow-teal-400"></div>
                <span class="ml-1.5">Test Outliner</span>
              </div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Weekly Review</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Task</div>
              <div class="px-2 py-1.5 rounded-lg text-[12px] text-white/40">Daily Notes</div>
            </div>
          </div>
        </div>
        <div class="flex-1 flex flex-col bg-[#0a0a0f]">
          <div class="h-12 px-5 flex items-center gap-3 border-b border-white/[0.04]">
            <span class="text-[13px] font-semibold text-white/90">Test Outliner</span>
            <span class="text-[10px] px-1.5 py-0.5 rounded-md bg-teal-500/10 text-teal-400 border border-teal-500/15">Project</span>
          </div>
          <div class="flex-1 px-8 py-6">
            <div class="space-y-1 max-w-2xl">
              <!-- Neon glow border on focused block -->
              <div class="relative border-l-2 border-teal-400 shadow-[inset_2px_0_8px_-2px] shadow-teal-500/20 pl-3 py-1.5">
                <div class="text-[13px] text-white/85">Ship the docs refresh <span class="text-[11px] px-1.5 py-0.5 rounded bg-white/[0.06] text-white/50">#Task</span></div>
                <div class="text-[11px] text-white/25 mt-0.5">status doing · priority high</div>
              </div>
              <div class="pl-8 space-y-0.5">
                <div class="pl-3 py-1 text-[13px] text-white/55 border-l border-white/[0.06]">Write the getting started guide</div>
                <div class="pl-3 py-1 text-[13px] text-white/55 border-l border-white/[0.06]">Review with <span class="text-teal-400/70">Person</span> team</div>
              </div>
              <div class="border-l-2 border-transparent pl-3 py-1.5">
                <div class="text-[13px] text-white/85">Fix the login bug <span class="text-[11px] px-1.5 py-0.5 rounded bg-white/[0.06] text-white/50">#Task</span> <span class="text-[11px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-400/70">#urgent</span></div>
                <div class="text-[11px] text-white/25 mt-0.5">status todo</div>
              </div>
              <div class="pl-8 space-y-0.5">
                <div class="pl-3 py-1 text-[13px] text-white/55 border-l border-white/[0.06]">Reproduce on staging</div>
                <div class="pl-3 py-1 text-[13px] text-white/55 border-l border-white/[0.06]">Check auth middleware</div>
              </div>
            </div>
          </div>
          <!-- Neon status bar -->
          <div class="h-6 bg-[#060608] border-t border-teal-500/10 flex items-center px-4 shadow-[0_-1px_8px_-2px] shadow-teal-500/10">
            <span class="text-[11px] font-mono font-bold text-teal-400 drop-shadow-[0_0_4px_rgba(20,184,166,0.4)]">NORMAL</span>
            <span class="text-[11px] font-mono text-white/20 ml-4">test-outliner</span>
            <div class="flex-1"></div>
            <span class="inline-block h-1.5 w-1.5 rounded-full bg-teal-400 shadow-[0_0_6px] shadow-teal-400 mr-1.5"></span>
            <span class="text-[11px] font-mono text-white/20">connected</span>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>
