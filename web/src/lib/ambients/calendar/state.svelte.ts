/**
 * Workspace-level state for the `calendar` ambient buffer.
 *
 * Per the v5 spec, ambient state survives unmounting of every pane that
 * renders it. The same calendar in tab A and tab B shares one
 * `selectedDate` and `viewMonth` value.
 */

function todayISO(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

function todayMonth(): string {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}

let selectedDate = $state<string>(todayISO());
let viewMonth = $state<string>(todayMonth());

export function getSelectedDate(): string {
  return selectedDate;
}
export function setSelectedDate(d: string): void {
  selectedDate = d;
}
export function getViewMonth(): string {
  return viewMonth;
}
export function setViewMonth(m: string): void {
  viewMonth = m;
}
