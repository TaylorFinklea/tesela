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
import { eventToShortcutGlyph } from "./shortcut-glyph.ts";

export type Surface = 'slash' | 'colon' | 'leader' | 'palette';

export type CommandContext = {
  route?: string | null;
  bufferKind?: 'page' | 'derived' | 'ambient' | null;
  vimMode?: string | null;
  focusedBlock?: { id: string; properties: Record<string, string> } | null;
  splitOpen?: boolean;
  editor?: SlashContext;
  /**
   * True when a BlockEditor is focused but its full `editor` SlashContext is
   * NOT on this ctx (the leader path — the shell can't build one). Lets
   * `available()` admit `surface:'editor'` commands so they populate the
   * leader's i/p buckets; execution routes via the `tesela:run-editor-command`
   * event to the focused editor, which supplies the real context.
   */
  editorFocused?: boolean;
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
  surfaces?: ReadonlySet<Surface>;
  keywords: string[];
  argPrompt?: string;
  /** Optional predicate controlling whether the command is available. */
  when?: (ctx: CommandContext) => boolean;
  run: (arg?: string, ctx?: CommandContext) => void | Promise<void>;
};

export type RegisteredCommand = Command & { registeredAt: number };

/**
 * True outside a Vite production build. `import.meta.env.DEV` is statically
 * `false` only in a real production build; every other context (Vite dev
 * server, SSR, and plain `node --test` where `import.meta.env` doesn't exist
 * at all) is treated as dev so a duplicate-id bug fails loud in tests too.
 */
function isDevEnv(): boolean {
  return (import.meta as { env?: { DEV?: boolean } }).env?.DEV !== false;
}

class CommandRegistry {
  private commands = new Map<string, RegisteredCommand>();
  private registrationOrder: string[] = [];

  register(cmd: Command): void {
    if (this.commands.has(cmd.id)) {
      const msg = `command-registry: command "${cmd.id}" registered twice`;
      if (isDevEnv()) {
        throw new Error(msg);
      }
      console.warn(msg);
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
      if (cmd.surface === 'editor' && !ctx.editor && !ctx.editorFocused) return false;
      if (!cmd.when) return true;
      try {
        return cmd.when(ctx);
      } catch (e) {
        console.warn(`command-registry: when() threw for "${cmd.id}"`, e);
        return false;
      }
    });
  }

  availableOn(surface: Surface, ctx: CommandContext): RegisteredCommand[] {
    return this.available(ctx).filter((cmd) => surfacesFor(cmd).has(surface));
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

/**
 * True when `query` matches a command's label, verb/id, or keywords
 * (case-insensitive substring). Powers palette + colon suggestion filtering
 * — the single matcher both surfaces share instead of each reaching around
 * the registry for their own copy.
 */
export function matchesCommand(cmd: Command | RegisteredCommand, query: string): boolean {
  if (!query) return true;
  const q = query.toLowerCase();
  if (cmd.label.toLowerCase().includes(q)) return true;
  if (cmd.verb && cmd.verb.toLowerCase().includes(q)) return true;
  return cmd.keywords.some((kw) => kw.includes(q));
}

/**
 * Per-surface visibility for a command. When `cmd.surfaces` is set it is
 * authoritative; otherwise derive back-compat defaults from today's fields so
 * Phase A is a no-op until later phases set explicit `surfaces`.
 */
export function surfacesFor(cmd: Command | RegisteredCommand): ReadonlySet<Surface> {
  if (cmd.surfaces) return cmd.surfaces;
  const out = new Set<Surface>();
  if (cmd.surface === 'editor') {
    // editor command — slash menu always; ALSO the leader when it has a chord,
    // so Space → i/p can run it on the focused block. The leader dispatches
    // `tesela:run-editor-command` and the focused BlockEditor supplies the real
    // ctx.editor (the shell only carries ctx.editorFocused, which available()
    // honors so these commands aren't dropped from the leader tree).
    out.add('slash');
    if (cmd.chord && cmd.chord.length > 0) out.add('leader');
    return out;
  }
  if (cmd.surface === 'global') {
    // surface 'global' leaks everywhere today
    out.add('slash');
    out.add('palette');
    out.add('colon');
    out.add('leader');
    return out;
  }
  // surface unset → visible to palette + colon today.
  out.add('palette');
  out.add('colon');
  if (cmd.slashKey) out.add('slash');
  if (cmd.chord && cmd.chord.length > 0) out.add('leader');
  return out;
}

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

/**
 * Keybinding override type for user-rebindable shortcuts.
 * Tri-state per channel: key ABSENT = inherit compiled-in default;
 * null = explicitly unbound; a value = rebound.
 */
export type BindingOverride = {
  shortcut?: string | null;
  chord?: string[] | null;
};

/**
 * Returns the effective shortcut for a command, considering overrides.
 * - If overrides[id] has shortcut key present (even if null), use that (null → undefined)
 * - Otherwise, fall back to cmd.shortcut
 */
export function effectiveShortcut(
  cmd: Command | RegisteredCommand,
  overrides: Record<string, BindingOverride>
): string | undefined {
  const override = overrides[cmd.id];
  if (override && 'shortcut' in override) {
    return override.shortcut ?? undefined;
  }
  return cmd.shortcut;
}

/**
 * Returns the effective chord for a command, considering overrides.
 * - If overrides[id] has chord key present (even if null), use that (null → undefined)
 * - Otherwise, fall back to cmd.chord
 */
export function effectiveChord(
  cmd: Command | RegisteredCommand,
  overrides: Record<string, BindingOverride>
): string[] | undefined {
  const override = overrides[cmd.id];
  if (override && 'chord' in override) {
    return override.chord ?? undefined;
  }
  return cmd.chord;
}

export function buildKeymapIndex(
  registry: CommandRegistry = commandRegistry,
  overrides: Record<string, BindingOverride> = {}
) {
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
    const shortcut = effectiveShortcut(cmd, overrides);
    if (shortcut) {
      const list = shortcuts.get(shortcut) ?? [];
      list.push(cmd);
      shortcuts.set(shortcut, list);
    }
    const chord = effectiveChord(cmd, overrides);
    if (chord && chord.length > 0) {
      const key = chord.join(' ');
      const list = chords.get(key) ?? [];
      list.push(cmd);
      chords.set(key, list);
    }
  }

  return { shortcuts, chords };
}

