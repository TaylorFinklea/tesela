<script lang="ts">
  import { browser } from "$app/environment";
  import { prefs, type BulletStyle, type BareDateField } from "$lib/preferences.svelte";
  import { theme } from "$lib/theme.svelte";
  import { THEMES } from "$lib/themes";
  import { commandRegistry, effectiveShortcut, effectiveChord, checkRebind } from "$lib/command-registry.svelte";
  import { eventToShortcutGlyph } from "$lib/shortcut-glyph";
  import * as keybindings from "$lib/stores/keybindings.svelte";
  // Side-effect import: populates commandRegistry (buildV4Commands runs on
  // module load) so the rebindable list isn't empty on this standalone route
  // — the /g shell loads it via the leader/palette, but /settings doesn't.
  import "$lib/v4/commands";

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

  // ── Keyboard Shortcuts section state ────────────────────────────────────
  // Which command id is currently in "press keys…" capture mode
  let capturingId = $state<string | null>(null);
  // Per-command inline caption (conflict warning / reserved error)
  let captions = $state<Record<string, { kind: 'warn' | 'error'; text: string }>>({});

  // Commands that have a shortcut or chord defined (compiled-in or overridden)
  let shortcutCommands = $derived(
    commandRegistry.all().filter((c) => {
      const ovr = keybindings.snapshot();
      return effectiveShortcut(c, ovr) !== undefined || (effectiveChord(c, ovr)?.length ?? 0) > 0;
    })
  );

  function startCapture(id: string) {
    capturingId = id;
    captions = { ...captions, [id]: { kind: 'warn', text: '' } };
    // clear caption once capturing starts
    const { [id]: _, ...rest } = captions;
    captions = rest;
  }

  function cancelCapture() {
    capturingId = null;
  }

  // Focus the "press keys…" span the moment it mounts so the one-shot
  // onkeydown actually receives the rebind keystroke (clicking Rebind alone
  // leaves focus on the button → keydown went to <body> and never captured).
  function focusOnMount(node: HTMLElement) {
    node.focus();
  }

  function handleRebindKey(e: KeyboardEvent, cmdId: string) {
    e.preventDefault();
    e.stopPropagation();

    if (e.key === 'Escape') {
      cancelCapture();
      return;
    }

    if (e.key === 'Backspace' || e.key === 'Delete') {
      keybindings.setShortcut(cmdId, null);
      cancelCapture();
      // clear any caption
      const { [cmdId]: _, ...rest } = captions;
      captions = rest;
      return;
    }

    const glyph = eventToShortcutGlyph(e);
    if (!glyph) return; // bare modifier key — wait for a real combo

    const ovr = keybindings.snapshot();
    const result = checkRebind(cmdId, 'shortcut', glyph, ovr);

    if (result.ok) {
      keybindings.setShortcut(cmdId, glyph);
      cancelCapture();
      const { [cmdId]: _, ...rest } = captions;
      captions = rest;
    } else if (result.reason === 'reserved') {
      // Refuse and show inline error — stay in capture mode so user can try again
      captions = { ...captions, [cmdId]: { kind: 'error', text: `macOS reserves ${glyph} — pick another` } };
    } else if (result.reason === 'taken') {
      // Allow but warn
      keybindings.setShortcut(cmdId, glyph);
      cancelCapture();
      const names = result.by.map((c) => c.label).join(', ');
      captions = { ...captions, [cmdId]: { kind: 'warn', text: `⚠ Taken by ${names}` } };
    }
  }
</script>

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

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Editor</h2>
  <label class="flex items-center gap-3 cursor-pointer">
    <button
      class="w-9 h-5 rounded-full transition-colors {vimEnabled === 'true' ? 'bg-primary' : 'bg-muted'}"
      onclick={handleVimToggle}
      aria-label="Toggle vim mode"
    >
      <span class="block w-3.5 h-3.5 rounded-full bg-background transition-transform {vimEnabled === 'true' ? 'translate-x-4.5' : 'translate-x-0.5'}"></span>
    </button>
    <span class="text-[13px]">Vim mode</span>
  </label>
  <label class="flex items-center gap-3 cursor-pointer mt-3">
    <button
      class="w-9 h-5 rounded-full transition-colors {prefs.newEntityGuard ? 'bg-primary' : 'bg-muted'}"
      onclick={() => prefs.setNewEntityGuard(!prefs.newEntityGuard)}
      aria-label="Toggle new entity confirmation"
    >
      <span class="block w-3.5 h-3.5 rounded-full bg-background transition-transform {prefs.newEntityGuard ? 'translate-x-4.5' : 'translate-x-0.5'}"></span>
    </button>
    <span class="text-[13px]">Confirm near-match tags, links, and select values</span>
  </label>
  <p class="text-[11px] text-muted-foreground/40 mt-1.5">When on, typo-like new entities ask before creating. Esc/click-away creates the typed value.</p>
