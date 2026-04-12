<script lang="ts">
  import { browser } from "$app/environment";
  import { themes, applyTheme, type ThemeId } from "$lib/themes";

  function loadSetting(key: string, fallback: string): string {
    if (!browser) return fallback;
    return localStorage.getItem(`tesela:${key}`) ?? fallback;
  }

  function saveSetting(key: string, value: string) {
    if (browser) localStorage.setItem(`tesela:${key}`, value);
  }

  let themeId = $state(loadSetting("theme-id", "tesela"));
  let fontSize = $state(loadSetting("fontSize", "14"));
  let vimEnabled = $state(loadSetting("vimEnabled", "true"));
  let serverUrl = $state(loadSetting("serverUrl", "http://127.0.0.1:7474"));

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

      <!-- Theme -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Theme</h2>
        <div class="grid grid-cols-2 gap-2">
          {#each themes as t}
            <button
              class="text-left px-3 py-2.5 rounded-lg text-[12px] transition-all border {themeId === t.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
              onclick={() => { themeId = t.id; saveSetting("theme-id", t.id); applyTheme(t.id); }}
            >
              <div class="font-medium">{t.name}</div>
              <div class="text-[10px] mt-0.5 {themeId === t.id ? 'text-primary/60' : 'text-muted-foreground/40'}">{t.description}</div>
            </button>
          {/each}
        </div>
      </section>

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

      <!-- Keyboard shortcuts reference -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Keyboard Shortcuts</h2>
        <div class="space-y-1.5 text-[12px]">
          {#each [
            ["⌘K", "Command palette"],
            ["Space", "Leader menu (outside editors)"],
            ["/", "Search / filter"],
            ["1", "Toggle sidebar"],
            ["[  ]", "Navigate back / forward"],
            ["j / k", "Sidebar: move selection"],
            ["Enter", "Sidebar: open selected"],
            ["i", "Vim: Insert mode"],
            ["Esc", "Vim: Normal mode"],
            ["dd", "Vim: Delete block"],
            ["yy / p", "Vim: Yank / paste block"],
            ["o / O", "Vim: New block below / above"],
            [">> / <<", "Vim: Indent / outdent"],
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
