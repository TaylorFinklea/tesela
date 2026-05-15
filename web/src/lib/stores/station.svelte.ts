/**
 * Prism v4 — Command Station modal state.
 *
 * The Station is the ⌘K-opened 50/50 modal: Palette on the left, Dashboard
 * on the right, with tabs (⌘1 Palette · ⌘2 Dashboard · ⌘3 AI · ⌘4 History)
 * cycling between full-bleed views when the user wants more room.
 *
 * State here is intentionally minimal: open/close, active tab, and the id of
 * the pane that was focused right before opening (so Esc can restore focus
 * to the correct shell rather than the body). Everything else — the palette
 * search query, dashboard widget choices, selected command — is local to
 * the Station component.
 */

export type StationTab = "palette" | "dashboard" | "ai" | "history";

let open = $state(false);
let activeTab = $state<StationTab>("palette");
let priorPaneId = $state<string | undefined>(undefined);
/** Optional seed for the palette search input when the Station opens.
 *  Set by the top-bar command field so typing there flows into the modal. */
let initialQuery = $state<string>("");

export function isStationOpen(): boolean {
  return open;
}

export function getStationTab(): StationTab {
  return activeTab;
}

export function setStationTab(tab: StationTab) {
  activeTab = tab;
}

export function getStationPriorPaneId(): string | undefined {
  return priorPaneId;
}

export function getStationInitialQuery(): string {
  return initialQuery;
}

export function openStation(opts?: { tab?: StationTab; query?: string; priorPaneId?: string }) {
  activeTab = opts?.tab ?? "palette";
  initialQuery = opts?.query ?? "";
  priorPaneId = opts?.priorPaneId;
  open = true;
}

export function closeStation() {
  open = false;
  initialQuery = "";
}
