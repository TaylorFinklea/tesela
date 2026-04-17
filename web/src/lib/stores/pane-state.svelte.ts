/**
 * Split pane state — Vim-style window management.
 * Transient session state for split open/focus; splitRatio persists to localStorage.
 */
import { browser } from "$app/environment";

type ActivePane = "outliner" | "kanban";

const RATIO_KEY = "tesela:splitRatio";
const VIM_KEY = "tesela:vimEnabled";

function loadRatio(): number {
  if (!browser) return 60;
  try {
    const stored = localStorage.getItem(RATIO_KEY);
    if (!stored) return 60;
    const n = Number(stored);
    if (Number.isFinite(n) && n >= 20 && n <= 80) return n;
    return 60;
  } catch {
    return 60;
  }
}

function saveRatio(n: number) {
  if (!browser) return;
  try {
    localStorage.setItem(RATIO_KEY, String(n));
  } catch {
    // ignore
  }
}

let splitOpen = $state(false);
let activePane = $state<ActivePane>("outliner");
let splitRatio = $state(loadRatio());
let ctrlWPending = $state(false);

export function isSplitOpen(): boolean {
  return splitOpen;
}

export function getActivePane(): ActivePane {
  return activePane;
}

export function getSplitRatio(): number {
  return splitRatio;
}

export function isCtrlWPending(): boolean {
  return ctrlWPending;
}

export function openSplit() {
  splitOpen = true;
}

export function closeSplit() {
  splitOpen = false;
  activePane = "outliner";
}

export function toggleSplit() {
  if (splitOpen) closeSplit();
  else splitOpen = true;
}

export function setActivePane(pane: ActivePane) {
  activePane = pane;
  // Release CM6 focus when moving to kanban so j/k reach the kanban handler
  if (pane === "kanban" && browser) {
    const active = document.activeElement as HTMLElement | null;
    if (active && active.closest(".cm-editor")) active.blur();
  }
}

export function adjustSplitRatio(delta: number) {
  const next = Math.max(20, Math.min(80, splitRatio + delta));
  splitRatio = next;
  saveRatio(next);
}

export function setSplitRatio(n: number) {
  const next = Math.max(20, Math.min(80, n));
  splitRatio = next;
  saveRatio(next);
}

export function setCtrlWPending(v: boolean) {
  ctrlWPending = v;
}

export function isVimEnabled(): boolean {
  if (!browser) return true;
  try {
    const stored = localStorage.getItem(VIM_KEY);
    return stored === null ? true : stored === "true";
  } catch {
    return true;
  }
}
