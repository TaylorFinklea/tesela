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
  if (viewModes[tagName] !== undefined) return viewModes[tagName];
  const stored = load(VIEW_KEY_PREFIX + tagName);
  const mode = stored === "kanban" ? "kanban" : "table";
  viewModes[tagName] = mode;
  return mode;
}

export function setViewMode(tagName: string, mode: ViewMode) {
  viewModes[tagName] = mode;
  save(VIEW_KEY_PREFIX + tagName, mode);
}

export function getGroupByProp(tagName: string): string {
  if (groupByProps[tagName] !== undefined) return groupByProps[tagName];
  const stored = load(GROUP_KEY_PREFIX + tagName);
  groupByProps[tagName] = stored ?? "";
  return stored ?? "";
}

export function setGroupByProp(tagName: string, propName: string) {
  groupByProps[tagName] = propName;
  save(GROUP_KEY_PREFIX + tagName, propName);
}
