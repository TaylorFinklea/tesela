/**
 * Widget registry. Builds the rail's widget list from notes whose
 * `note_type === "Query"`. Frontmatter properties drive each widget's
 * configuration:
 *
 *   - query::    DSL string (compact text, see `query-language.ts`)
 *   - group::    optional property key for grouping
 *   - sort::     optional sort spec
 *   - icon::     optional kind hint (`task` / `project` / `person` / etc.)
 *   - color::    optional explicit palette color name
 *   - section::  optional `pinned` | `browse` | `saved` (default `saved`)
 *
 * Reactive: callers should consume from a `createQuery(["notes", ...])` and
 * pipe the resulting Note[] through `parseWidgets()` so the rail stays in
 * sync with the WS-invalidated notes cache.
 */
import type { Note } from "$lib/types/Note";
import type { Widget, WidgetSection } from "$lib/types/Widget";

const SYSTEM_WIDGET_IDS: ReadonlySet<string> = new Set([
  "dailies",
  "tasks",
  "projects",
  "people",
  "inbox",
  "calendar",
  "recent",
  "pinned",
  "pages",
]);

function asString(v: unknown): string | null {
  if (typeof v === "string" && v.length > 0) return v;
  return null;
}

function asSection(v: unknown): WidgetSection {
  if (v === "pinned" || v === "browse" || v === "saved") return v;
  return "saved";
}

/**
 * Extract a `key:: value` continuation line from the body of a Query note.
 * Frontmatter `key: value` already lives in `note.metadata.custom`, but the
 * Logseq-style `key:: value` lines (which is what Tesela's block parser
 * understands) live in the body — so we read both.
 */
function readBodyProperty(body: string, key: string): string | null {
  const re = new RegExp(`^\\s*${key}::\\s*(.*)$`, "im");
  const m = body.match(re);
  return m && m[1].trim().length > 0 ? m[1].trim() : null;
}

/**
 * Build a Widget from a Note that has `note_type: Query`.
 */
export function widgetFromNote(note: Note): Widget {
  const custom = note.metadata.custom ?? {};
  const body = note.content.includes("\n---")
    ? note.content.slice(note.content.indexOf("\n---", 3) + 4)
    : note.content;
  const fm = (k: string) => asString(custom[k]) ?? readBodyProperty(body, k);
  const icon = fm("icon");
  const color = fm("color");
  const section = asSection(fm("section"));
  return {
    id: note.id,
    title: note.title,
    query: fm("query") ?? "",
    group: fm("group"),
    sort: fm("sort"),
    icon,
    color,
    section,
    view: fm("view"),
    system: SYSTEM_WIDGET_IDS.has(note.id),
  };
}

/**
 * Filter a Note[] down to just the Query widgets, sorted for stable rail
 * ordering: section (pinned → browse → saved), then by title.
 */
export function parseWidgets(notes: Note[]): Widget[] {
  const sectionOrder: Record<WidgetSection, number> = { pinned: 0, browse: 1, saved: 2 };
  return notes
    .filter((n) => n.metadata.note_type === "Query")
    .map(widgetFromNote)
    .sort((a, b) => {
      const so = sectionOrder[a.section] - sectionOrder[b.section];
      if (so !== 0) return so;
      return a.title.localeCompare(b.title);
    });
}

/**
 * Group widgets by section for rendering. Order preserved from `parseWidgets`.
 */
export function widgetsBySection(widgets: Widget[]): Record<WidgetSection, Widget[]> {
  const out: Record<WidgetSection, Widget[]> = { pinned: [], browse: [], saved: [] };
  for (const w of widgets) out[w.section].push(w);
  return out;
}
