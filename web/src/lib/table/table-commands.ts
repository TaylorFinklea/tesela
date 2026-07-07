/**
 * tesela-ya4.3 — QueryTable's actions registered in the unified command
 * registry so they are reachable via ⌘K palette AND the leader chord menu
 * (the cmdd spine), not only via the table's own in-pane keydown handler.
 * Mirrors `kanban/kanban-commands.ts` (tesela-ya4.2) exactly.
 *
 * tesela-ya4.4 adds hide/unhide-all/move-column-left/move-column-right
 * (gap G5 — column display config persistence) alongside the pre-existing
 * sort, which now persists too (see `table-config.ts`/`QueryTable.svelte`).
 *
 * The DIRECT key bindings (j/k/h/l/g/G/0/$/Enter/e/i/s/x/U/H/L) live in
 * `QueryTable.svelte::handleTableKeydown`. Those + these registry entries
 * route to the SAME handlers: each registered command's `run` dispatches a
 * `tesela:run-table-command` CustomEvent carrying its id, and the focused
 * table listens + dispatches to its handler (mirrors the
 * `tesela:run-kanban-command` bridge KanbanBoard uses).
 *
 * Chords live under a fresh `s` leader prefix ("sets" — the product name
 * for this render mode) — `s` is not used as a first-key bucket by any
 * existing built-in, editor, or kanban command.
 *
 * Registered via `registerBuiltinCommands()` (see `commands/index.ts`) so the
 * manifest generator + freshness check pick them up automatically — no
 * script edits needed. Idempotent (guard below) so re-calling it on layout
 * re-init is a no-op, matching `registerKanbanCommands`'s contract.
 */
import {
  commandRegistry,
  type Command,
  type Surface,
} from "../command-registry.svelte.ts";
import { isTableFocused } from "./table-focus.svelte.ts";

let tableCommandsRegistered = false;

/** Dispatch a `tesela:run-table-command` event carrying `id`. The focused
 *  QueryTable listens (only while `focused`) and routes to its handler.
 *  Browser-only (palette/leader/colon all fire on real key/click), but
 *  guarded for SSR + the manifest generator (which never calls `run`). */
function dispatchTable(id: string): void {
  if (typeof document === "undefined") return;
  document.dispatchEvent(
    new CustomEvent("tesela:run-table-command", { detail: { id } }),
  );
}

/** Shared keyword set so palette search hits every table verb from "table"
 *  or "sets" without restating it on each entry. */
const TABLE_KW = ["table", "sets", "columns"];

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
    // New category so the palette + keymap overlay group table actions
    // together. The Rust manifest stores category as a plain String, so no
    // Rust-side change is needed — just the TS union in command-registry.
    category: "table",
    // cmd-K palette + leader chord (per acceptance). NOT slash/colon to
    // keep table actions scoped to the surfaces where keyboard users look.
    surfaces: new Set<Surface>(["palette", "leader"]),
    chord,
    keywords: [...TABLE_KW, ...keywords],
    // Admitted only while a table owns focus — otherwise these would
    // clutter the palette/leader for every other surface.
    when: () => isTableFocused(),
    run: () => dispatchTable(runId),
  };
}

/** Register the query table's command set. Idempotent — safe to call more
 *  than once (the root layout re-runs on certain navigations). */
export function registerTableCommands(): void {
  if (tableCommandsRegistered) return;

  const commands: Command[] = [
    // ── row navigation (mirror the direct j/k/g/G keys) ──
    cmd(
      "table.focus-down",
      "Table: focus row below",
      "↓",
      ["s", "j"],
      ["focus", "down", "next", "row"],
      "table.focus-down",
    ),
    cmd(
      "table.focus-up",
      "Table: focus row above",
      "↑",
      ["s", "k"],
      ["focus", "up", "prev", "row"],
      "table.focus-up",
    ),
    cmd(
      "table.focus-first-row",
      "Table: focus first row",
      "⇱",
      ["s", "g"],
      ["focus", "first", "top", "row"],
      "table.focus-first-row",
    ),
    cmd(
      "table.focus-last-row",
      "Table: focus last row",
      "⇲",
      ["s", "G"],
      ["focus", "last", "bottom", "row"],
      "table.focus-last-row",
    ),
    // ── column navigation (mirror the direct h/l/0/$ keys) ──
    cmd(
      "table.focus-left",
      "Table: focus previous column",
      "←",
      ["s", "h"],
      ["focus", "left", "prev", "column"],
      "table.focus-left",
    ),
    cmd(
      "table.focus-right",
      "Table: focus next column",
      "→",
      ["s", "l"],
      ["focus", "right", "next", "column"],
      "table.focus-right",
    ),
    cmd(
      "table.focus-first-col",
      "Table: focus first column",
      "⇤",
      ["s", "0"],
      ["focus", "first", "column"],
      "table.focus-first-col",
    ),
    cmd(
      "table.focus-last-col",
      "Table: focus last column",
      "⇥",
      ["s", "$"],
      ["focus", "last", "column"],
      "table.focus-last-col",
    ),
    // ── row/cell actions (mirror Enter / e / i) ──
    cmd(
      "table.open-row",
      "Table: open focused row",
      "↵",
      ["s", "o"],
      ["open", "goto", "jump", "row"],
      "table.open-row",
    ),
    cmd(
      "table.edit-cell",
      "Table: edit focused cell",
      "✎",
      ["s", "e"],
      ["edit", "cell", "property", "value"],
      "table.edit-cell",
    ),
    cmd(
      "table.edit-properties",
      "Table: edit focused row properties",
      "⚙",
      ["s", "i"],
      ["edit", "properties", "drawer", "inspect"],
      "table.edit-properties",
    ),
    // ── sort (acceptance: sortable header, keyboard-reachable; persists
    // via updateView/localStorage as of ya4.4 — see table-config.ts) ──
    cmd(
      "table.sort-column",
      "Table: sort by focused column",
      "⇅",
      ["s", "s"],
      ["sort", "column", "toggle", "order"],
      "table.sort-column",
    ),
    // ── column display config (ya4.4, gap G5) ──
    cmd(
      "table.hide-column",
      "Table: hide focused column",
      "⊘",
      ["s", "x"],
      ["hide", "column", "remove"],
      "table.hide-column",
    ),
    cmd(
      "table.unhide-all-columns",
      "Table: unhide all columns",
      "◎",
      ["s", "U"],
      ["unhide", "show", "column", "reset"],
      "table.unhide-all-columns",
    ),
    cmd(
      "table.move-column-left",
      "Table: move focused column left",
      "◀",
      ["s", "H"],
      ["move", "column", "prev", "left", "reorder"],
      "table.move-column-left",
    ),
    cmd(
      "table.move-column-right",
      "Table: move focused column right",
      "▶",
      ["s", "L"],
      ["move", "column", "next", "right", "reorder"],
      "table.move-column-right",
    ),
  ];

  for (const c of commands) {
    commandRegistry.register(c);
  }
  tableCommandsRegistered = true;
}
