/**
 * Pane state — Vim-style window management.
 *
 * Two layers:
 *   • `activeRegion` — which of the four top-level layout regions has focus
 *     (rail / middle / focus / bottom drawer). Driven by `Ctrl+w h/l/j/k`.
 *   • `activePane` — sub-state of the focus region. Today only `outliner`
 *     vs `kanban` (when the kanban split is open inside focus).
 *
 * Transient session state for everything except `splitRatio` and
 * `bottomDrawerOpen` / `bottomTab`, which persist to localStorage.
 */
import { browser } from "$app/environment";

export type Region = "rail" | "middle" | "focus" | "bottom";
export type MainPane = "outliner" | "kanban";
export type BottomTab = "backlinks" | "properties" | "outline" | "history" | "linkedTasks";

const RATIO_KEY = "tesela:splitRatio";
const VSPLIT_RATIO_KEY = "tesela:vSplitRatio";
const VIM_KEY = "tesela:vimEnabled";
const BOTTOM_OPEN_KEY = "tesela:bottomDrawerOpen";
const BOTTOM_TAB_KEY = "tesela:bottomDrawerTab";

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

function loadVSplitRatio(): number {
  if (!browser) return 50;
  try {
    const stored = localStorage.getItem(VSPLIT_RATIO_KEY);
    if (!stored) return 50;
    const n = Number(stored);
    if (Number.isFinite(n) && n >= 20 && n <= 80) return n;
    return 50;
  } catch {
    return 50;
  }
}

function saveVSplitRatio(n: number) {
  if (!browser) return;
  try {
    localStorage.setItem(VSPLIT_RATIO_KEY, String(n));
  } catch {
    // ignore
  }
}

function loadBottomOpen(): boolean {
  if (!browser) return true;
  try {
    const stored = localStorage.getItem(BOTTOM_OPEN_KEY);
    if (stored === null) return true;
    return stored === "true";
  } catch {
    return true;
  }
}

function saveBottomOpen(v: boolean) {
  if (!browser) return;
  try {
    localStorage.setItem(BOTTOM_OPEN_KEY, String(v));
  } catch {
    // ignore
  }
}

const VALID_TABS: ReadonlySet<BottomTab> = new Set([
  "backlinks",
  "properties",
  "outline",
  "history",
  "linkedTasks",
]);

function loadBottomTab(): BottomTab {
  if (!browser) return "backlinks";
  try {
    const stored = localStorage.getItem(BOTTOM_TAB_KEY);
    if (stored && VALID_TABS.has(stored as BottomTab)) return stored as BottomTab;
    return "backlinks";
  } catch {
    return "backlinks";
  }
}

function saveBottomTab(tab: BottomTab) {
  if (!browser) return;
  try {
    localStorage.setItem(BOTTOM_TAB_KEY, tab);
  } catch {
    // ignore
  }
}

let splitOpen = $state(false);
let activeRegion = $state<Region>("focus");
let activePane = $state<MainPane>("outliner");
let splitRatio = $state(loadRatio());
let ctrlWPending = $state(false);
let vimMode = $state("NORMAL");
let bottomDrawerOpen = $state(loadBottomOpen());
let bottomTab = $state<BottomTab>(loadBottomTab());

// Phase 9.5 — vertical (side-by-side) split inside the focus region. Mutex with
// the kanban (horizontal) split: opening one closes the other. When open, two
// BlockOutliner instances render side-by-side in the focus pane; each side has
// independent note + drill state, with the right pane's note tracked via URL
// query params.
export type VSplitSide = "left" | "right";
let vSplitOpen = $state(false);
let vSplitActiveSide = $state<VSplitSide>("left");
let vSplitRatio = $state(loadVSplitRatio());

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
  // Mutex: kanban split and vertical split can't be open simultaneously.
  if (vSplitOpen) closeVSplit();
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
  else openSplit();
}

// ----- Phase 9.5 vertical split -----

export function isVSplitOpen(): boolean {
  return vSplitOpen;
}

export function getVSplitActiveSide(): VSplitSide {
  return vSplitActiveSide;
}

export function setVSplitActiveSide(side: VSplitSide) {
  vSplitActiveSide = side;
  // Switching sides keeps focus in the focus region.
  activeRegion = "focus";
}

export function getVSplitRatio(): number {
  return vSplitRatio;
}

export function setVSplitRatio(n: number) {
  const next = Math.max(20, Math.min(80, n));
  vSplitRatio = next;
  saveVSplitRatio(next);
}

export function adjustVSplitRatio(delta: number) {
  setVSplitRatio(vSplitRatio + delta);
}

export function openVSplit() {
  // Mutex with kanban split.
  if (splitOpen) closeSplit();
  vSplitOpen = true;
}

export function closeVSplit() {
  vSplitOpen = false;
  vSplitActiveSide = "left";
}

export function toggleVSplit() {
  if (vSplitOpen) closeVSplit();
  else openVSplit();
}

export function setActiveRegion(r: Region) {
  activeRegion = r;
  if (r !== "focus") {
    releaseEditorFocus();
  } else if (browser) {
    // Returning to focus — ask the outliner to refocus its currently
    // focused block's cm-editor. BlockOutliner listens for this.
    document.dispatchEvent(new CustomEvent("tesela:restore-focus"));
  }
}

export function setActivePane(pane: MainPane) {
  activeRegion = "focus";
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

export function isBottomDrawerOpen(): boolean {
  return bottomDrawerOpen;
}

export function setBottomDrawerOpen(v: boolean) {
  bottomDrawerOpen = v;
  saveBottomOpen(v);
  if (!v && activeRegion === "bottom") {
    activeRegion = "focus";
  }
}

export function toggleBottomDrawer() {
  setBottomDrawerOpen(!bottomDrawerOpen);
}

export function getBottomTab(): BottomTab {
  return bottomTab;
}

export function setBottomTab(tab: BottomTab) {
  bottomTab = tab;
  saveBottomTab(tab);
}
