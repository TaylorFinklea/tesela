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
  type Command,
  type CommandContext,
} from "$lib/command-registry.svelte";
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
  n: "new…",
  g: "go to…",
  b: "buffer…",
};

function buildChordTree(
  commands: Command[],
  depth: number,
  ctx?: CommandContext,
): ChordNode[] {
  const available = ctx
    ? commands.filter((cmd) => {
        if (!cmd.when) return true;
        try {
          return cmd.when(ctx);
        } catch (e) {
          console.warn(`leader-tree: when() threw for "${cmd.id}"`, e);
          return false;
        }
      })
    : commands;

  const groups = new Map<string, Command[]>();
  for (const cmd of available) {
    if (!cmd.chord || cmd.chord.length <= depth) continue;
    const key = cmd.chord[depth];
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key)!.push(cmd);
  }

  const nodes: ChordNode[] = [];
  for (const [key, group] of groups) {
    const leaf = group.find((cmd) => cmd.chord!.length === depth + 1);
    const branches = group.filter((cmd) => cmd.chord!.length > depth + 1);

    if (leaf && branches.length === 0) {
      nodes.push({
        key,
        label: leaf.label,
        action: () => void leaf.run(),
      });
    } else if (leaf && branches.length > 0) {
      // Both a leaf and a subtree share this key — show the leaf as the first
      // entry and the subtree below it.
      nodes.push({
        key,
        label: leaf.label,
        action: () => void leaf.run(),
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
  const commands = commandRegistry.all().filter((cmd) => cmd.chord && cmd.chord.length > 0);
  return buildChordTree(commands, 0, ctx);
}
