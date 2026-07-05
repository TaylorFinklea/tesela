/**
 * Leader chord menu state + tree.
 *
 * Spacemacs/which-key style: Space (in NORMAL mode of cm-vim, or anywhere
 * outside a text entry) opens a hierarchical chord menu. Each keystroke
 * either descends into a sub-menu or runs an action and closes.
 *
 * The tree definition lives here so callers (commands, shell overlays,
 * and BlockEditor's vim binding) all read the same source.
 */

import {
  commandRegistry,
  effectiveChord,
  isHiddenOn,
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
  k: "kanban…",
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

/**
 * Resolve a bucket's label at `path` (the chord keys from the root down to
 * and including this bucket's own key): a user override
 * (`keybindings.getGroupLabel`, tesela-cmdd.4's "leader-tree regroup") wins
 * first, then the compiled-in `CHORD_GROUP_LABELS`, then a joined-children
 * fallback.
 */
function groupLabel(path: string[], childLabels: string[]): string {
  const key = path[path.length - 1];
  return (
    keybindings.getGroupLabel(path.join(" ")) ??
    CHORD_GROUP_LABELS[key] ??
    childLabels.join(" / ")
  );
}

function buildChordTree(
  commands: Command[],
  depth: number,
  ctx?: CommandContext,
  path: string[] = [],
): ChordNode[] {
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
    const childPath = [...path, key];

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
      const children = buildChordTree(branches, depth + 1, ctx, childPath);
      if (children.length > 0) {
        nodes.push({
          key,
          label: keybindings.getGroupLabel(childPath.join(" ")) ?? `${leaf.label}…`,
          children,
        });
      }
    } else {
      const children = buildChordTree(branches, depth + 1, ctx, childPath);
      if (children.length > 0) {
        nodes.push({
          key,
          label: groupLabel(
            childPath,
            children.map((c) => c.label),
          ),
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
  // The with-ctx branch goes through `availableOn`, which already filters
  // by surface AND by `isHiddenOn(cmd, 'leader', overrides)`. The no-ctx
  // branch goes through `all()`, which doesn't — so we apply the per-surface
  // hidden filter here unconditionally. With-ctx it's a defensive no-op
  // (re-checks a filter `availableOn` already ran); no-ctx it's the actual
  // gate. Keeping the check unconditional means a future change to
  // `availableOn`'s contract can't silently let a hidden command into the
  // tree (qwen review finding on tesela-cmdd.4).
  const commands = (
    ctx ? commandRegistry.availableOn('leader', ctx, overrides) : commandRegistry.all()
  ).filter((cmd) => {
    if (isHiddenOn(cmd, 'leader', overrides)) return false;
    const chord = effectiveChord(cmd, overrides);
    return chord && chord.length > 0;
  });
  return buildChordTree(commands, 0, ctx);
}
