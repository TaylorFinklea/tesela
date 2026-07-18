import type { ViewRecord } from "$lib/api-client";
import type { QueryItem } from "$lib/types/QueryItem";
import type { QueryResult } from "$lib/types/QueryResult";
import type { Widget } from "$lib/types/Widget";

export const RAIL_LAYOUT_STORAGE_KEY = "tesela:graphite:rail-layout:v1";
export const RAIL_PROJECTION_QUERY_KEY = "rail-projection";

export type RailBuiltinId =
  | "quick-capture"
  | "inbox"
  | "agenda"
  | "favorites"
  | "pinned"
  | "recent"
  | "sync-health";

export type RailWidgetPlacement = {
  id: string;
  fallbackTitle: string;
  collapsed: boolean;
};

export type RailWidgetLayout = {
  version: 1;
  placements: RailWidgetPlacement[];
};

export type RailWidgetCandidate = RailWidgetPlacement & {
  kind: "builtin" | "query" | "view";
  sourceId: string;
  title: string;
  subtitle: string;
  icon: string;
};

export type RailQueryProjection = {
  id: string;
  title: string;
  icon: string;
  dsl: string;
  group: string | null;
  sort: string | null;
  definitionRevision: string;
};

export const BUILTIN_RAIL_WIDGETS: ReadonlyArray<RailWidgetCandidate> = [
  builtin("quick-capture", "Quick capture", "Capture without leaving the current page", "bolt"),
  builtin("inbox", "Inbox", "Canonical saved view", "inbox"),
  builtin("agenda", "Agenda", "Open tasks and scheduled work", "square-check"),
  builtin("favorites", "Favorites", "Device-local favorite pages", "star"),
  builtin("pinned", "Pinned", "Pinned workspace pages", "pin"),
  builtin("recent", "Today", "Recently focused pages", "sun"),
  builtin("sync-health", "Sync Health", "Relay and live connection status", "circle-dot"),
];

export const DEFAULT_RAIL_WIDGET_LAYOUT: RailWidgetLayout = {
  version: 1,
  placements: BUILTIN_RAIL_WIDGETS.map(({ id, fallbackTitle, collapsed }) => ({
    id,
    fallbackTitle,
    collapsed,
  })),
};

function builtin(id: RailBuiltinId, title: string, subtitle: string, icon: string): RailWidgetCandidate {
  return {
    id: `builtin:${id}`,
    kind: "builtin",
    sourceId: id,
    title,
    fallbackTitle: title,
    subtitle,
    icon,
    collapsed: false,
  };
}

function cloneDefaultLayout(): RailWidgetLayout {
  return {
    version: 1,
    placements: DEFAULT_RAIL_WIDGET_LAYOUT.placements.map((placement) => ({ ...placement })),
  };
}

function validPlacement(value: unknown): RailWidgetPlacement | null {
  if (!value || typeof value !== "object") return null;
  const raw = value as Record<string, unknown>;
  if (typeof raw.id !== "string" || !/^(builtin|query|view):.+/.test(raw.id)) return null;
  return {
    id: raw.id,
    fallbackTitle:
      typeof raw.fallbackTitle === "string" && raw.fallbackTitle.trim()
        ? raw.fallbackTitle.trim()
        : sourceIdFromPlacement(raw.id),
    collapsed: raw.collapsed === true,
  };
}

export function normalizeRailWidgetLayout(value: unknown): RailWidgetLayout {
  let rawPlacements: unknown[] | null = null;
  if (Array.isArray(value)) rawPlacements = value;
  else if (value && typeof value === "object") {
    const raw = value as Record<string, unknown>;
    if (raw.version === 1 && Array.isArray(raw.placements)) rawPlacements = raw.placements;
  }
  if (!rawPlacements) return cloneDefaultLayout();

  const seen = new Set<string>();
  const placements: RailWidgetPlacement[] = [];
  for (const raw of rawPlacements) {
    const placement = typeof raw === "string"
      ? validPlacement({ id: raw, fallbackTitle: sourceIdFromPlacement(raw) })
      : validPlacement(raw);
    if (!placement || seen.has(placement.id)) continue;
    seen.add(placement.id);
    placements.push(placement);
  }
  return { version: 1, placements };
}

export function loadRailWidgetLayout(storage: Pick<Storage, "getItem"> | null = browserStorage()): RailWidgetLayout {
  if (!storage) return cloneDefaultLayout();
  const raw = storage.getItem(RAIL_LAYOUT_STORAGE_KEY);
  if (raw === null) return cloneDefaultLayout();
  try {
    return normalizeRailWidgetLayout(JSON.parse(raw));
  } catch {
    return cloneDefaultLayout();
  }
}

