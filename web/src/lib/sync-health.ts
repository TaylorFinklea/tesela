export type SyncStatusTone = "green" | "amber" | "red";

export interface RelayHealthSnapshot {
  configured: boolean;
  last_poll_at: number | null;
  last_put_at: number | null;
  last_error: string | null;
}

export const RELAY_STALE_AFTER_MS = 120_000;

function latestRelaySuccessAt(relay: RelayHealthSnapshot): number | null {
  const timestamps = [relay.last_poll_at, relay.last_put_at].filter(
    (timestamp): timestamp is number => timestamp != null && Number.isFinite(timestamp),
  );
  return timestamps.length > 0 ? Math.max(...timestamps) : null;
}

export function blendSyncStatus(
  wsConnected: boolean,
  relay: RelayHealthSnapshot | null,
  nowMs = Date.now(),
  statusError: string | null = null,
): SyncStatusTone {
  if (!wsConnected) return "red";
  if (!relay?.configured || statusError || relay.last_error?.trim()) return "amber";

  const latestSuccessAt = latestRelaySuccessAt(relay);
  if (
    latestSuccessAt == null ||
    nowMs - latestSuccessAt * 1_000 > RELAY_STALE_AFTER_MS
  ) {
    return "amber";
  }
  return "green";
}

export function formatRelaySuccessAge(
  relay: RelayHealthSnapshot | null,
  nowMs = Date.now(),
): string {
  if (!relay) return "never";
  const latestSuccessAt = latestRelaySuccessAt(relay);
  if (latestSuccessAt == null) return "never";

  const ageSeconds = Math.max(0, Math.round(nowMs / 1_000 - latestSuccessAt));
  if (ageSeconds < 60) return `${ageSeconds}s ago`;
  const ageMinutes = Math.round(ageSeconds / 60);
  if (ageMinutes < 60) return `${ageMinutes}m ago`;
  const ageHours = Math.round(ageMinutes / 60);
  if (ageHours < 24) return `${ageHours}h ago`;
  return `${Math.round(ageHours / 24)}d ago`;
}
