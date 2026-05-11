<script lang="ts">
  import { browser } from "$app/environment";
  import { onMount } from "svelte";
  import { useQueryClient } from "@tanstack/svelte-query";
  import { prefs, type BulletStyle } from "$lib/preferences.svelte";
  import { runRemindersSync } from "$lib/reminders-sync";
  import { api, type RemindersLastSync } from "$lib/api-client";
  import {
    isEnabled as isNotifyEnabled,
    setEnabled as setNotifyEnabled,
    isMuted as isKindMuted,
    setMuted as setKindMuted,
    requestPermission as requestNotifyPermission,
    permissionState,
    type NotificationKind,
  } from "$lib/notifications";

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
  let lastSync = $state<RemindersLastSync | null>(null);
  const queryClient = useQueryClient();

  // Phase 12.3 — notification toggles. Permission state is read on mount
  // (it isn't reactive across permission changes; user has to refresh
  // settings if they revoke browser-side, which is rare).
  let notifyEnabled = $state(isNotifyEnabled());
  let notifyPermission = $state<NotificationPermission>("default");
  // Per-kind mute toggles.
  let muteDeadline = $state(isKindMuted("deadline"));
  let muteScheduled = $state(isKindMuted("scheduled"));
  let muteRecurring = $state(isKindMuted("recurring"));

  async function toggleNotifyEnabled() {
    notifyEnabled = !notifyEnabled;
    setNotifyEnabled(notifyEnabled);
    if (notifyEnabled && notifyPermission === "default") {
      notifyPermission = await requestNotifyPermission();
    }
  }
  function toggleMute(kind: NotificationKind) {
    let next: boolean;
    if (kind === "deadline") next = (muteDeadline = !muteDeadline);
    else if (kind === "scheduled") next = (muteScheduled = !muteScheduled);
    else next = (muteRecurring = !muteRecurring);
    setKindMuted(kind, next);
  }
  async function askPermission() {
    notifyPermission = await requestNotifyPermission();
  }

  async function refreshLastSync() {
    try { lastSync = await api.remindersStatus(); }
    catch { /* server not reachable; leave as-is */ }
  }
  async function syncRemindersNow() {
    if (syncing) return;
    syncing = true;
    try {
      await runRemindersSync(queryClient);
      await refreshLastSync();
    } finally { syncing = false; }
  }

  onMount(() => {
    void refreshLastSync();
    notifyPermission = permissionState();
    // Refresh every 15s so the auto-sync triggers reflect in the UI
    // without needing a page reload. Cheap call; just reads in-memory
    // state on the server.
    const id = setInterval(refreshLastSync, 15_000);
    return () => clearInterval(id);
  });

  function formatRelative(iso: string | null): string {
    if (!iso) return "never";
    const then = new Date(iso).getTime();
    const now = Date.now();
    const sec = Math.max(0, Math.round((now - then) / 1000));
    if (sec < 60) return `${sec}s ago`;
    const min = Math.round(sec / 60);
    if (min < 60) return `${min}m ago`;
    const hr = Math.round(min / 60);
    if (hr < 24) return `${hr}h ago`;
    return new Date(iso).toLocaleString();
  }

  function summarizeOutcome(s: RemindersLastSync | null): string {
    if (!s) return "";
    if (s.error) return `error: ${s.error}`;
    if (!s.outcome) return "";
    const o = s.outcome;
    const created = o.push.created.length;
    const updated = o.push.updated.length;
    const pulled = o.pull.updated.length;
    const errs = o.pull.errors.length + o.push.errors.length;
    const parts: string[] = [];
    if (created > 0) parts.push(`${created} new`);
    if (updated > 0) parts.push(`${updated} pushed`);
    if (pulled > 0) parts.push(`${pulled} pulled`);
    if (errs > 0) parts.push(`${errs} error${errs === 1 ? "" : "s"}`);
    return parts.length === 0 ? "no changes" : parts.join(", ");
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

<div class="flex-1 flex flex-col min-h-0">
  <header class="border-b border-border px-5 h-11 flex items-center shrink-0">
    <h1 class="text-[13px] font-semibold tracking-tight">Settings</h1>
  </header>

  <div class="flex-1 overflow-y-auto min-h-0">
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

      <!-- Notifications -->
      <section>
        <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Notifications</h2>
        <label class="flex items-center gap-3 cursor-pointer">
          <button
            class="w-9 h-5 rounded-full transition-colors {notifyEnabled ? 'bg-primary' : 'bg-muted'}"
            onclick={toggleNotifyEnabled}
          >
            <span class="block w-3.5 h-3.5 rounded-full bg-background transition-transform {notifyEnabled ? 'translate-x-4.5' : 'translate-x-0.5'}"></span>
          </button>
          <span class="text-[13px]">Desktop notifications</span>
        </label>
        {#if notifyEnabled}
          <div class="mt-3 space-y-2">
            {#if notifyPermission === "default"}
              <button
                class="px-3 py-1.5 rounded-md text-[12px] border border-border/50 hover:bg-muted/40 hover:text-foreground transition-colors"
                onclick={askPermission}
              >Grant browser permission</button>
              <p class="text-[11px] text-muted-foreground/40">Toasts appear regardless. Browser notifications need this permission.</p>
            {:else if notifyPermission === "denied"}
              <p class="text-[11px] text-amber-400/80">Browser permission denied — toasts only. Re-enable in your browser's site settings.</p>
            {:else}
              <p class="text-[11px] text-muted-foreground/50">Browser permission granted. Toasts + system notifications.</p>
            {/if}
            <div class="space-y-1.5 pt-1">
              {#each [
                { kind: "deadline" as NotificationKind, label: "Deadline approaching (1h before)", muted: muteDeadline },
                { kind: "scheduled" as NotificationKind, label: "Scheduled time fires",            muted: muteScheduled },
                { kind: "recurring" as NotificationKind, label: "Recurring task rolled to next",    muted: muteRecurring },
              ] as opt}
                <label class="flex items-center gap-2 cursor-pointer text-[12px]">
                  <input
                    type="checkbox"
                    checked={!opt.muted}
                    onchange={() => toggleMute(opt.kind)}
                    class="accent-primary"
                  />
                  <span class={opt.muted ? "text-muted-foreground/40" : "text-foreground/80"}>{opt.label}</span>
                </label>
              {/each}
            </div>
          </div>
        {/if}
        <p class="text-[11px] text-muted-foreground/40 mt-1.5">Server scans deadlines every minute. Default lead: 1h before deadline, on-time for scheduled.</p>
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
        {#if lastSync}
          <div class="mt-2 text-[11px] text-muted-foreground/70 leading-relaxed">
            {#if lastSync.at}
              <div>
                Last synced <span class="text-foreground/80">{formatRelative(lastSync.at)}</span>
                {#if lastSync.trigger}
                  via <span class="text-foreground/80">{lastSync.trigger}</span>
                {/if}
                {#if summarizeOutcome(lastSync)}
                  · <span class={lastSync.error ? "text-red-400" : "text-foreground/80"}>{summarizeOutcome(lastSync)}</span>
                {/if}
              </div>
            {:else}
              <div>Has not synced yet — startup trigger fires 10s after server boot, then every 5 minutes.</div>
            {/if}
          </div>
        {/if}
        <p class="text-[11px] text-muted-foreground/40 mt-1.5">macOS only. Auto-syncs on server start, every 5 minutes, and 30 seconds after edits. Pulls changes from Reminders.app then pushes Tesela tasks with deadlines.</p>
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
