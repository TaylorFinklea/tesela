/**
 * Save state indicator — tracks whether a save is in progress, succeeded, or failed.
 */

export type SaveStatus = "idle" | "saving" | "saved" | "error";

let status = $state<SaveStatus>("idle");
let errorMessage = $state("");
let clearTimer: ReturnType<typeof setTimeout> | null = null;

export function getSaveStatus(): SaveStatus {
  return status;
}

export function getSaveError(): string {
  return errorMessage;
}

export function setSaving() {
  status = "saving";
  errorMessage = "";
  if (clearTimer) clearTimeout(clearTimer);
}

export function setSaved() {
  status = "saved";
  errorMessage = "";
  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = setTimeout(() => {
    status = "idle";
  }, 2000);
}

export function setSaveError(msg: string) {
  status = "error";
  errorMessage = msg;
  if (clearTimer) clearTimeout(clearTimer);
  // Don't auto-clear errors — user needs to see them
}