</section>

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
  <div class="flex items-center gap-2 mt-3">
    <span class="text-[13px] mr-3">Bare date field:</span>
    {#each [{ id: "scheduled" as BareDateField, label: "Scheduled" }, { id: "deadline" as BareDateField, label: "Deadline" }] as opt}
      <button
        class="px-3 py-1.5 rounded-md text-[12px] transition-all border {prefs.bareDateField === opt.id ? 'bg-primary/10 text-primary border-primary/20 ring-1 ring-primary/15' : 'text-muted-foreground border-border/50 hover:bg-muted/40 hover:text-foreground'}"
        onclick={() => prefs.setBareDateField(opt.id)}
      >{opt.label}</button>
    {/each}
  </div>
  <p class="text-[11px] text-muted-foreground/40 mt-1.5">A date typed without a <code>deadline</code>/<code>scheduled</code> keyword sets this field.</p>
</section>

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

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Keyboard Shortcuts</h2>
  {#if shortcutCommands.length === 0}
    <p class="text-[11px] text-muted-foreground/40">No commands with shortcuts registered yet.</p>
  {:else}
    <div class="space-y-1 text-[12px]">
      {#each shortcutCommands as cmd (cmd.id)}
        {@const ovr = keybindings.snapshot()}
        {@const effShortcut = effectiveShortcut(cmd, ovr)}
        {@const effChord = effectiveChord(cmd, ovr)}
        {@const isCapturing = capturingId === cmd.id}
        {@const isOverridden = cmd.id in ovr}
        {@const caption = captions[cmd.id]}
        <div class="flex flex-col gap-0.5">
          <div class="flex items-center gap-2 min-h-[28px]">
            <!-- Label -->
            <span class="flex-1 text-muted-foreground/70 truncate">{cmd.label}</span>

            <!-- Effective binding badge(s) -->
            <div class="flex items-center gap-1 shrink-0">
              {#if effShortcut}
                <kbd class="text-[10px] font-mono bg-muted px-1.5 py-0.5 rounded text-muted-foreground">{effShortcut}</kbd>
              {:else if !effChord}
                <span class="text-[10px] text-muted-foreground/30 font-mono">unbound</span>
              {/if}
              {#if effChord && effChord.length > 0}
                <kbd class="text-[10px] font-mono bg-muted px-1.5 py-0.5 rounded text-muted-foreground">{effChord.join(' ')}</kbd>
              {/if}
            </div>

            <!-- Rebind capture button -->
            {#if isCapturing}
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <span
                class="text-[10px] px-2 py-0.5 rounded border border-primary/40 bg-primary/5 text-primary font-mono outline-none min-w-[72px] text-center"
                role="status"
                tabindex="0"
                use:focusOnMount
                onkeydown={(e) => handleRebindKey(e, cmd.id)}
                onblur={() => cancelCapture()}
              >press keys…</span>
            {:else}
              <button
                type="button"
                class="text-[10px] px-2 py-0.5 rounded border border-border/40 text-muted-foreground/50 hover:text-foreground hover:border-border transition-colors"
                onclick={() => startCapture(cmd.id)}
              >Rebind</button>
            {/if}

            <!-- Reset button — only when overridden -->
            {#if isOverridden}
              <button
                type="button"
                class="text-[10px] px-1.5 py-0.5 rounded text-muted-foreground/40 hover:text-muted-foreground transition-colors"
                onclick={() => keybindings.reset(cmd.id)}
                title="Reset to default"
              >↺</button>
            {/if}
          </div>

          <!-- Inline caption (conflict / reserved warning) -->
          {#if caption && caption.text}
            <p class="text-[10px] pl-1 {caption.kind === 'error' ? 'text-destructive' : 'text-amber-500'}">{caption.text}</p>
          {/if}
        </div>
      {/each}
    </div>

    <div class="mt-3 flex justify-end">
      <button
        type="button"
        class="text-[11px] px-2.5 py-1 rounded border border-border/40 text-muted-foreground/50 hover:text-foreground hover:border-border transition-colors"
        onclick={() => keybindings.resetAll()}
      >Reset all to defaults</button>
    </div>
  {/if}
</section>
