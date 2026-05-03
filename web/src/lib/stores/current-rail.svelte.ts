/**
 * Phase 9.5b — "current rail widget": the widget the user is most recently
 * working from. The middle column always shows this widget's result list, so
 * drilling from a rail widget into individual pages keeps the source list
 * visible (Finder column-view metaphor).
 *
 * Updates automatically when the focus pane lands on a Query-typed note. Set
 * via `setCurrentRailWidget` from `+page.svelte`'s effect.
 *
 * Persisted to localStorage so a reload preserves the user's last rail
 * context. Default falls back to "pages" so a fresh user sees something
 * sensible in the middle column.
 */
import { browser } from "$app/environment";

const RAIL_KEY = "tesela:currentRailWidget";
const DEFAULT_RAIL = "pages";

function loadCurrentRail(): string {
  if (!browser) return DEFAULT_RAIL;
  try {
    return localStorage.getItem(RAIL_KEY) ?? DEFAULT_RAIL;
  } catch {
    return DEFAULT_RAIL;
  }
}

function saveCurrentRail(id: string) {
  if (!browser) return;
  try {
    if (id) localStorage.setItem(RAIL_KEY, id);
    else localStorage.removeItem(RAIL_KEY);
  } catch {
    // ignore
  }
}

let currentRail = $state(loadCurrentRail());

export function getCurrentRailWidget(): string {
  return currentRail;
}

export function setCurrentRailWidget(id: string) {
  if (!id || id === currentRail) return;
  currentRail = id;
  saveCurrentRail(id);
}
