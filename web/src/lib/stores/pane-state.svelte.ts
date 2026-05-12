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
export type DrawerSide = "bottom" | "right";

const RATIO_KEY = "tesela:splitRatio";
// Phase 9.5b — bumped from `tesela:vSplitRatio` so existing values from the
// 9.5 toggle-vsplit (where ratio meant the right pane's %) don't carry over;
// in 9.5b ratio means the LEFT (back-context) pane's % and the default is 30.
const VSPLIT_RATIO_KEY = "tesela:vSplitRatio:v2";
const VIM_KEY = "tesela:vimEnabled";
const BOTTOM_OPEN_KEY = "tesela:bottomDrawerOpen";
const BOTTOM_TAB_KEY = "tesela:bottomDrawerTab";

const RAIL_OPEN_KEY = "tesela:railOpen";
const DRAWER_SIDE_KEY = "tesela:drawerSide";
const DRAWER_HEIGHT_KEY = "tesela:drawerHeight";
const DRAWER_WIDTH_KEY = "tesela:drawerWidth";

function loadRailOpen(): boolean {
  if (!browser) return true;
  try {
    const v = localStorage.getItem(RAIL_OPEN_KEY);
    return v === null ? true : v === "true";
  } catch {
    return true;
  }
}

function saveRailOpen(v: boolean): void {
  if (!browser) return;
  try {
    localStorage.setItem(RAIL_OPEN_KEY, String(v));
  } catch {
    // ignore
  }
}

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
  // Default 30 = left back-pane is 30% wide, right current-pane is 70%.
  if (!browser) return 30;
  try {
    const stored = localStorage.getItem(VSPLIT_RATIO_KEY);
    if (!stored) return 30;
    const n = Number(stored);
    if (Number.isFinite(n) && n >= 20 && n <= 80) return n;
    return 30;
  } catch {
    return 30;
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

function loadDrawerSide(): DrawerSide {
  if (!browser) return "bottom";
  try {
    const v = localStorage.getItem(DRAWER_SIDE_KEY);
    return v === "right" ? "right" : "bottom";
  } catch {
    return "bottom";
  }
}

function saveDrawerSide(v: DrawerSide): void {
  if (!browser) return;
  try {
    localStorage.setItem(DRAWER_SIDE_KEY, v);
  } catch {
    // ignore
  }
}

function loadNumber(key: string, fallback: number, min: number, max: number): number {
  if (!browser) return fallback;
  try {
    const v = localStorage.getItem(key);
    if (v === null) return fallback;
    const n = Number(v);
    if (Number.isFinite(n) && n >= min && n <= max) return n;
    return fallback;
  } catch {
    return fallback;
  }
}

function saveNumber(key: string, n: number): void {
  if (!browser) return;
  try {
    localStorage.setItem(key, String(n));
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
let railOpen = $state(loadRailOpen());
let drawerSide = $state<DrawerSide>(loadDrawerSide());
let drawerHeight = $state(loadNumber(DRAWER_HEIGHT_KEY, 220, 120, 600));
let drawerWidth = $state(loadNumber(DRAWER_WIDTH_KEY, 360, 240, 800));

// Phase 9.5b — column-view navigation. The split is open whenever the URL
// has `?back=<noteId>`; this store only carries the active side + ratio.
// All open/close transitions happen via URL navigation through
// `$lib/stores/active-pane-nav` — no toggle helpers here.
export type VSplitSide = "left" | "right";
let vSplitActiveSide = $state<VSplitSide>("right");
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
  // Kanban-mutex: if the column-view split is shown, drop ?back= via the
  // nav helper before opening kanban. Dynamic import keeps this module
  // free of $app/navigation (which can't run in SSR contexts).
  if (browser) {
    void import("$lib/stores/active-pane-nav.svelte").then((m) => m.collapseSplit());
  }
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

// ----- Phase 9.5b column-view split -----

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

// Route-driven auto-close (e.g. Settings): we remember the user's
// persisted preference and route-suppress without overwriting it.
// `forcedClosed` is in-memory only so a refresh + nav back returns to
// the user's last actual choice.
let forcedClosed = $state(false);

export function isBottomDrawerOpen(): boolean {
  if (forcedClosed) return false;
  return bottomDrawerOpen;
}

export function setBottomDrawerOpen(v: boolean) {
  // A user action releases the route-driven suppression. If they
  // explicitly want it open on Settings, honor that.
  forcedClosed = false;
  bottomDrawerOpen = v;
  saveBottomOpen(v);
  if (!v && activeRegion === "bottom") {
    activeRegion = "focus";
  }
}

export function toggleBottomDrawer() {
  setBottomDrawerOpen(!isBottomDrawerOpen());
}

/// Suppress the drawer while a route says it shouldn't be visible
/// (e.g. Settings). The user's persisted open/close preference is
/// preserved; we just hide it for the duration.
export function setDrawerRouteSuppressed(suppress: boolean) {
  if (forcedClosed === suppress) return;
  forcedClosed = suppress;
  if (suppress && activeRegion === "bottom") {
    activeRegion = "focus";
  }
}

export function getBottomTab(): BottomTab {
  return bottomTab;
}

export function setBottomTab(tab: BottomTab) {
  bottomTab = tab;
  saveBottomTab(tab);
}

export function isRailOpen(): boolean {
  return railOpen;
}

export function setRailOpen(v: boolean): void {
  railOpen = v;
  saveRailOpen(v);
  if (!v && activeRegion === "rail") {
    activeRegion = "focus";
  }
}

export function toggleRail(): void {
  setRailOpen(!railOpen);
}

export function getDrawerSide(): DrawerSide { return drawerSide; }
export function setDrawerSide(side: DrawerSide): void {
  drawerSide = side;
  saveDrawerSide(side);
}
export function toggleDrawerSide(): void {
  setDrawerSide(drawerSide === "bottom" ? "right" : "bottom");
}

export function getDrawerHeight(): number { return drawerHeight; }
export function setDrawerHeight(n: number): void {
  const clamped = Math.max(120, Math.min(600, n));
  drawerHeight = clamped;
  saveNumber(DRAWER_HEIGHT_KEY, clamped);
}

export function getDrawerWidth(): number { return drawerWidth; }
export function setDrawerWidth(n: number): void {
  const clamped = Math.max(240, Math.min(800, n));
  drawerWidth = clamped;
  saveNumber(DRAWER_WIDTH_KEY, clamped);
}
