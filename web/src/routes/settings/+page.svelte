<script lang="ts">
  import { browser } from "$app/environment";
  import { useQueryClient } from "@tanstack/svelte-query";
  import { prefs, type BulletStyle } from "$lib/preferences.svelte";
  import { runRemindersSync } from "$lib/reminders-sync";
  import { theme } from "$lib/theme.svelte";
  import { THEMES } from "$lib/themes";

  function loadSetting(key: string, fallback: string): string {
    if (!browser) return fallback;
    return localStorage.getItem(`tesela:${key}`) ?? fallback;
  }

  function saveSetting(key: string, value: string) {
    if (browser) localStorage.setItem(`tesela:${key}`, value);
  }

  let fontSize = $state(loadSetting("fontSize", "14"));
  let vimEnabled = $state(loadSetting("vimEnabled", "true"));
  let serverUrl = $state(loadSetting("serverUrl", "http://127.0.0.1:7474"));

  let syncing = $state(false);
  const queryClient = useQueryClient();
  async function syncRemindersNow() {
    if (syncing) return;
    syncing = true;
    try { await runRemindersSync(queryClient); }
    finally { syncing = false; }
  }

  function handleFontSizeChange(value: string) {
    fontSize = value;
    saveSetting("fontSize", value);
  }

  function handleVimToggle() {
    vimEnabled = vimEnabled === "true" ? "false" : "true";
    saveSetting("vimEnabled", vimEnabled);
  }

  function handleServerUrlChange(value: string) {
    serverUrl = value;
    saveSetting("serverUrl", value);
  }
</script>

