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

export type CommandContext = {
  route?: string;
  bufferKind?: 'page' | 'derived' | 'ambient' | null;
  vimMode?: 'normal' | 'insert' | 'visual' | null;
  focusedBlock?: { id: string; properties: Record<string, string> } | null;
  splitOpen?: boolean;
};

export type Command = {
  id: string;
  verb?: string;
  label: string;
  glyph: string;
  category: 'pane' | 'tab' | 'tile' | 'create' | 'navigate' | 'derived' | 'ambient';
  shortcut?: string;
  /** Chord path, e.g. ['g','d'] for "g d" in the leader menu. */
  chord?: string[];
  keywords: string[];
  argPrompt?: string;
  /** Optional predicate controlling whether the command is available. */
  when?: (ctx: CommandContext) => boolean;
  run: (arg?: string) => void | Promise<void>;
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
