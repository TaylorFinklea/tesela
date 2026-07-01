<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import QRCode from "qrcode-svg";
  import { api, ApiError } from "$lib/api-client";
  import type {
    SyncDeviceInfo,
    SyncPeer,
    SyncPeerStatus,
    SyncDiscoveredPeer,
    SyncPairingCode,
    SyncRecoveryPhrase,
  } from "$lib/api-client";

  // --- state -----------------------------------------------------------------

  let device = $state<SyncDeviceInfo | null>(null);
  let paired = $state<SyncPeer[]>([]);
  let statuses = $state<SyncPeerStatus[]>([]);
  let discovered = $state<SyncDiscoveredPeer[]>([]);
  let errorMsg = $state<string | null>(null);
  let syncingAll = $state(false);
  let lastSyncResult = $state<{ peers: number; appliedTotal: number; errors: string[]; at: number } | null>(null);
  let pollHandle: ReturnType<typeof setInterval> | null = null;

  // Manual-pair form. The full pairing-code flow lands in a follow-up; for
  // POC the operator can paste a device-id + url from the other side's
  // device card.
  let manualHex = $state("");
  let manualUrl = $state("");
  let manualName = $state("");
  let manualBusy = $state(false);

  // Pairing-code state — Phase 2.2. The local device can show its code on
  // demand (kept hidden by default because it's a secret), and the joining
  // device pastes a code into the textarea to one-shot pair.
  let pairingCode = $state<SyncPairingCode | null>(null);
  let pairingCodeRevealed = $state(false);
  let pairingCodeError = $state<string | null>(null);
  let pasteCode = $state("");
  let pasteBusy = $state(false);
  let pasteResultMsg = $state<string | null>(null);

  // Recovery phrase state — tesela-ra7 P0.3c. Show-side only: reveals the
  // current mosaic's 24-word BIP39 phrase (= the group key) behind an
  // explicit action, mirroring the pairing-code reveal above.
  let recoveryPhrase = $state<SyncRecoveryPhrase | null>(null);
  let recoveryPhraseRevealed = $state(false);
  let recoveryPhraseError = $state<string | null>(null);

  // --- helpers ---------------------------------------------------------------

  async function refresh() {
    try {
      const [d, p, s, disc] = await Promise.all([
        api.syncDevice(),
        api.syncListPeers(),
        api.syncStatus(),
        api.syncDiscovered(),
      ]);
      device = d;
      paired = p;
      statuses = s;
      discovered = disc;
      errorMsg = null;
    } catch (e) {
      errorMsg = e instanceof ApiError ? `${e.status} ${e.body}` : (e as Error).message;
    }
  }

  function statusFor(deviceIdHex: string): SyncPeerStatus | null {
    return statuses.find((s) => s.device_id_hex === deviceIdHex) ?? null;
  }

  function shortHex(hex: string): string {
    if (hex.length <= 12) return hex;
    return `${hex.slice(0, 6)}…${hex.slice(-4)}`;
  }

  function ntpToDate(ntp64: number | null): string {
    if (ntp64 == null) return "—";
    // uhlc stores its NTP64 as 32.32 fixed-point seconds-since-1970:
    // upper 32 bits are integer unix seconds, lower 32 are fractional.
    // JS numbers lose mantissa precision above 2^53 but the upper-32
    // divide still resolves the integer-seconds portion correctly.
    const unixSecs = Math.floor(ntp64 / 0x100000000);
    if (unixSecs <= 0) return "—";
    const ms = unixSecs * 1000;
    const ago = Date.now() - ms;
    if (ago < 0 || ago > 1000 * 60 * 60 * 24 * 365) {
      return new Date(ms).toLocaleString();
    }
    const s = Math.round(ago / 1000);
    if (s < 60) return `${s}s ago`;
    const m = Math.round(s / 60);
    if (m < 60) return `${m}m ago`;
    const h = Math.round(m / 60);
    return `${h}h ago`;
  }

  async function pair(d: SyncDiscoveredPeer) {
    try {
      await api.syncAddPeer({
        device_id_hex: d.device_id_hex,
        url: d.url,
        display_name: d.display_name,
      });
      await refresh();
    } catch (e) {
      errorMsg = `pair failed: ${e instanceof ApiError ? `${e.status} ${e.body}` : (e as Error).message}`;
    }
  }

  async function pairManual() {
    if (!manualHex.trim() || !manualUrl.trim()) return;
    manualBusy = true;
    try {
      await api.syncAddPeer({
        device_id_hex: manualHex.trim(),
        url: manualUrl.trim(),
        display_name: manualName.trim() || null,
      });
      manualHex = "";
      manualUrl = "";
      manualName = "";
      await refresh();
    } catch (e) {
      errorMsg = `pair failed: ${e instanceof ApiError ? `${e.status} ${e.body}` : (e as Error).message}`;
    } finally {
      manualBusy = false;
    }
  }

  async function unpair(deviceIdHex: string) {
    try {
      await api.syncRemovePeer(deviceIdHex);
      await refresh();
    } catch (e) {
      errorMsg = (e as Error).message;
    }
  }

  async function syncAll() {
    syncingAll = true;
    try {
      const r = await api.syncNow();
      const entries = Object.values(r.peers ?? {});
      let applied = 0;
      const errors: string[] = [];
      for (const e of entries) {
        if (typeof e.applied === "number") applied += e.applied;
        if (e.error) errors.push(e.error);
      }
      lastSyncResult = {
        peers: entries.length,
        appliedTotal: applied,
        errors,
        at: Date.now(),
      };
      await refresh();
    } catch (e) {
      errorMsg = `sync failed: ${e instanceof ApiError ? `${e.status} ${e.body}` : (e as Error).message}`;
    } finally {
      syncingAll = false;
    }
  }

  async function copyHex() {
    if (!device) return;
    try {
      await navigator.clipboard.writeText(device.device_id_hex);
    } catch {
      /* ignore */
    }
  }

  async function revealPairingCode() {
    pairingCodeError = null;
    try {
      pairingCode = await api.syncGetPairingCode();
      pairingCodeRevealed = true;
    } catch (e) {
      pairingCodeError =
        e instanceof ApiError ? `${e.status} ${e.body}` : (e as Error).message;
    }
  }

  function hidePairingCode() {
    pairingCodeRevealed = false;
  }

  async function copyPairingCode() {
    if (!pairingCode) return;
    try {
      await navigator.clipboard.writeText(pairingCode.code);
    } catch {
      /* ignore */
    }
  }

  async function revealRecoveryPhrase() {
    recoveryPhraseError = null;
    try {
      recoveryPhrase = await api.syncRecoveryPhrase();
      recoveryPhraseRevealed = true;
    } catch (e) {
      recoveryPhraseError =
        e instanceof ApiError ? `${e.status} ${e.body}` : (e as Error).message;
    }
  }

  function hideRecoveryPhrase() {
    recoveryPhraseRevealed = false;
  }

  async function copyRecoveryPhrase() {
    if (!recoveryPhrase) return;
    try {
      await navigator.clipboard.writeText(recoveryPhrase.phrase);
    } catch {
      /* ignore */
    }
  }

  async function pasteAndPair() {
    if (!pasteCode.trim()) return;
    pasteBusy = true;
    pasteResultMsg = null;
    try {
      const r = await api.syncPairWithCode(pasteCode.trim());
      pasteResultMsg = r.adopted_group
        ? `Paired with ${r.display_name || r.device_id_hex.slice(0, 8)} (adopted their group key).`
        : `Paired with ${r.display_name || r.device_id_hex.slice(0, 8)}.`;
      pasteCode = "";
      await refresh();
    } catch (e) {
      pasteResultMsg = null;
      errorMsg =
        e instanceof ApiError ? `${e.status} ${e.body}` : (e as Error).message;
    } finally {
      pasteBusy = false;
    }
  }

  const pairedHexes = $derived(new Set(paired.map((p) => p.device_id_hex)));
  const unpairedDiscovered = $derived(
    discovered.filter((d) => !pairedHexes.has(d.device_id_hex)),
  );

  // QR-encode the local pairing code so a phone can scan it directly.
  // Renders as inline SVG via qrcode-svg — no canvas, no async, no image
  // fetch. Falls back to empty string while the code isn't loaded.
  const pairingQrSvg = $derived(
    pairingCode
      ? new QRCode({
          content: pairingCode.code,
          padding: 2,
          width: 220,
          height: 220,
          color: "#0a0a0a",
          background: "#ffffff",
          ecl: "M",
          join: true,
        }).svg()
      : "",
  );

  const recoveryWords = $derived(
    recoveryPhrase ? recoveryPhrase.phrase.split(/\s+/) : [],
  );

  onMount(() => {
    refresh();
    pollHandle = setInterval(refresh, 5000);
  });
  onDestroy(() => {
    if (pollHandle) clearInterval(pollHandle);
  });
