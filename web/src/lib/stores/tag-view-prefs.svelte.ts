/**
 * Tag page view preferences — persisted in localStorage per tag.
 * Stores view mode (table/kanban) and kanban group-by property.
 */
import { browser } from "$app/environment";

type ViewMode = "table" | "kanban";

const VIEW_KEY_PREFIX = "tesela:tag-view:";
const GROUP_KEY_PREFIX = "tesela:kanban-group:";

function load(key: string): string | null {
  if (!browser) return null;
  try {
    return localStorage.getItem(key);
  } catch {
    return null;
  }
}

function save(key: string, value: string) {
  if (!browser) return;
  try {
    localStorage.setItem(key, value);
  } catch {
    // localStorage full or blocked
  }
}

let viewModes = $state<Record<string, ViewMode>>({});
let groupByProps = $state<Record<string, string>>({});

export function getViewMode(tagName: string): ViewMode {
  // Read $state first (for reactivity tracking), fall back to localStorage
  const cached = viewModes[tagName];
  if (cached !== undefined) return cached;
  const stored = load(VIEW_KEY_PREFIX + tagName);
  return stored === "kanban" ? "kanban" : "table";
}

export function setViewMode(tagName: string, mode: ViewMode) {
  viewModes[tagName] = mode;
  save(VIEW_KEY_PREFIX + tagName, mode);
}

export function getGroupByProp(tagName: string): string {
  const cached = groupByProps[tagName];
  if (cached !== undefined) return cached;
  const stored = load(GROUP_KEY_PREFIX + tagName);
  return stored ?? "";
}

export function setGroupByProp(tagName: string, propName: string) {
  groupByProps[tagName] = propName;
  save(GROUP_KEY_PREFIX + tagName, propName);
}
