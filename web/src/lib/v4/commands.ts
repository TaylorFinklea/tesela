/**
 * Prism v4 command verbs — the rows that appear in the Station's Palette
 * tab. Kept separate from the legacy `lib/commands.ts` (which is wired into
 * the old chrome's CommandPalette and its `deps` shape) so v4's verb set
 * can evolve independently. Phases 5+ add `:`-prefixed ex-mode dispatch on
 * top of the same registry.
 */

import { goto } from "$app/navigation";
import { api } from "$lib/api-client";
import {
  closePane,
  closeTab,
  getState,
  hsplit,
  jumpToTile,
  newTab,
  stackAdd,
  stackNext,
  vsplit,
} from "$lib/stores/pane-tree.svelte";

const SETTINGS_PAGES: { slug: string; label: string }[] = [
  { slug: "general", label: "General" },
  { slug: "devices", label: "Devices" },
  { slug: "sync", label: "Sync" },
  { slug: "mosaic", label: "Mosaic" },
  { slug: "data", label: "Data" },
];

export type V4Command = {
  id: string;
  /** The colon verb form, used by Phase 5's `:` ex-mode (without the `:`). */
  verb?: string;
  label: string;
  /** Short glyph rendered alongside the label — keeps Palette rows scannable. */
  glyph: string;
  category: "pane" | "tab" | "tile" | "create" | "navigate";
  shortcut?: string;
  keywords: string[];
  /** If set, the verb prompts the user for a single string arg before running. */
  argPrompt?: string;
  run: (arg?: string) => void | Promise<void>;
};

async function jumpToDaily() {
  const daily = await api.getDailyNote();
  jumpToTile(daily.id);
}

async function createNoteAndJump(title: string) {
  const note = await api.createNote(title, "");
  jumpToTile(note.id);
}

export function buildV4Commands(): V4Command[] {
  return [
    // ── pane ────────────────────────────────────────────────────────────
    {
      id: "vsplit",
      verb: "vsplit",
      label: "Split pane vertically",
      glyph: "│",
      category: "pane",
      shortcut: "⌘\\",
      keywords: ["split", "vsplit", "vertical", "right", "pane"],
      run: () => vsplit("editor"),
    },
    {
      id: "hsplit",
      verb: "hsplit",
      label: "Split pane horizontally",
      glyph: "─",
      category: "pane",
      shortcut: "⌘-",
      keywords: ["split", "hsplit", "horizontal", "below", "pane"],
      run: () => hsplit("editor"),
    },
    {
      id: "close-pane",
      verb: "quit",
      label: "Close focused pane",
      glyph: "×",
      category: "pane",
      shortcut: "⌘W",
      keywords: ["close", "quit", "kill", "pane"],
      run: () => closePane(),
    },

    // ── tab ─────────────────────────────────────────────────────────────
    {
      id: "tabnew",
      verb: "tabnew",
      label: "New tab",
      glyph: "+",
      category: "tab",
      shortcut: "⌘T",
      keywords: ["tab", "new", "open", "window"],
      run: () => newTab(),
    },
    {
      id: "tab-close",
      verb: "tabclose",
      label: "Close current tab",
      glyph: "×",
      category: "tab",
      shortcut: "⌘⇧W",
      keywords: ["tab", "close", "kill"],
      run: () => closeTab(getState().activeTabId),
    },

    // ── tile ────────────────────────────────────────────────────────────
    {
      id: "jump",
      verb: "jump",
      label: "Jump to tile…",
      glyph: "→",
      category: "tile",
      keywords: ["jump", "go", "open", "tile", "note", "page"],
      argPrompt: "note slug or id",
      run: (arg) => {
        if (arg) jumpToTile(arg);
      },
    },
    {
      id: "stack",
      verb: "stack",
      label: "Stack tile on focused pane…",
      glyph: "≣",
      category: "tile",
      keywords: ["stack", "add", "zellij", "tile"],
      argPrompt: "note slug or id",
      run: (arg) => {
        if (arg) stackAdd(arg);
      },
    },
    {
      id: "stack-next",
      verb: "stacknext",
      label: "Cycle stack forward",
      glyph: "]",
      category: "tile",
      shortcut: "]",
      keywords: ["stack", "next", "cycle"],
      run: () => stackNext(1),
    },
    {
      id: "stack-prev",
      verb: "stackprev",
      label: "Cycle stack backward",
      glyph: "[",
      category: "tile",
      shortcut: "[",
      keywords: ["stack", "prev", "cycle"],
      run: () => stackNext(-1),
    },

    // ── create / navigate ───────────────────────────────────────────────
    {
      id: "daily",
      verb: "daily",
      label: "Today's daily note",
      glyph: "☀",
      category: "navigate",
      keywords: ["daily", "today", "journal"],
      run: () => jumpToDaily(),
    },
    // One palette row per settings page so users can pick directly
     //   without a modal arg prompt. The `:settings <page>` ex-form is
     //   covered by `findCommandByVerb` matching the page slug.
    ...SETTINGS_PAGES.map(({ slug, label }) => ({
      id: `settings-${slug}`,
      verb: `settings-${slug}`,
      label: `Settings · ${label}`,
      glyph: "⚙",
      category: "navigate" as const,
      keywords: ["settings", "preferences", "config", slug, label.toLowerCase()],
      run: () => goto(`/settings/${slug}`),
    })),
    {
      id: "new-note",
      verb: "new",
      label: "New note…",
      glyph: "✎",
      category: "create",
      keywords: ["new", "create", "note", "page"],
      argPrompt: "note title",
      run: (arg) => {
        if (arg) return createNoteAndJump(arg);
      },
    },
  ];
}

export function matchesV4Command(cmd: V4Command, query: string): boolean {
  if (!query) return true;
  const q = query.toLowerCase();
  if (cmd.label.toLowerCase().includes(q)) return true;
  if (cmd.verb && cmd.verb.toLowerCase().includes(q)) return true;
  return cmd.keywords.some((kw) => kw.includes(q));
}

/** Look up a verb (used by Phase 5's `:` ex-mode parser). */
export function findCommandByVerb(verb: string): V4Command | undefined {
  const v = verb.toLowerCase();
  return buildV4Commands().find((c) => c.verb === v || c.id === v);
}
