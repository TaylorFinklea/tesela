/**
 * tesela-ya4.2 — KanbanBoard actions registered in the unified command
 * registry so they are reachable via ⌘K palette AND the leader chord menu
 * (the cmdd spine), not only via the board's own in-Pane keydown handler.
 *
 * The DIRECT key bindings (j/k/h/l/g/G/Enter/m/H/L/i + the new s/S/c) live
 * in `KanbanBoard.svelte::handleKanbanKeydown`. Those + these registry
 * entries route to the SAME handlers: each registered command's `run`
 * dispatches a `tesela:run-kanban-command` CustomEvent carrying its id, and
 * the focused board listens + dispatches to its handler (mirroring the
 * `tesela:run-editor-command` bridge the leader uses for editor commands —
 * see `BlockEditor.svelte`). This keeps the command metadata
 * (id/label/chord/glyph/when) closure-free + identical to the manifest the
 * Rust `GET /commands` route serves (`command-manifest.json`), while the
 * behavior stays bound to the live board instance.
 *
 * Chords live under a fresh `k` leader prefix (kanban) — `k` is not used as
 * a first-key bucket by any v4 or editor command, so the whole board action
 * set groups cleanly under one leader subtree ("kanban…").
 *
 * Registered via `registerBuiltinCommands()` (see `v4/commands.ts`) so the
 * manifest generator + freshness check pick them up automatically — no
 * script edits needed. Idempotent (guard below) so re-calling it on layout
 * re-init is a no-op, matching `buildV4Commands`'s contract.
 */
import {
  commandRegistry,
  type Command,
  type Surface,
} from "../command-registry.svelte.ts";
import { isKanbanFocused } from "./kanban-focus.svelte.ts";

let kanbanCommandsRegistered = false;

/** Dispatch a `tesela:run-kanban-command` event carrying `id`. The focused
 *  KanbanBoard listens (only while `focused`) and routes to its handler.
 *  Browser-only (palette/leader/colon all fire on real key/click), but
 *  guarded for SSR + the manifest generator (which never calls `run`). */
function dispatchKanban(id: string): void {
  if (typeof document === "undefined") return;
  document.dispatchEvent(
    new CustomEvent("tesela:run-kanban-command", { detail: { id } }),
  );
}

/** Shared keyword set so palette search hits every kanban verb from "kanban"
 *  without restating it on each entry. */
const KANBAN_KW = ["kanban", "board"];

function cmd(
  id: string,
  label: string,
  glyph: string,
  chord: string[],
  keywords: string[],
  runId: string,
): Command {
  return {
    id,
    label,
    glyph,
    // New category so the palette + keymap overlay group board actions
    // together. The Rust manifest stores category as a plain String, so no
    // Rust-side change is needed — just the TS union in command-registry.
    category: "kanban",
    // cmd-K palette + leader chord (per acceptance). NOT slash/colon to
    // keep board actions scoped to the surfaces where keyboard users look.
    surfaces: new Set<Surface>(["palette", "leader"]),
    chord,
    keywords: [...KANBAN_KW, ...keywords],
    // Admitted only while a board owns focus — otherwise these would
    // clutter the palette/leader for every other surface.
    when: () => isKanbanFocused(),
    run: () => dispatchKanban(runId),
  };
}

/** Register the kanban board's command set. Idempotent — safe to call more
 *  than once (the root layout re-runs on certain navigations). */
export function registerKanbanCommands(): void {
  if (kanbanCommandsRegistered) return;

  const commands: Command[] = [
    // ── group-by switch (acceptance: change group-by from keyboard) ──
    cmd(
      "kanban.cycle-group-by",
      "Kanban: cycle group-by forward",
      "⇋",
      ["k", "s"],
      ["group", "switch", "cycle", "groupby"],
      "kanban.cycle-group-by",
    ),
    cmd(
      "kanban.cycle-group-by-back",
      "Kanban: cycle group-by backward",
      "⇌",
      ["k", "S"],
      ["group", "switch", "cycle", "groupby", "back"],
      "kanban.cycle-group-by-back",
    ),
    // ── new card into the focused column (acceptance) ──
    cmd(
      "kanban.new-card",
      "Kanban: new card in focused column",
      "✚",
      ["k", "n"],
      ["new", "create", "card", "add"],
      "kanban.new-card",
    ),
    // ── focus navigation (mirror the direct j/k/h/l/g/G keys) ──
    cmd(
      "kanban.focus-down",
      "Kanban: focus card below",
      "↓",
      ["k", "j"],
      ["focus", "down", "next"],
      "kanban.focus-down",
    ),
    cmd(
      "kanban.focus-up",
      "Kanban: focus card above",
      "↑",
      ["k", "k"],
      ["focus", "up", "prev"],
      "kanban.focus-up",
    ),
    cmd(
      "kanban.focus-left",
      "Kanban: focus previous column",
      "←",
      ["k", "h"],
      ["focus", "left", "prev", "column"],
      "kanban.focus-left",
    ),
    cmd(
      "kanban.focus-right",
      "Kanban: focus next column",
      "→",
      ["k", "l"],
      ["focus", "right", "next", "column"],
      "kanban.focus-right",
    ),
    cmd(
      "kanban.focus-first",
      "Kanban: focus first card in column",
      "⇱",
      ["k", "g"],
      ["focus", "first", "top"],
      "kanban.focus-first",
    ),
    cmd(
      "kanban.focus-last",
      "Kanban: focus last card in column",
      "⇲",
      ["k", "G"],
      ["focus", "last", "bottom"],
      "kanban.focus-last",
    ),
    // ── card actions (mirror Enter / m / H / L / i) ──
    cmd(
      "kanban.open-card",
      "Kanban: open focused card",
      "↵",
      ["k", "o"],
      ["open", "goto", "jump", "card"],
      "kanban.open-card",
    ),
    cmd(
      "kanban.open-move-picker",
      "Kanban: move focused card…",
      "⇄",
      ["k", "m"],
      ["move", "picker", "column"],
      "kanban.open-move-picker",
    ),
    cmd(
      "kanban.move-card-prev-col",
      "Kanban: move focused card to previous column",
      "◀",
      ["k", "H"],
      ["move", "card", "prev", "left", "column"],
      "kanban.move-card-prev-col",
    ),
    cmd(
      "kanban.move-card-next-col",
      "Kanban: move focused card to next column",
      "▶",
      ["k", "L"],
      ["move", "card", "next", "right", "column"],
      "kanban.move-card-next-col",
    ),
    cmd(
      "kanban.edit-properties",
      "Kanban: edit focused card properties",
      "⚙",
      ["k", "i"],
      ["edit", "properties", "drawer", "inspect"],
      "kanban.edit-properties",
    ),
  ];

  for (const c of commands) {
    commandRegistry.register(c);
  }
  kanbanCommandsRegistered = true;
}
