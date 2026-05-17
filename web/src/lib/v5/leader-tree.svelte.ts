/**
 * Prism v5 — leader chord menu state + tree.
 *
 * Spacemacs/which-key style: Space (in NORMAL mode of cm-vim, or anywhere
 * outside a text entry) opens a hierarchical chord menu. Each keystroke
 * either descends into a sub-menu or runs an action and closes.
 *
 * The tree definition lives here so callers (commands.ts, +layout.svelte,
 * BlockEditor's vim binding) all read the same source.
 */

import { api } from "$lib/api-client";
import { getAppQueryClient } from "$lib/app-query-client.svelte";
import {
  closeFocusedLeaf,
  getWorkspace,
  hsplit,
  openPageInFocused,
  vsplit,
} from "$lib/buffer/state.svelte";
import { asPageId } from "$lib/buffer/types";
import {
  makeAmbientBuffer,
  makePageBuffer,
} from "$lib/buffer/tree";
import { openStation } from "$lib/stores/station.svelte";
import { openPeek } from "$lib/stores/peek.svelte";
import { openFullscreenGraph } from "$lib/stores/fullscreen-overlay.svelte";
import { openSettingsOverlay } from "$lib/stores/fullscreen-overlay.svelte";
// Type-only import via the `<script module>` block of ChordMenu.svelte.
// Svelte's TS support exports the module-script types via the .svelte
// path with a side-effect import.
type ChordNode = {
  key: string;
  label: string;
  action?: () => void;
  children?: ChordNode[];
  hint?: string;
};

let open = $state(false);
let initialPath = $state<string[]>([]);

export function isLeaderOpen(): boolean {
  return open;
}
export function openLeader(path: string[] = []): void {
  initialPath = path;
  open = true;
}
export function closeLeader(): void {
  open = false;
  initialPath = [];
}
export function getLeaderInitialPath(): string[] {
  return initialPath;
}

async function jumpDaily() {
  const d = await api.getDailyNote();
  openPageInFocused(asPageId(d.id));
}

async function newScratch() {
  const d = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const stamp = `scratch/${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}-${pad(d.getHours())}${pad(d.getMinutes())}`;
  const content = `---\ntitle: "${stamp}"\ntype: scratch\ntags: []\n---\n- \n`;
  const note = await api.createNote(stamp, content);
  openPageInFocused(asPageId(note.id));
  const qc = getAppQueryClient();
  if (qc) qc.invalidateQueries({ queryKey: ["notes"] });
}

async function newNote() {
  const title = window.prompt("note title")?.trim();
  if (!title) return;
  const note = await api.createNote(title, "");
  openPageInFocused(asPageId(note.id));
}

/** The chord tree the menu walks. Mirrors common spacemacs leader groups. */
export function getLeaderTree(): ChordNode[] {
  return [
    {
      key: "n",
      label: "new…",
      children: [
        { key: "s", label: "scratch", action: () => void newScratch() },
        { key: "n", label: "note", action: () => void newNote() },
        { key: "d", label: "daily", action: () => void jumpDaily() },
      ],
    },
    {
      key: "g",
      label: "go to…",
      children: [
        { key: "d", label: "today's daily", action: () => void jumpDaily() },
        {
          key: "c",
          label: "calendar",
          action: () => vsplit(makeAmbientBuffer("calendar")),
        },
        {
          key: "i",
          label: "in-progress",
          action: () => vsplit(makeAmbientBuffer("today-in-progress")),
        },
        {
          key: "h",
          label: "dashboard (home)",
          action: () => vsplit(makeAmbientBuffer("workspace-dashboard")),
        },
        {
          key: "g",
          label: "graph",
          action: () => openFullscreenGraph(),
        },
      ],
    },
    {
      key: "b",
      label: "buffer…",
      children: [
        {
          key: "v",
          label: "vsplit (empty)",
          action: () => vsplit(makePageBuffer(asPageId(""))),
        },
        {
          key: "h",
          label: "hsplit (empty)",
          action: () => hsplit(makePageBuffer(asPageId(""))),
        },
        {
          key: "q",
          label: "close pane",
          action: () => closeFocusedLeaf(),
        },
      ],
    },
    {
      key: "p",
      label: "peek (⌘I)",
      action: () => openPeek("backlinks-of-page"),
    },
    {
      key: "/",
      label: "command station (⌘K)",
      action: () =>
        openStation({
          tab: "palette",
          priorPaneId: getWorkspace().tabs.find(
            (t) => t.id === getWorkspace().activeTabId,
          )?.lastFocusedLeafId as unknown as string | undefined,
        }),
    },
    {
      key: ",",
      label: "settings",
      action: () => openSettingsOverlay("general"),
    },
  ];
}