/**
 * Resolve a keyboard event to a command based on effective shortcuts.
 * Returns the first available command whose effective shortcut matches the event,
 * or undefined if no match or if the key is browser-reserved.
 */
export function resolveShortcut(
  e: KeyboardEvent,
  ctx: CommandContext,
  overrides: Record<string, BindingOverride>
): RegisteredCommand | undefined {
  const glyph = eventToShortcutGlyph(e);
  if (!glyph) return undefined;
  
  // Skip browser-reserved keys
  if (BROWSER_RESERVED_KEYS.has(glyph)) return undefined;
  
  // Find the first available command whose effective shortcut matches
  const available = commandRegistry.available(ctx);
  for (const cmd of available) {
    if (effectiveShortcut(cmd, overrides) === glyph) {
      return cmd;
    }
  }
  return undefined;
}

/**
 * Validate a pending rebind of `cmdId`'s `kind` channel to `key`. Three-tier:
 *  - `reserved` — a browser-reserved shortcut (`preventDefault` can't intercept
 *    it, so the binding would be dead) → hard block.
 *  - `taken` — another command already holds this effective binding → soft warn
 *    (`by` lists the holders; the caller may "rebind anyway", last-writer-wins).
 *  - `ok` — free.
 * Probes against (current overrides + the pending rebind) so a key already
 * moved off another command by an override doesn't false-positive.
 */
export function checkRebind(
  cmdId: string,
  kind: 'shortcut' | 'chord',
  key: string,
  overrides: Record<string, BindingOverride>
):
  | { ok: true }
  | { ok: false; reason: 'reserved' }
  | { ok: false; reason: 'taken'; by: RegisteredCommand[] } {
  if (kind === 'shortcut' && BROWSER_RESERVED_KEYS.has(key)) {
    return { ok: false, reason: 'reserved' };
  }
  const pending: BindingOverride = {
    ...(overrides[cmdId] ?? {}),
    [kind]: kind === 'chord' ? key.split(' ') : key,
  };
  const { shortcuts, chords } = buildKeymapIndex(commandRegistry, {
    ...overrides,
    [cmdId]: pending,
  });
  const index = kind === 'shortcut' ? shortcuts : chords;
  const holders = (index.get(key) ?? []).filter((c) => c.id !== cmdId);
  if (holders.length > 0) {
    return { ok: false, reason: 'taken', by: holders };
  }
  return { ok: true };
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
