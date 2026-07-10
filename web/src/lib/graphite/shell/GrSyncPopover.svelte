<script lang="ts">
  import type { RelayStatus } from "$lib/api-client";
  import { formatRelaySuccessAge, type SyncStatusTone } from "$lib/sync-health";

  let {
    tone,
    relay,
    statusError,
  }: {
    tone: SyncStatusTone;
    relay: RelayStatus | null;
    statusError: string | null;
  } = $props();

  const toneLabel = $derived(
    tone === "green" ? "Relay healthy" : tone === "amber" ? "Relay needs attention" : "Server disconnected",
  );
  const lastSuccess = $derived(formatRelaySuccessAge(relay));
</script>

<div class="gr-sync-popover" role="status" aria-live="polite">
  <div class="head">
    <span class="dot {tone}"></span>
    <span class="label">{toneLabel}</span>
  </div>

  {#if relay?.configured}
    <div class="url" title={relay.url ?? undefined}>{relay.url ?? "Relay configured"}</div>
    <dl>
      <div>
        <dt>Last success</dt>
        <dd>{lastSuccess}</dd>
      </div>
      <div>
        <dt>Last poll</dt>
        <dd>{formatRelaySuccessAge({ ...relay, last_put_at: null })}</dd>
      </div>
    </dl>
    {#if relay.last_error}
      <p class="error"><strong>Relay error:</strong> {relay.last_error}</p>
    {/if}
  {:else if relay}
    <p class="hint">No relay configured — sync is LAN-only.</p>
  {:else}
    <p class="hint">Waiting for relay status…</p>
  {/if}

  {#if statusError}
    <p class="error"><strong>Status request:</strong> {statusError}</p>
  {/if}
</div>

<style>
  .gr-sync-popover {
    position: absolute;
    top: 38px;
    right: 0;
    width: 272px;
    padding: 12px 14px;
    border: 1px solid var(--line-2);
    border-radius: 10px;
    background: var(--surface);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.28);
    color: var(--fg2);
    font-family: var(--sans);
    font-size: 12px;
    z-index: 40;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 9px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  .dot.green {
    background: var(--query);
    box-shadow: 0 0 0 3px rgba(133, 188, 99, 0.16);
  }
  .dot.amber {
    background: #d9a441;
    box-shadow: 0 0 0 3px rgba(217, 164, 65, 0.15);
  }
  .dot.red {
    background: var(--coral);
    box-shadow: 0 0 0 3px rgba(224, 122, 95, 0.14);
  }
  .label {
    font-weight: 600;
  }
  .url {
    overflow: hidden;
    color: var(--subtle);
    font-family: var(--mono);
    font-size: 10.5px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  dl {
    display: grid;
    gap: 4px;
    margin: 10px 0 0;
  }
  dl div {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  dt {
    color: var(--subtle);
  }
  dd {
    margin: 0;
    color: var(--fg2);
    font-family: var(--mono);
    font-size: 11px;
  }
  .hint,
  .error {
    margin: 8px 0 0;
    line-height: 1.45;
  }
  .hint {
    color: var(--subtle);
  }
  .error {
    color: var(--coral);
    overflow-wrap: anywhere;
  }
  .error strong {
    font-weight: 600;
  }
</style>
