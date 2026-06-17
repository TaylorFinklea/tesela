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

import {
  commandRegistry,
  effectiveChord,
  type Command,
  type CommandContext,
} from "../command-registry.svelte.ts";
import * as keybindings from "../stores/keybindings.svelte.ts";
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

const CHORD_GROUP_LABELS: Record<string, string> = {
  g: "go to…",
  w: "windows…",
  b: "buffers…",
  n: "new…",
  i: "insert…",
  p: "properties…",
  v: "views…",
  a: "actions…",
  t: "toggle…",
  ",": "config…",
};

/**
 * The action for a leader LEAF. Editor commands (`category: 'editor'`) can't
 * run against the shell ctx — it has no live `editor` SlashContext — so they
 * dispatch `tesela:run-editor-command` to the focused BlockEditor, which
 * supplies one (mirrors the `g f` follow-wiki bridge). Every other command
 * runs in place. The dispatch only fires on a real keypress (browser), so
 * `document` is always defined here.
 */
function leafAction(leaf: Command, ctx?: CommandContext): () => void {
  if (leaf.category === 'editor') {
    return () =>
      document.dispatchEvent(
        new CustomEvent('tesela:run-editor-command', { detail: { id: leaf.id } }),
      );
  }
  return () => void leaf.run(undefined, ctx);
}

function buildChordTree(commands: Command[], depth: number, ctx?: CommandContext): ChordNode[] {
  const overrides = keybindings.snapshot();
  const groups = new Map<string, Command[]>();
  for (const cmd of commands) {
    const chord = effectiveChord(cmd, overrides);
    if (!chord || chord.length <= depth) continue;
    const key = chord[depth];
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(cmd);
  }

  const nodes: ChordNode[] = [];
  for (const [key, group] of groups) {
    const leaf = group.find((cmd) => effectiveChord(cmd, overrides)!.length === depth + 1);
    const branches = group.filter((cmd) => effectiveChord(cmd, overrides)!.length > depth + 1);

    if (leaf && branches.length === 0) {
      nodes.push({
        key,
        label: leaf.label,
        action: leafAction(leaf, ctx),
      });
    } else if (leaf && branches.length > 0) {
      // Both a leaf and a subtree share this key — show the leaf as the first
      // entry and the subtree below it.
      nodes.push({
        key,
        label: leaf.label,
        action: leafAction(leaf, ctx),
      });
      const children = buildChordTree(branches, depth + 1, ctx);
      if (children.length > 0) {
        nodes.push({
          key,
          label: `${leaf.label}…`,
          children,
        });
      }
    } else {
      const children = buildChordTree(branches, depth + 1, ctx);
      if (children.length > 0) {
        nodes.push({
          key,
          label: CHORD_GROUP_LABELS[key] ?? children.map((c) => c.label).join(" / "),
          children,
        });
      }
    }
  }

  // Stable order: sort by key (case-sensitive so 'D' comes after 'd').
  nodes.sort((a, b) => a.key.localeCompare(b.key));
  return nodes;
}

/** The chord tree the menu walks. Derived from the unified command registry. */
export function getLeaderTree(ctx?: CommandContext): ChordNode[] {
  const overrides = keybindings.snapshot();
  const commands = (ctx ? commandRegistry.availableOn('leader', ctx) : commandRegistry.all()).filter(
    (cmd) => {
      const chord = effectiveChord(cmd, overrides);
      return chord && chord.length > 0;
    },
  );
  return buildChordTree(commands, 0, ctx);
}
