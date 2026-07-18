<script lang="ts">
  import { onMount } from "svelte";
  import { commandRegistry } from "$lib/command-registry.svelte";
  import { getRelayStatus, getRelayStatusError, startRelayStatusPolling } from "$lib/relay-status.svelte";
  import { blendSyncStatus, formatRelaySuccessAge } from "$lib/sync-health";
  import { getConnected } from "$lib/ws-client.svelte";

  const relay = $derived(getRelayStatus());
  const statusError = $derived(getRelayStatusError());
  const tone = $derived(blendSyncStatus(getConnected(), relay, Date.now(), statusError));
  const label = $derived(tone === "green" ? "Healthy" : tone === "amber" ? "Needs attention" : "Disconnected");
  const lastSuccess = $derived(formatRelaySuccessAge(relay));

  onMount(() => startRelayStatusPolling());
</script>

<div class="summary" role="status" aria-live="polite">
  <div class="line"><span class="dot {tone}"></span><strong>{label}</strong></div>
  {#if !relay && !statusError}
    <div class="detail">Loading relay status…</div>
  {:else if statusError}
    <div class="detail error">{statusError}</div>
  {:else if relay?.last_error}
    <div class="detail error">{relay.last_error}</div>
  {:else}
    <div class="detail">Last relay success · {lastSuccess}</div>
    <div class="detail">Inbound {relay?.inbound_cursor ?? 0} · outbound {relay?.outbound_cursor_ntp ?? "none"}</div>
  {/if}
  <button
    type="button"
    data-rail-action=""
    data-command-id="settings-sync"
    onclick={() => void commandRegistry.get("settings-sync")?.run()}
  >Open Sync settings</button>
</div>

<style>
  .summary { display:flex; flex-direction:column; gap:5px; padding:5px 8px 1px; }
  .line { display:flex; align-items:center; gap:7px; color:var(--fg2); font-size:12px; }
  .dot { width:7px; height:7px; border-radius:50%; background:var(--faint); }
  .dot.green { background:#4caf78; } .dot.amber { background:#d9a441; } .dot.red { background:var(--coral); }
  .detail { font:10px var(--mono); color:var(--faint); overflow-wrap:anywhere; }
  .detail.error { color:var(--coral); }
  button { align-self:flex-start; padding:3px 0; border:0; background:transparent; color:var(--subtle); font:10px var(--mono); cursor:pointer; }
  button:hover { color:var(--fg2); }
</style>