export function saveRailWidgetLayout(
  layout: RailWidgetLayout,
  storage: Pick<Storage, "setItem"> | null = browserStorage(),
): void {
  storage?.setItem(RAIL_LAYOUT_STORAGE_KEY, JSON.stringify(normalizeRailWidgetLayout(layout)));
}

function browserStorage(): Storage | null {
  return typeof localStorage === "undefined" ? null : localStorage;
}

export function addRailWidget(layout: RailWidgetLayout, candidate: RailWidgetCandidate): RailWidgetLayout {
  if (layout.placements.some((placement) => placement.id === candidate.id)) return layout;
  return {
    version: 1,
    placements: [
      ...layout.placements,
      { id: candidate.id, fallbackTitle: candidate.title, collapsed: false },
    ],
  };
}

export function removeRailWidget(layout: RailWidgetLayout, id: string): RailWidgetLayout {
  return { version: 1, placements: layout.placements.filter((placement) => placement.id !== id) };
}

export function moveRailWidget(layout: RailWidgetLayout, id: string, delta: -1 | 1): RailWidgetLayout {
  const index = layout.placements.findIndex((placement) => placement.id === id);
  const target = index + delta;
  if (index < 0 || target < 0 || target >= layout.placements.length) return layout;
  const placements = layout.placements.map((placement) => ({ ...placement }));
  [placements[index], placements[target]] = [placements[target], placements[index]];
  return { version: 1, placements };
}

export function toggleRailWidgetCollapsed(layout: RailWidgetLayout, id: string): RailWidgetLayout {
  return {
    version: 1,
    placements: layout.placements.map((placement) =>
      placement.id === id ? { ...placement, collapsed: !placement.collapsed } : placement
    ),
  };
}

export function placementKind(id: string): "builtin" | "query" | "view" | null {
  const prefix = id.split(":", 1)[0];
  return prefix === "builtin" || prefix === "query" || prefix === "view" ? prefix : null;
}

export function sourceIdFromPlacement(id: string): string {
  const separator = id.indexOf(":");
  return separator < 0 ? id : id.slice(separator + 1);
}

export function queryWidgetCandidate(widget: Widget): RailWidgetCandidate {
  return {
    id: `query:${widget.id}`,
    kind: "query",
    sourceId: widget.id,
    title: widget.title,
    fallbackTitle: widget.title,
    subtitle: "Query note",
    icon: normalizeRailWidgetIcon(widget.icon),
    collapsed: false,
  };
}

export function savedViewCandidate(view: ViewRecord): RailWidgetCandidate {
  return {
    id: `view:${view.id}`,
    kind: "view",
    sourceId: view.id,
    title: view.name,
    fallbackTitle: view.name,
    subtitle: "Saved view",
    icon: "inbox",
    collapsed: false,
  };
}

export function savedViewRevision(view: ViewRecord): string {
  return JSON.stringify([
    view.id,
    view.name,
    view.dsl,
    view.order,
    view.display_mode,
    view.display_group_by,
    view.display_show_done,
    view.display_table_config,
  ]);
}

export function projectionFromQueryWidget(widget: Widget, checksum: string): RailQueryProjection {
  return {
    id: `query:${widget.id}`,
    title: widget.title,
    icon: normalizeRailWidgetIcon(widget.icon),
    dsl: widget.query,
    group: widget.group,
    sort: widget.sort,
    definitionRevision: checksum,
  };
}

function normalizeRailWidgetIcon(icon: string | null): string {
  if (icon === "task") return "square-check";
  if (icon === "project") return "folder";
  if (icon === "person") return "user";
  if (icon === "cal") return "calendar";
  if (icon === "note") return "file-text";
  return icon ?? "search";
}

export function projectionFromSavedView(view: ViewRecord, id = `view:${view.id}`): RailQueryProjection {
  return {
    id,
    title: view.name,
    icon: view.id === "builtin-inbox" ? "inbox" : "search",
    dsl: view.dsl,
    group: view.display_group_by,
    sort: null,
    definitionRevision: savedViewRevision(view),
  };
}

export function flattenRailQueryRows(result: QueryResult | undefined, limit = 6): QueryItem[] {
  if (!result) return [];
  return result.groups.flatMap((group) => group.items).slice(0, limit);
}
