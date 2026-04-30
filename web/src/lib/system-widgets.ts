/**
 * System widgets — pre-defined Query notes that ship with v9. Created on
 * first app load if missing. The Phase 9.0 rail nav (Today / Pages / Timeline /
 * Graph / Properties) is reframed here as Query widgets so the rail is purely
 * data-driven from 9.1 onward.
 *
 * Widgets that don't have real middle-column behavior yet (Calendar, Inbox)
 * still ship as Query notes — their middle-column renderers stub to
 * "Coming in 9.x" until the corresponding phase lands.
 */
import type { api as Api } from "$lib/api-client";

type SystemWidgetSpec = {
  id: string;
  title: string;
  query: string;
  group?: string;
  sort?: string;
  icon?: string;
  color?: string;
  section: "pinned" | "browse" | "saved";
};

export const SYSTEM_WIDGETS: SystemWidgetSpec[] = [
  // Pinned — daily-driver entry points.
  {
    id: "today",
    title: "Today",
    query: "",
    section: "pinned",
    icon: "calendar",
    color: "amber",
  },
  {
    id: "pages",
    title: "Pages",
    query: "kind:page",
    section: "pinned",
    icon: "cal",
    color: "amber-2",
  },
  // Browse — corpus-wide views.
  {
    id: "tasks",
    title: "Tasks",
    query: "kind:block tag:Task -status:done",
    group: "status",
    sort: "deadline asc",
    section: "browse",
    icon: "task",
    color: "rose",
  },
  {
    id: "projects",
    title: "Projects",
    query: "kind:page note_type:Project",
    section: "browse",
    icon: "project",
    color: "indigo",
  },
  {
    id: "people",
    title: "People",
    query: "kind:page note_type:Person",
    section: "browse",
    icon: "person",
    color: "plum",
  },
  {
    id: "inbox",
    title: "Inbox",
    // 9.2 will refine to "no parent project, not daily, no status".
    query: "kind:block -has:status",
    section: "browse",
    icon: "inbox",
    color: "teal",
  },
  {
    id: "calendar",
    title: "Calendar",
    // 9.2 lights this up with the mini-calendar UI; for now it's a
    // listing of dated blocks.
    query: "kind:block has:scheduled",
    sort: "scheduled asc",
    section: "browse",
    icon: "cal",
    color: "amber-2",
  },
  // Saved — quick filters
  {
    id: "recent",
    title: "Recent",
    query: "kind:page",
    sort: "modified desc",
    section: "saved",
    icon: "clock",
    color: "ochre",
  },
  {
    id: "pinned",
    title: "Pinned",
    query: "kind:page",
    section: "saved",
    icon: "pin",
    color: "rose",
  },
];

function widgetTemplate(w: SystemWidgetSpec): string {
  const lines = [
    "---",
    `title: "${w.title}"`,
    `type: "Query"`,
    "tags: []",
    "---",
    `query:: ${w.query}`,
  ];
  if (w.group) lines.push(`group:: ${w.group}`);
  if (w.sort) lines.push(`sort:: ${w.sort}`);
  if (w.icon) lines.push(`icon:: ${w.icon}`);
  if (w.color) lines.push(`color:: ${w.color}`);
  lines.push(`section:: ${w.section}`);
  lines.push(""); // trailing newline
  return lines.join("\n");
}

/**
 * Idempotent: ensure every system widget exists as a Query note. Safe to call
 * on every app load. Per-widget `getNote` lookup; only creates on 404.
 *
 * `apiClient` is injected so callers can pass the singleton from `api-client.ts`
 * without coupling this module to it.
 */
export async function ensureSystemWidgets(
  apiClient: typeof Api,
): Promise<{ created: string[]; existing: string[] }> {
  const created: string[] = [];
  const existing: string[] = [];
  await Promise.all(
    SYSTEM_WIDGETS.map(async (w) => {
      try {
        await apiClient.getNote(w.id);
        existing.push(w.id);
      } catch (err) {
        // 404 (or similar) → create. Anything else, swallow — we don't want
        // a transient backend hiccup to block app load.
        try {
          await apiClient.createNote(w.title, widgetTemplate(w));
          created.push(w.id);
        } catch {
          // give up silently; user can re-create manually via ⌘K.
        }
      }
    }),
  );
  return { created, existing };
}
