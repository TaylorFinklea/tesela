/**
 * Unified command registry — the single spine for palette (⌘K), leader
 * chords (Space), slash menu (`/`), and colon ex-mode (`:`).
 *
 * Phase B1 (2026-06-13): introduces the registry and ports palette + leader
 * to read from it. Colon mode also resolves verbs through the registry.
 *
 * Commands are registered by calling `commandRegistry.register(cmd)`.
 * Importing `web/src/lib/v4/commands.ts` automatically registers the legacy
 * V4 command set so existing consumers keep working.
 */

import { BUILTIN_SLASH_CHORDS } from "./chord-keys.ts";
import type { SlashContext } from "./editor/slash-context.ts";

export type CommandContext = {
  route?: string | null;
  bufferKind?: 'page' | 'derived' | 'ambient' | null;
  vimMode?: string | null;
  focusedBlock?: { id: string; properties: Record<string, string> } | null;
  splitOpen?: boolean;
  editor?: SlashContext;
};

export type Command = {
  id: string;
  verb?: string;
  label: string;
  glyph: string;
  category: 'pane' | 'tab' | 'tile' | 'create' | 'navigate' | 'derived' | 'ambient' | 'editor';
  shortcut?: string;
  /** Chord path, e.g. ['g','d'] for "g d" in the leader menu. */
  chord?: string[];
  surface?: 'global' | 'editor';
  slashKey?: string;
  keywords: string[];
  argPrompt?: string;
  /** Optional predicate controlling whether the command is available. */
  when?: (ctx: CommandContext) => boolean;
  run: (arg?: string, ctx?: CommandContext) => void | Promise<void>;
};

export type RegisteredCommand = Command & { registeredAt: number };

class CommandRegistry {
  private commands = new Map<string, RegisteredCommand>();
  private registrationOrder: string[] = [];

  register(cmd: Command): void {
    if (this.commands.has(cmd.id)) {
      console.warn(`command-registry: command "${cmd.id}" registered twice`);
      return;
    }
    const registered: RegisteredCommand = {
      ...cmd,
      registeredAt: Date.now(),
    };
    this.commands.set(cmd.id, registered);
    this.registrationOrder.push(cmd.id);
  }

  get(id: string): RegisteredCommand | undefined {
    return this.commands.get(id);
  }

  all(): RegisteredCommand[] {
    return this.registrationOrder.map((id) => this.commands.get(id)!);
  }

  available(ctx: CommandContext): RegisteredCommand[] {
    return this.all().filter((cmd) => {
      if (cmd.surface === 'editor' && !ctx.editor) return false;
      if (!cmd.when) return true;
      try {
        return cmd.when(ctx);
      } catch (e) {
        console.warn(`command-registry: when() threw for "${cmd.id}"`, e);
        return false;
      }
    });
  }

  findByVerb(verb: string): RegisteredCommand | undefined {
    const v = verb.toLowerCase();
    return this.all().find((c) => c.verb === v || c.id === v);
  }

  /** Reset is intended for tests only. */
  _reset(): void {
    this.commands.clear();
    this.registrationOrder = [];
  }
}

export const commandRegistry = new CommandRegistry();

// ── keymap introspection (B2) ─────────────────────────────────────────────

export type BindingConflict = {
  kind: 'shortcut' | 'chord' | 'browser-reserved';
  key: string;
  commands: RegisteredCommand[];
};

/** Static list of browser-reserved macOS shortcuts that pages should not
 *  claim, because preventDefault cannot stop them from closing/switching tabs. */
export const BROWSER_RESERVED_KEYS = new Set([
  '⌘T', '⌘W', '⌘⇧W', '⌘N', '⌘Q', '⌘R',
]);

export function buildKeymapIndex(registry: CommandRegistry = commandRegistry) {
  const shortcuts = new Map<string, RegisteredCommand[]>();
  const chords = new Map<string, RegisteredCommand[]>();

  for (const [key, label] of BUILTIN_SLASH_CHORDS) {
    chords.set(`/ ${key}`, [{
      id: `slash:${key}`,
      label,
      glyph: "/",
      category: "ambient",
      keywords: ["slash", label.toLowerCase()],
      registeredAt: 0,
      run: () => {},
    }]);
  }

  for (const cmd of registry.all()) {
    if (cmd.shortcut) {
      const list = shortcuts.get(cmd.shortcut) ?? [];
      list.push(cmd);
      shortcuts.set(cmd.shortcut, list);
    }
    if (cmd.chord && cmd.chord.length > 0) {
      const key = cmd.chord.join(' ');
      const list = chords.get(key) ?? [];
      list.push(cmd);
      chords.set(key, list);
    }
  }

  return { shortcuts, chords };
}

export function findConflicts(registry: CommandRegistry = commandRegistry): BindingConflict[] {
  const { shortcuts, chords } = buildKeymapIndex(registry);
  const conflicts: BindingConflict[] = [];

  for (const [key, commands] of shortcuts) {
    if (commands.length > 1 || BROWSER_RESERVED_KEYS.has(key)) {
      conflicts.push({
        kind: commands.length > 1 ? 'shortcut' : 'browser-reserved',
        key,
        commands,
      });
    }
  }

  for (const [key, commands] of chords) {
    if (commands.length > 1) {
      conflicts.push({ kind: 'chord', key, commands });
    }
  }

  return conflicts;
}

export function formatKeymap(registry: CommandRegistry = commandRegistry): string {
  const { shortcuts, chords } = buildKeymapIndex(registry);
  const conflicts = findConflicts(registry);
  const conflictKeys = new Set(conflicts.map((c) => `${c.kind}:${c.key}`));

  const lines: string[] = [];
  lines.push('== Command Registry Keymap ==');
  lines.push('');

  lines.push('-- shortcuts --');
  for (const [key, commands] of [...shortcuts.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    const flag = conflictKeys.has(`shortcut:${key}`) || conflictKeys.has(`browser-reserved:${key}`) ? ' ⚠' : '';
    for (const cmd of commands) {
      lines.push(`${key}${flag} → ${cmd.id} (${cmd.label})`);
    }
  }

  lines.push('');
  lines.push('-- chords --');
  for (const [key, commands] of [...chords.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    const flag = conflictKeys.has(`chord:${key}`) ? ' ⚠' : '';
    for (const cmd of commands) {
      lines.push(`${key}${flag} → ${cmd.id} (${cmd.label})`);
    }
  }

  if (conflicts.length > 0) {
    lines.push('');
    lines.push('-- conflicts --');
    for (const c of conflicts) {
      lines.push(`${c.kind}:${c.key} → ${c.commands.map((cmd) => cmd.id).join(', ')}`);
    }
  }

  return lines.join('\n');
}
