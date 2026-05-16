/**
 * Prism v5 command verbs — the rows that appear in the Station's Palette
 * tab and the `:` ex-mode dispatcher.
 *
 * File still lives under `lib/v4/` for now to avoid mid-cutover import
 * churn; will move to `lib/v5/` in Phase 13.
 */

import { api } from "$lib/api-client";
import {
  closeFocusedLeaf,
  closeTab,
  getWorkspace,
  hsplit,
  movePane,
  newTab,
  openPageInFocused,
  vsplit,
} from "$lib/buffer/state.svelte";
import {
  asPageId,
  type DerivedBinding,
} from "$lib/buffer/types";
import {
  makeAmbientBuffer,
  makeDerivedBuffer,
  makePageBuffer,
} from "$lib/buffer/tree";
import {
  openSettingsOverlay,
  type SettingsSlug,
} from "$lib/stores/fullscreen-overlay.svelte";

const SETTINGS_PAGES: { slug: SettingsSlug; label: string }[] = [
  { slug: "general", label: "General" },
  { slug: "devices", label: "Devices" },
  { slug: "sync", label: "Sync" },
  { slug: "mosaic", label: "Mosaic" },
  { slug: "data", label: "Data" },
];

const DERIVED_RENDERERS: { name: string; label: string; verb: string; glyph: string }[] = [
  { name: "backlinks-of-page", label: "Backlinks (follow)", verb: "backlinks", glyph: "↩" },
  { name: "outline-of-page", label: "Outline (follow)", verb: "outline", glyph: "⋮" },
  { name: "properties-of-page", label: "Properties (follow)", verb: "properties", glyph: "⚙" },
  { name: "tasks-linked-to-page", label: "Linked tasks (follow)", verb: "tasks", glyph: "☑" },
  { name: "local-graph-of-page", label: "Local graph (follow)", verb: "graph-local", glyph: "✦" },
];

const AMBIENTS: { name: string; label: string; verb: string; glyph: string }[] = [
  { name: "calendar", label: "Calendar", verb: "calendar", glyph: "📅" },
  { name: "today-in-progress", label: "Today in progress", verb: "in-progress", glyph: "⏱" },
  { name: "workspace-dashboard", label: "Workspace dashboard", verb: "dashboard", glyph: "▦" },
  { name: "ai-workspace", label: "AI workspace", verb: "ai", glyph: "✺" },
];

export type V4Command = {
  id: string;
  verb?: string;
  label: string;
  glyph: string;
  category: "pane" | "tab" | "tile" | "create" | "navigate" | "derived" | "ambient";
  shortcut?: string;
  keywords: string[];
  argPrompt?: string;
  run: (arg?: string) => void | Promise<void>;
};

async function jumpToDaily() {
  const daily = await api.getDailyNote();
  openPageInFocused(asPageId(daily.id));
}

async function createNoteAndJump(title: string) {
  const note = await api.createNote(title, "");
  openPageInFocused(asPageId(note.id));
}

function followBinding(): DerivedBinding {
  return { mode: "follow" };
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
      run: () => vsplit(makePageBuffer(asPageId(""))),
    },
    {
      id: "hsplit",
      verb: "hsplit",
      label: "Split pane horizontally",
      glyph: "─",
      category: "pane",
      shortcut: "⌘-",
      keywords: ["split", "hsplit", "horizontal", "below", "pane"],
      run: () => hsplit(makePageBuffer(asPageId(""))),
    },
    {
      id: "close-pane",
      verb: "quit",
      label: "Close focused pane",
      glyph: "×",
      category: "pane",
      shortcut: "⌘W",
      keywords: ["close", "quit", "kill", "pane"],
      run: () => closeFocusedLeaf(),
    },
    ...(["left", "right", "up", "down"] as const).map((dir) => ({
      id: `move-${dir}`,
      verb: `move-${dir}`,
      label: `Move pane ${dir}`,
      glyph:
        dir === "left" ? "←" : dir === "right" ? "→" : dir === "up" ? "↑" : "↓",
      category: "pane" as const,
      shortcut: `⌘⇧${dir === "left" ? "H" : dir === "right" ? "L" : dir === "up" ? "K" : "J"}`,
      keywords: ["move", "push", "send", dir, "pane"],
      run: () => movePane(dir),
    })),

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
      run: () => closeTab(getWorkspace().activeTabId),
    },

    // ── tile (page-buffer ops) ──────────────────────────────────────────
    {
      id: "jump",
      verb: "jump",
      label: "Jump to page…",
      glyph: "→",
      category: "tile",
      keywords: ["jump", "go", "open", "tile", "note", "page"],
      argPrompt: "note slug or id",
      run: (arg) => {
        if (arg) openPageInFocused(asPageId(arg));
      },
    },

    // ── derived buffers ────────────────────────────────────────────────
    ...DERIVED_RENDERERS.map((d) => ({
      id: d.verb,
      verb: d.verb,
      label: `Open ${d.label}`,
      glyph: d.glyph,
      category: "derived" as const,
      keywords: [d.verb, "derived", "follow", d.name],
      run: () => vsplit(makeDerivedBuffer(d.name, followBinding())),
    })),

    // ── ambient buffers ────────────────────────────────────────────────
    ...AMBIENTS.map((a) => ({
      id: a.verb,
      verb: a.verb,
      label: `Open ${a.label}`,
      glyph: a.glyph,
      category: "ambient" as const,
      keywords: [a.verb, "ambient", a.name],
      run: () => vsplit(makeAmbientBuffer(a.name)),
    })),

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
    {
      id: "scratch",
      verb: "scratch",
      label: "New scratch page",
      glyph: "✎",
      category: "create",
      shortcut: "Space n s",
      keywords: ["scratch", "draft", "throwaway", "new"],
      run: async () => {
        // Scratch impl proper lands in Phase 11. For now: create a
        // timestamped note with no special type and open it.
        const d = new Date();
        const stamp = `scratch/${d.toISOString().slice(0, 16).replace(":", "-")}`;
        await createNoteAndJump(stamp);
      },
    },
    ...SETTINGS_PAGES.map(({ slug, label }) => ({
      id: `settings-${slug}`,
      verb: `settings-${slug}`,
      label: `Settings · ${label}`,
      glyph: "⚙",
      category: "navigate" as const,
      keywords: ["settings", "preferences", "config", slug, label.toLowerCase()],
      run: () => openSettingsOverlay(slug),
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

export function findCommandByVerb(verb: string): V4Command | undefined {
  const v = verb.toLowerCase();
  return buildV4Commands().find((c) => c.verb === v || c.id === v);
}
