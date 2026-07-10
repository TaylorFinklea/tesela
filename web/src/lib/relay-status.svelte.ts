import { api, type RelayStatus } from "$lib/api-client";

const RELAY_STATUS_POLL_MS = 30_000;

let relayStatus = $state<RelayStatus | null>(null);
let relayStatusError = $state<string | null>(null);
let pollOwners = 0;
let pollTimer: ReturnType<typeof setInterval> | null = null;

export function getRelayStatus(): RelayStatus | null {
  return relayStatus;
}

export function getRelayStatusError(): string | null {
  return relayStatusError;
}

export async function refreshRelayStatus(): Promise<void> {
  try {
    relayStatus = await api.syncRelayStatus();
    relayStatusError = null;
  } catch (error) {
    relayStatusError = error instanceof Error ? error.message : String(error);
  }
}

function clearPollTimer() {
  if (pollTimer !== null) {
    clearInterval(pollTimer);
    pollTimer = null;
  }
}

function armPollTimer() {
  clearPollTimer();
  if (typeof document === "undefined" || document.hidden) return;
  pollTimer = setInterval(() => void refreshRelayStatus(), RELAY_STATUS_POLL_MS);
}

function onVisibilityChange() {
  if (document.hidden) {
    clearPollTimer();
    return;
  }
  void refreshRelayStatus();
  armPollTimer();
}

export function startRelayStatusPolling(): () => void {
  if (typeof document === "undefined") return () => {};

  pollOwners += 1;
  if (pollOwners === 1) {
    document.addEventListener("visibilitychange", onVisibilityChange);
    void refreshRelayStatus();
    armPollTimer();
  }

  let active = true;
  return () => {
    if (!active) return;
    active = false;
    pollOwners -= 1;
    if (pollOwners === 0) {
      clearPollTimer();
      document.removeEventListener("visibilitychange", onVisibilityChange);
    }
  };
}
