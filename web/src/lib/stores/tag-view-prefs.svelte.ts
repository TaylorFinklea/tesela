/**
 * Tag page view preferences — persisted in localStorage per tag.
 * Stores view mode (table/kanban), kanban group-by property, and
 * (tesela-ya4.4) the non-saved-view table's column display config
 * (hide/reorder/sort).
 */
import { browser } from "$app/environment";
import { EMPTY_TABLE_CONFIG, type TableColumnConfig } from "$lib/table/table-config";

type ViewMode = "table" | "kanban";

const VIEW_KEY_PREFIX = "tesela:tag-view:";
const GROUP_KEY_PREFIX = "tesela:kanban-group:";
const TABLE_CONFIG_KEY_PREFIX = "tesela:table-config:";

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

// tesela-ya4.4 — table column config (hide/reorder/sort) for a NON-saved-
// view table (a plain tag page / Query-note widget). A saved-view table's
// config is round-trip-authoritative through `updateView` instead (spec
// decision 4) — see `QueryTable.svelte`'s `viewId` branch.
let tableConfigs = $state<Record<string, TableColumnConfig>>({});

export function getTableConfig(key: string): TableColumnConfig {
  const cached = tableConfigs[key];
  if (cached !== undefined) return cached;
  const stored = load(TABLE_CONFIG_KEY_PREFIX + key);
  if (!stored) return EMPTY_TABLE_CONFIG;
  try {
    return JSON.parse(stored) as TableColumnConfig;
  } catch {
    return EMPTY_TABLE_CONFIG;
  }
}

export function setTableConfig(key: string, config: TableColumnConfig) {
  tableConfigs[key] = config;
  save(TABLE_CONFIG_KEY_PREFIX + key, JSON.stringify(config));
}
