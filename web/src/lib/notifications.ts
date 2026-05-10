/**
 * Phase 12.3 — desktop notification dispatch.
 *
 * Subscribes to the WS events from `ws-client.svelte.ts` and surfaces a
 * `Notification` (when permission is granted) plus an in-app toast (always).
 * Permission is requested lazily on the first event after the user enables
 * the toggle in Settings, so we don't pop a permission prompt at boot.
 */
import { goto } from "$app/navigation";
import { toast } from "$lib/stores/toast.svelte";
import type {
  DeadlineApproachingEvent,
  ScheduledFiresEvent,
  RecurringRolledEvent,
} from "$lib/ws-client.svelte";

const ENABLED_KEY = "tesela:notifications:enabled";
const MUTED_KEY = "tesela:notifications:muted"; // CSV of "deadline,scheduled,recurring"

export function isEnabled(): boolean {
  if (typeof window === "undefined") return false;
  // Default ON — first-time users see notifications. They can toggle off
  // in Settings if they don't want them.
  return localStorage.getItem(ENABLED_KEY) !== "false";
}

export function setEnabled(value: boolean) {
  if (typeof window === "undefined") return;
  localStorage.setItem(ENABLED_KEY, value ? "true" : "false");
}

export type NotificationKind = "deadline" | "scheduled" | "recurring";

export function isMuted(kind: NotificationKind): boolean {
  if (typeof window === "undefined") return false;
  const csv = localStorage.getItem(MUTED_KEY) ?? "";
  return csv.split(",").map((s) => s.trim()).includes(kind);
}

export function setMuted(kind: NotificationKind, muted: boolean) {
  if (typeof window === "undefined") return;
  const csv = localStorage.getItem(MUTED_KEY) ?? "";
  const set = new Set(csv.split(",").map((s) => s.trim()).filter(Boolean));
  if (muted) set.add(kind);
  else set.delete(kind);
  localStorage.setItem(MUTED_KEY, [...set].join(","));
}

/** Trigger the browser permission prompt. Returns the resulting state. */
export async function requestPermission(): Promise<NotificationPermission> {
  if (typeof Notification === "undefined") return "denied";
  if (Notification.permission !== "default") return Notification.permission;
  return await Notification.requestPermission();
}

export function permissionState(): NotificationPermission {
  if (typeof Notification === "undefined") return "denied";
  return Notification.permission;
}

function show(title: string, body: string, noteId: string) {
  toast(`${title} — ${body}`, "info", 6000);
  if (
    typeof Notification === "undefined" ||
    Notification.permission !== "granted"
  ) {
    return;
  }
  // Use noteId as the tag so back-to-back fires for the same task collapse
  // into one toast in macOS Notification Center.
  const n = new Notification(title, { body, tag: noteId, silent: false });
  n.onclick = () => {
    window.focus();
    void goto(`/p/${encodeURIComponent(noteId)}`);
    n.close();
  };
}

export function handleDeadlineApproaching(e: DeadlineApproachingEvent) {
  if (!isEnabled() || isMuted("deadline")) return;
  const when = formatTime(e.deadline_iso);
  const lead = e.lead_minutes >= 60
    ? `${Math.round(e.lead_minutes / 60)}h`
    : `${e.lead_minutes}m`;
  show(`Due in ${lead}: ${e.title}`, `Deadline ${when}`, e.note_id);
}

export function handleScheduledFires(e: ScheduledFiresEvent) {
  if (!isEnabled() || isMuted("scheduled")) return;
  const when = formatTime(e.scheduled_iso);
  show(`Scheduled now: ${e.title}`, `${when}`, e.note_id);
}

export function handleRecurringRolled(e: RecurringRolledEvent) {
  if (!isEnabled() || isMuted("recurring")) return;
  show(
    `Rolled to next: ${e.title}`,
    `Next due ${formatDate(e.next_deadline)}`,
    e.note_id,
  );
}

function formatTime(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: "short",
      day: "numeric",
      hour: "numeric",
      minute: "2-digit",
    });
  } catch {
    return iso;
  }
}

function formatDate(s: string): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(s)) return s;
  try {
    return new Date(s + "T00:00:00").toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
    });
  } catch {
    return s;
  }
}