<div class="flex-1 flex flex-col">
  <header class="border-b border-border px-5 h-11 flex items-center shrink-0">
    <h1 class="text-[13px] font-semibold tracking-tight">Settings</h1>
  </header>

  <div class="flex-1 overflow-y-auto">
    <div class="max-w-lg mx-auto py-8 px-6 space-y-8">

      <!-- Font size -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Font Size</h2>
        <div class="flex items-center gap-3">
          <input
            type="range"
            min="12"
            max="18"
            step="1"
            value={fontSize}
            oninput={(e) => handleFontSizeChange((e.target as HTMLInputElement).value)}
            class="flex-1 accent-primary"
          />
          <span class="text-[12px] text-muted-foreground font-mono w-8">{fontSize}px</span>
        </div>
      </section>

      <!-- Theme -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Theme</h2>
        <div class="grid grid-cols-2 gap-2">
          {#each THEMES as t}
            <button
              type="button"
              aria-label={`Switch to ${t.name} theme`}
              class="group flex items-center gap-3 p-2 rounded-md border transition-all text-left {theme.current === t.id ? 'border-primary/50 bg-primary/5 ring-1 ring-primary/20' : 'border-border/40 hover:border-border hover:bg-muted/30'}"
              onclick={() => theme.set(t.id)}
            >
              <span
                class="shrink-0 inline-flex w-10 h-10 rounded overflow-hidden border border-black/20"
                style="background: {t.swatch.bg};"
                aria-hidden="true"
              >
                <span class="flex flex-col w-full">
                  <span class="flex-1 flex">
                    <span class="flex-1" style="background: {t.swatch.bg};"></span>
                    <span class="flex-1" style="background: {t.swatch.fg}; opacity: 0.95;"></span>
                  </span>
                  <span class="flex-1 flex">
                    <span class="flex-1" style="background: {t.swatch.primary};"></span>
                    <span class="flex-1" style="background: {t.swatch.secondary};"></span>
                  </span>
                </span>
              </span>
              <span class="flex-1 min-w-0">
                <span class="block text-[12.5px] truncate {theme.current === t.id ? 'text-primary' : 'text-foreground/90'}">{t.name}</span>
                <span class="block text-[10px] uppercase tracking-wider text-muted-foreground/50">{t.mode}</span>
              </span>
            </button>
          {/each}
        </div>
        <p class="text-[11px] text-muted-foreground/40 mt-2">Switches instantly. Persisted across sessions.</p>
      </section>

      <!-- Vim mode -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Editor</h2>
        <label class="flex items-center gap-3 cursor-pointer">
          <button
            class="w-9 h-5 rounded-full transition-colors {vimEnabled === 'true' ? 'bg-primary' : 'bg-muted'}"
            onclick={handleVimToggle}
          >
            <span class="block w-3.5 h-3.5 rounded-full bg-background transition-transform {vimEnabled === 'true' ? 'translate-x-4.5' : 'translate-x-0.5'}"></span>
          </button>
          <span class="text-[13px]">Vim mode</span>
        </label>
      </section>

      <!-- Outliner -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Outliner</h2>
        <div class="flex items-center gap-2">
          <span class="text-[13px] mr-3">Block bullet:</span>
          {#each [{ id: "dot" as BulletStyle, label: "Dot" }, { id: "arrow" as BulletStyle, label: "Arrow" }] as opt}
            <button
              class="px-3 py-1.5 rounded-md text-[12px] transition-all border {prefs.bulletStyle === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
              onclick={() => prefs.setBulletStyle(opt.id)}
            >{opt.label}</button>
          {/each}
        </div>
        <p class="text-[11px] text-muted-foreground/40 mt-1.5">Dot = Logseq style. Arrow = explicit drill-in chevron.</p>
      </section>

      <!-- Server URL -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Server</h2>
        <input
          type="text"
          value={serverUrl}
          oninput={(e) => handleServerUrlChange((e.target as HTMLInputElement).value)}
          class="w-full text-[13px] bg-muted/50 rounded-md px-3 py-2 text-foreground/90 font-mono outline-none border border-transparent focus:border-ring/30 transition-colors"
        />
        <p class="text-[11px] text-muted-foreground/40 mt-1.5">Restart required after changing</p>
      </section>

      <!-- Apple Reminders sync -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Apple Reminders</h2>
        <button
          class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors disabled:opacity-50 disabled:cursor-progress"
          disabled={syncing}
          onclick={syncRemindersNow}
        >
          {syncing ? "Syncing…" : "Sync now"}
        </button>
        <p class="text-[11px] text-muted-foreground/40 mt-1.5">macOS only. Pulls changes from Reminders.app then pushes Tesela tasks with deadlines.</p>
      </section>

      <!-- Keyboard shortcuts reference -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Keyboard Shortcuts</h2>
        <div class="space-y-1.5 text-[12px]">
          {#each [
            ["⌘K", "Command palette"],
            ["Space", "Leader menu (outside editors)"],
            ["/", "Search / filter"],
            ["1 / b", "Toggle bottom drawer"],
            ["⌃w h/j/k/l", "Focus rail / bottom / focus / right"],
            ["[  ]", "Navigate back / forward"],
            ["j / k", "Rail / drawer: move selection"],
            ["Enter", "Rail / drawer: open selected"],
            ["i", "Vim: Insert mode"],
            ["Esc", "Vim: Normal mode"],
            ["dd", "Vim: Delete block"],
            ["yy / p", "Vim: Yank / paste block"],
            ["o / O", "Vim: New block below / above"],
            [">> / <<", "Vim: Indent / outdent"],
            ["Ctrl+w s", "Split: toggle Kanban split"],
            ["Ctrl+w j / k", "Split: focus bottom / top pane"],
            ["Ctrl+w q", "Split: close split"],
            ["Ctrl+w =", "Split: equalize panes"],
            ["Ctrl+w + / -", "Split: resize panes"],
            ["j / k", "Kanban: prev / next card in column"],
            ["h / l", "Kanban: prev / next column"],
            ["Enter", "Kanban: open focused card's note"],
            ["m", "Kanban: move card to another column"],
          ] as [key, desc]}
            <div class="flex items-center gap-3">
              <kbd class="text-[10px] font-mono bg-muted px-1.5 py-0.5 rounded text-muted-foreground min-w-[48px] text-center">{key}</kbd>
              <span class="text-muted-foreground/70">{desc}</span>
            </div>
          {/each}
        </div>
      </section>

    </div>
  </div>
</div>
