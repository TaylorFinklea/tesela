<script lang="ts">
  import { onMount } from "svelte";
  import { useQueryClient } from "@tanstack/svelte-query";
  import { runRemindersSync } from "$lib/reminders-sync";
  import { api, type RemindersLastSync, type RelayStatus } from "$lib/api-client";
  import {
    isEnabled as isNotifyEnabled,
    setEnabled as setNotifyEnabled,
    isMuted as isKindMuted,
    setMuted as setKindMuted,
    requestPermission as requestNotifyPermission,
    permissionState,
    type NotificationKind,
  } from "$lib/notifications";

  const queryClient = useQueryClient();

  // Phase 12.3 — notification toggles. Permission state is read on mount
  // (it isn't reactive across permission changes; user has to refresh
  // settings if they revoke browser-side, which is rare).
  let notifyEnabled = $state(isNotifyEnabled());
  let notifyPermission = $state<NotificationPermission>("default");
  let muteDeadline = $state(isKindMuted("deadline"));
  let muteScheduled = $state(isKindMuted("scheduled"));
  let muteRecurring = $state(isKindMuted("recurring"));

  let syncing = $state(false);
  let lastSync = $state<RemindersLastSync | null>(null);

  // Relay status — polled every 5s. `null` while the first request
  // is in flight; `{ configured: false }` means LAN-only.
  let relay = $state<RelayStatus | null>(null);
  async function refreshRelayStatus() {
    try {
      relay = await api.syncRelayStatus();
    } catch {
      relay = null;
    }
  }
  function formatRelativeSecs(secs: number | null): string {
    if (secs == null) return "never";
    const ageSec = Math.max(0, Math.round(Date.now() / 1000 - secs));
    if (ageSec < 60) return `${ageSec}s ago`;
    const min = Math.round(ageSec / 60);
    if (min < 60) return `${min}m ago`;
    const hr = Math.round(min / 60);
    if (hr < 24) return `${hr}h ago`;
    return new Date(secs * 1000).toLocaleString();
  }

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
    void refreshRelayStatus();
    notifyPermission = permissionState();
    const id = setInterval(refreshLastSync, 15_000);
    const relayId = setInterval(refreshRelayStatus, 5_000);
    return () => {
      clearInterval(id);
      clearInterval(relayId);
    };
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
</script>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">WAN Relay</h2>
  {#if relay == null}
    <p class="text-[11px] text-muted-foreground/40">Loading status…</p>
  {:else if !relay.configured}
    <div class="text-[12px] text-muted-foreground/80 leading-relaxed space-y-2">
      <p>
        No relay is configured for this mosaic — sync is LAN-only.
        Add a <code class="text-foreground/80">[sync.relay]</code> block
        to <code class="text-foreground/80">.tesela/config.toml</code>
        and restart the server to enable cross-network sync:
      </p>
      <pre class="bg-muted/30 border border-border/40 rounded p-2 text-[11px] font-mono overflow-x-auto">{`[sync.relay]
url = "https://relay.example.com"
# Optional. Default is 5000 (5 s).
poll_interval_ms = 5000`}</pre>
      <p class="text-[11px] text-muted-foreground/50">
        Deploy a relay first — see
        <code class="text-foreground/70">crates/tesela-relay/DEPLOY.md</code>
        in the repo for the Docker recipe + Cloudflare Tunnel setup.
      </p>
    </div>
  {:else}
    <div class="text-[12px] text-foreground/85 leading-relaxed space-y-2">
      <div class="flex items-center gap-2">
        <span
          class="inline-block w-2 h-2 rounded-full {relay.last_error ? 'bg-red-400' : relay.last_poll_at ? 'bg-emerald-400' : 'bg-amber-400'}"
          aria-hidden="true"
        ></span>
        <span class="font-mono text-[11px] text-muted-foreground/70 break-all">{relay.url}</span>
      </div>
      {#if relay.last_error}
        <div class="text-[11px] text-red-400 bg-red-950/30 border border-red-900/40 rounded px-2 py-1.5">
          <span class="font-medium">Error:</span> {relay.last_error}
        </div>
      {/if}
      <dl class="grid grid-cols-[max-content_1fr] gap-x-3 gap-y-1 text-[11px] text-muted-foreground/70">
        <dt class="text-muted-foreground/50">Registered</dt>
        <dd class="text-foreground/80">{formatRelativeSecs(relay.registered_at)}</dd>
        <dt class="text-muted-foreground/50">Last poll</dt>
        <dd class="text-foreground/80">{formatRelativeSecs(relay.last_poll_at)}</dd>
        <dt class="text-muted-foreground/50">Last put</dt>
        <dd class="text-foreground/80">{formatRelativeSecs(relay.last_put_at)}</dd>
        <dt class="text-muted-foreground/50">Inbound cursor</dt>
        <dd class="font-mono text-foreground/80">seq {relay.inbound_cursor}</dd>
        <dt class="text-muted-foreground/50">Outbound cursor</dt>
        <dd class="font-mono text-foreground/80">{relay.outbound_cursor_ntp ?? "—"}</dd>
      </dl>
      <p class="text-[11px] text-muted-foreground/40">
        Zero-knowledge fanout — the relay sees opaque AEAD-sealed
        payloads only. Pairing codes generated from this device carry
        the relay URL automatically so joining devices auto-configure.
      </p>
    </div>
  {/if}
</section>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">Notifications</h2>
  <label class="flex items-center gap-3 cursor-pointer">
    <button
      class="w-9 h-5 rounded-full transition-colors {notifyEnabled ? 'bg-primary' : 'bg-muted'}"
      onclick={toggleNotifyEnabled}
      aria-label="Toggle desktop notifications"
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
          { kind: "scheduled" as NotificationKind, label: "Scheduled time fires", muted: muteScheduled },
          { kind: "recurring" as NotificationKind, label: "Recurring task rolled to next", muted: muteRecurring },
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
