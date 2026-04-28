/**
 * Pane state — Vim-style window management.
 *
 * Two layers:
 *   • `activeRegion` — which of the three top-level layout regions has focus
 *     (left sidebar, main content, right panel). Driven by `Ctrl+w h/l`.
 *   • `activePane` — sub-state of the main region. Today only `outliner` vs
 *     `kanban` (when the kanban split is open). Driven by `Ctrl+w j/k`.
 *
 * Transient session state for everything except `splitRatio`, which persists
 * to localStorage so a user's preferred kanban-split sizing survives reloads.
 */
import { browser } from "$app/environment";

export type Region = "left" | "main" | "right";
export type MainPane = "outliner" | "kanban";

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
let activeRegion = $state<Region>("main");
let activePane = $state<MainPane>("outliner");
let splitRatio = $state(loadRatio());
let ctrlWPending = $state(false);
let vimMode = $state("NORMAL");

/** Blur any focused cm-editor so cm-vim stops eating keys when we move
 *  region focus elsewhere. Safe to call from any region transition. */
function releaseEditorFocus() {
  if (!browser) return;
  const active = document.activeElement as HTMLElement | null;
  if (active && active.closest(".cm-editor")) active.blur();
}

export function isSplitOpen(): boolean {
  return splitOpen;
}

export function getActiveRegion(): Region {
  return activeRegion;
}

export function getActivePane(): MainPane {
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
  // Reset the sub-state but preserve activeRegion — closing the split
  // shouldn't yank focus across regions.
  activePane = "outliner";
}

export function toggleSplit() {
  if (splitOpen) closeSplit();
  else splitOpen = true;
}

export function setActiveRegion(r: Region) {
  activeRegion = r;
  if (r !== "main") {
    releaseEditorFocus();
  } else if (browser) {
    // Returning to main — ask the outliner to refocus its currently
    // focused block's cm-editor. BlockOutliner listens for this.
    document.dispatchEvent(new CustomEvent("tesela:restore-focus"));
  }
}

export function setActivePane(pane: MainPane) {
  activeRegion = "main";
  activePane = pane;
  if (pane === "kanban") releaseEditorFocus();
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

export function getVimMode(): string {
  return vimMode;
}

export function setVimMode(mode: string) {
  vimMode = mode.toUpperCase();
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
