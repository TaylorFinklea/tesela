/**
 * Command registry for the Tesela command palette.
 * Each command has an id, label, category, optional shortcut, keywords for fuzzy matching,
 * and an action function. Context-dependent commands specify which route they appear on.
 */

export type CommandCategory = "action" | "navigation" | "context";

export interface Command {
  id: string;
  label: string;
  icon: string; // Tabler icon name
  category: CommandCategory;
  shortcut?: string;
  keywords: string[];
  context?: string; // e.g. "note-page" — only show when on this route pattern
  action: () => void | Promise<void>;
}

export function buildCommands(deps: {
  goto: (path: string) => void;
  createNote: (title: string) => Promise<void>;
  createType: (title: string) => Promise<void>;
  goToDaily: () => Promise<void>;
  toggleSidebar: () => void;
  toggleTheme: () => void;
  deleteNote?: () => void;
  copyNoteLink?: () => void;
}): Command[] {
  const commands: Command[] = [
    // === Actions ===
    {
      id: "new-note",
      label: "New Note",
      icon: "IconFilePlus",
      category: "action",
      shortcut: "⌘N",
      keywords: ["new", "create", "note", "page"],
      action: () => deps.createNote(""),
    },
    {
      id: "new-type",
      label: "New Type",
      icon: "IconTag",
      category: "action",
      keywords: ["new", "create", "type", "tag"],
      action: () => deps.createType(""),
    },
    {
      id: "daily-note",
      label: "Today's Daily Note",
      icon: "IconSun",
      category: "action",
      shortcut: "⌘D",
      keywords: ["daily", "today", "journal", "morning"],
      action: () => deps.goToDaily(),
    },
    {
      id: "toggle-theme",
      label: "Toggle Theme (Day ↔ Evening)",
      icon: "IconMoon",
      category: "action",
      keywords: ["theme", "dark", "light", "mode", "toggle", "day", "evening"],
      action: () => deps.toggleTheme(),
    },
    {
      id: "toggle-sidebar",
      label: "Toggle Sidebar",
      icon: "IconLayoutSidebar",
      category: "action",
      shortcut: "1",
      keywords: ["sidebar", "toggle", "collapse", "expand", "panel"],
      action: () => deps.toggleSidebar(),
    },

    // === Navigation ===
    {
      id: "go-home",
      label: "All Notes",
      icon: "IconHome",
      category: "navigation",
      keywords: ["home", "all", "notes", "list"],
      action: () => deps.goto("/"),
    },
    {
      id: "go-timeline",
      label: "Journal / Timeline",
      icon: "IconCalendarEvent",
      category: "navigation",
      keywords: ["journal", "timeline", "daily", "calendar"],
      action: () => deps.goto("/timeline"),
    },
    {
      id: "go-graph",
      label: "Graph View",
      icon: "IconGraph",
      category: "navigation",
      keywords: ["graph", "connections", "links", "network"],
      action: () => deps.goto("/graph"),
    },
    {
      id: "go-settings",
      label: "Settings",
      icon: "IconSettings",
      category: "navigation",
      keywords: ["settings", "preferences", "config", "theme"],
      action: () => deps.goto("/settings"),
    },
  ];

  // === Context actions (only on note pages) ===
  if (deps.deleteNote) {
    commands.push({
      id: "delete-note",
      label: "Delete This Note",
      icon: "IconTrash",
      category: "context",
      keywords: ["delete", "remove", "trash"],
      context: "note-page",
      action: () => deps.deleteNote!(),
    });
  }

  if (deps.copyNoteLink) {
    commands.push({
      id: "copy-link",
      label: "Copy Note Link",
      icon: "IconLink",
      category: "context",
      keywords: ["copy", "link", "url", "share"],
      context: "note-page",
      action: () => deps.copyNoteLink!(),
    });
  }

  return commands;
}

/** Fuzzy match a command against a search query. */
export function matchesQuery(cmd: Command, query: string): boolean {
  const q = query.toLowerCase();
  if (cmd.label.toLowerCase().includes(q)) return true;
  return cmd.keywords.some((kw) => kw.includes(q));
}