</script>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    This Device
  </h2>
  <div class="rounded-md border border-border/40 bg-muted/20 px-3 py-2.5">
    {#if device}
      <div class="text-[11px] text-muted-foreground/70 mb-1">Device ID</div>
      <div class="flex items-center gap-2">
        <code class="text-[11.5px] font-mono break-all flex-1">{device.device_id_hex}</code>
        <button
          type="button"
          class="text-[11px] px-2 py-1 rounded-md border border-border/40 text-muted-foreground hover:text-foreground hover:bg-muted/40"
          onclick={copyHex}
        >
          Copy
        </button>
      </div>
    {:else if errorMsg}
      <span class="text-[11px] text-red-500/80">Server unreachable</span>
    {:else}
      <span class="text-[11px] text-muted-foreground/60">Loading…</span>
    {/if}
  </div>
</section>

<section>
  <div class="flex items-center justify-between mb-3">
    <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest">
      LAN — Discovered
    </h2>
    <span class="text-[10.5px] text-muted-foreground/60">
      {discovered.length} seen · {unpairedDiscovered.length} unpaired
    </span>
  </div>
  {#if unpairedDiscovered.length === 0}
    <div class="rounded-md border border-dashed border-border/30 px-3 py-3 text-[11.5px] text-muted-foreground/60">
      No new devices on the LAN. Start another tesela-server with a different mosaic and it should appear here.
    </div>
  {:else}
    <div class="space-y-1.5">
      {#each unpairedDiscovered as d (d.device_id_hex)}
        <div class="flex items-center gap-3 rounded-md border border-border/40 px-3 py-2">
          <div class="flex-1 min-w-0">
            <div class="text-[12.5px] truncate">{d.display_name}</div>
            <div class="text-[10.5px] font-mono text-muted-foreground/60 truncate">
              {shortHex(d.device_id_hex)} · {d.url} · seen {d.last_seen_secs_ago}s ago
            </div>
          </div>
          <button
            type="button"
            class="text-[11px] px-2.5 py-1 rounded-md border border-primary/40 text-primary hover:bg-primary/10"
            onclick={() => pair(d)}
          >
            Pair
          </button>
        </div>
      {/each}
    </div>
  {/if}
</section>

<section>
  <div class="flex items-center justify-between mb-3">
    <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest">
      Paired
    </h2>
    {#if paired.length > 0}
      <button
        type="button"
        class="text-[11px] px-2 py-1 rounded-md border border-primary/40 text-primary hover:bg-primary/10 disabled:opacity-50"
        disabled={syncingAll}
        onclick={syncAll}
      >
        {syncingAll ? "Syncing…" : "Sync now"}
      </button>
    {/if}
  </div>
  {#if paired.length === 0}
    <div class="rounded-md border border-dashed border-border/30 px-3 py-3 text-[11.5px] text-muted-foreground/60">
      No paired devices yet. Pair one from the LAN list above, or enter its address manually below.
    </div>
  {:else}
    <div class="space-y-1.5">
      {#each paired as p (p.device_id_hex)}
        {@const status = statusFor(p.device_id_hex)}
        <div class="flex items-center gap-3 rounded-md border border-border/40 px-3 py-2">
          <div class="flex-1 min-w-0">
            <div class="text-[12.5px] truncate">{p.display_name ?? "Unnamed device"}</div>
            <div class="text-[10.5px] font-mono text-muted-foreground/60 truncate">
              {shortHex(p.device_id_hex)} · {p.url}
            </div>
            <div class="text-[10.5px] text-muted-foreground/60 mt-0.5">
              Last received: {ntpToDate(status?.peer_cursor_ntp ?? null)}
            </div>
          </div>
          <button
            type="button"
            class="text-[11px] px-2 py-1 rounded-md border border-border/40 text-muted-foreground hover:text-red-500 hover:border-red-500/30"
            onclick={() => unpair(p.device_id_hex)}
          >
            Remove
          </button>
        </div>
      {/each}
    </div>
  {/if}
  {#if lastSyncResult}
    <div class="mt-2 text-[10.5px] text-muted-foreground/60">
      Last manual sync: {lastSyncResult.peers} peer(s), {lastSyncResult.appliedTotal} op(s) received{#if lastSyncResult.errors.length > 0}, {lastSyncResult.errors.length} error(s){/if}
    </div>
    {#if lastSyncResult.errors.length > 0}
      <div class="mt-1 text-[10.5px] text-red-500/80 break-words">
        {lastSyncResult.errors[0]}
      </div>
    {/if}
  {/if}
</section>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Pair via code
  </h2>
  <p class="text-[11px] text-muted-foreground/70 mb-2">
    On the device you want to bring in, open <span class="font-mono">Settings → Devices</span>, click
    <em>Show pairing code</em>, copy the code, and paste it below. One step pairs the device and
    shares the group key.
  </p>
  <textarea
    placeholder="Paste pairing code…"
    bind:value={pasteCode}
    rows="3"
    class="w-full px-2.5 py-1.5 text-[11.5px] font-mono rounded-md border border-border/40 bg-background resize-none break-all"
  ></textarea>
  <div class="mt-2 flex items-center gap-2">
    <button
      type="button"
      class="text-[11.5px] px-3 py-1.5 rounded-md border border-primary/40 text-primary hover:bg-primary/10 disabled:opacity-50"
      disabled={pasteBusy || !pasteCode.trim()}
      onclick={pasteAndPair}
    >
      {pasteBusy ? "Pairing…" : "Pair via code"}
    </button>
    {#if pasteResultMsg}
      <span class="text-[10.5px] text-muted-foreground/80">{pasteResultMsg}</span>
    {/if}
  </div>
</section>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Your pairing code
  </h2>
  <p class="text-[11px] text-muted-foreground/70 mb-2">
    Treat this like a password: anyone with it can sync into your notes. Show it on this
    device, paste it on the joining device.
  </p>
  {#if !pairingCodeRevealed}
    <button
      type="button"
      class="text-[11.5px] px-3 py-1.5 rounded-md border border-border/40 text-foreground hover:bg-muted/40"
      onclick={revealPairingCode}
    >
      Show pairing code
    </button>
    {#if pairingCodeError}
      <div class="mt-2 text-[10.5px] text-red-500/90">{pairingCodeError}</div>
    {/if}
  {:else if pairingCode}
    <div class="rounded-md border border-amber-500/40 bg-amber-500/5 px-3 py-2.5">
      <div class="text-[10.5px] uppercase tracking-widest text-amber-500/80 mb-1">Pairing code</div>
      <div class="flex flex-col items-center gap-3 my-2">
        <div
          class="bg-white p-2 rounded-md shadow-sm"
          aria-label="Pairing code QR"
        >
          {@html pairingQrSvg}
        </div>
        {#if pairingCode.short_code}
          <div class="flex flex-col items-center gap-1">
            <code
              class="text-[22px] font-mono font-semibold tracking-[4px] text-amber-500/95"
              aria-label="6-character short code"
            >{pairingCode.short_code.slice(0, 3)} · {pairingCode.short_code.slice(3)}</code>
            <div class="text-[10.5px] text-muted-foreground/70 font-mono">
              6-char code · valid for {Math.round(pairingCode.short_code_expires_in_secs / 60)} min
            </div>
          </div>
        {/if}
        <div class="text-[10.5px] text-muted-foreground/70 font-mono">
          Point your phone's camera at the QR, or type the short code in the iOS app.
        </div>
      </div>
      <details class="text-[11px]">
        <summary class="cursor-pointer text-muted-foreground/70 select-none hover:text-foreground">
          Show raw code
        </summary>
        <code class="block mt-2 text-[11px] font-mono break-all leading-relaxed">{pairingCode.code}</code>
      </details>
      <div class="mt-3 flex items-center gap-2">
        <button
          type="button"
          class="text-[11px] px-2.5 py-1 rounded-md border border-border/40 text-foreground hover:bg-muted/40"
          onclick={copyPairingCode}
        >
          Copy code
        </button>
        <button
          type="button"
          class="text-[11px] px-2.5 py-1 rounded-md border border-border/40 text-muted-foreground hover:text-foreground"
          onclick={hidePairingCode}
        >
          Hide
        </button>
        <span class="text-[10.5px] text-muted-foreground/60">{pairingCode.url}</span>
      </div>
    </div>
  {/if}
</section>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Recovery phrase
  </h2>
  <p class="text-[11px] text-muted-foreground/70 mb-2">
    A 24-word phrase that losslessly encodes this mosaic's group key. Use it to recover sync
    on a fresh install if you lose every paired device.
  </p>
  {#if !recoveryPhraseRevealed}
    <button
      type="button"
      class="text-[11.5px] px-3 py-1.5 rounded-md border border-border/40 text-foreground hover:bg-muted/40"
      onclick={revealRecoveryPhrase}
    >
      Reveal recovery phrase
    </button>
    {#if recoveryPhraseError}
      <div class="mt-2 text-[10.5px] text-red-500/90">{recoveryPhraseError}</div>
    {/if}
  {:else if recoveryPhrase}
    <div class="rounded-md border border-amber-500/40 bg-amber-500/5 px-3 py-2.5">
      <div class="text-[10.5px] uppercase tracking-widest text-amber-500/80 mb-1">Recovery phrase</div>
      <p class="text-[11px] text-amber-500/90 mb-3 leading-relaxed">
        Write these down and keep them safe. Anyone with this phrase can read your mosaic —
        and we can't recover it for you.
      </p>
      <ol class="grid grid-cols-3 gap-x-3 gap-y-1.5 mb-3">
        {#each recoveryWords as word, i (i)}
          <li class="text-[11.5px] font-mono text-foreground/90 flex gap-1.5">
            <span class="text-muted-foreground/50 w-4 text-right shrink-0">{i + 1}.</span>
            <span>{word}</span>
          </li>
        {/each}
      </ol>
      <div class="flex items-center gap-2">
        <button
          type="button"
          class="text-[11px] px-2.5 py-1 rounded-md border border-border/40 text-foreground hover:bg-muted/40"
          onclick={copyRecoveryPhrase}
        >
          Copy phrase
        </button>
        <button
          type="button"
          class="text-[11px] px-2.5 py-1 rounded-md border border-border/40 text-muted-foreground hover:text-foreground"
          onclick={hideRecoveryPhrase}
        >
          Hide
        </button>
      </div>
    </div>
  {/if}
</section>

<section>
  <h2 class="text-[12px] font-medium text-muted-foreground/60 uppercase tracking-widest mb-3">
    Pair manually
  </h2>
  <p class="text-[11px] text-muted-foreground/70 mb-2">
    Fallback when you can't reach the pairing-code flow (e.g. pairing back to an older build).
    Both sides need to share the same group key already, otherwise sync envelopes won't match.
  </p>
  <div class="space-y-2">
    <input
      type="text"
      placeholder="device id (32-char hex)"
      bind:value={manualHex}
      class="w-full px-2.5 py-1.5 text-[12px] font-mono rounded-md border border-border/40 bg-background"
    />
    <input
      type="text"
      placeholder="http://host:7474"
      bind:value={manualUrl}
      class="w-full px-2.5 py-1.5 text-[12px] rounded-md border border-border/40 bg-background"
    />
    <input
      type="text"
      placeholder="display name (optional)"
      bind:value={manualName}
      class="w-full px-2.5 py-1.5 text-[12px] rounded-md border border-border/40 bg-background"
    />
    <button
      type="button"
      class="text-[11.5px] px-3 py-1.5 rounded-md border border-primary/40 text-primary hover:bg-primary/10 disabled:opacity-50"
      disabled={manualBusy || !manualHex.trim() || !manualUrl.trim()}
      onclick={pairManual}
    >
      {manualBusy ? "Pairing…" : "Add peer"}
    </button>
  </div>
</section>

{#if errorMsg}
  <section>
    <div class="rounded-md border border-red-500/30 bg-red-500/5 px-3 py-2 text-[11.5px] text-red-500/90">
      {errorMsg}
    </div>
  </section>
{/if}
