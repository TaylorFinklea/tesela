/**
 * Tiny toast store. One message at a time — replacing a live toast with
 * a new one is fine for the surfaces using this (sync outcomes, action
 * confirmations). Auto-dismisses after 4s; explicit dismiss available
 * via `clear()`.
 */

type ToastTone = "info" | "success" | "warn" | "error";

let current = $state<{ message: string; tone: ToastTone; id: number } | null>(null);
let nextId = 0;
let dismissTimer: ReturnType<typeof setTimeout> | null = null;

export function toast(message: string, tone: ToastTone = "info", durationMs = 4000) {
  if (dismissTimer) clearTimeout(dismissTimer);
  nextId += 1;
  current = { message, tone, id: nextId };
  if (durationMs > 0) {
    const id = nextId;
    dismissTimer = setTimeout(() => {
      if (current?.id === id) current = null;
      dismissTimer = null;
    }, durationMs);
  }
}

export function clearToast() {
  if (dismissTimer) {
    clearTimeout(dismissTimer);
    dismissTimer = null;
  }
  current = null;
}

export function getToast() {
  return current;
}
