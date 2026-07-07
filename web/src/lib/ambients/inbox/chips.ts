/**
 * Inbox chip registry — the bridge between the chip toolbar UI and the
 * query DSL stored on the `inbox` Query note.
 *
 * Each chip declares its identity, the DSL fragment it represents, and
 * whether it's on by default in a freshly-seeded Inbox. Toggling a chip
 * rewrites the DSL by composing the active fragments together; on load
 * we parse the DSL string back into a chip-state map so the toolbar
 * reflects what's actually saved.
 *
 * Unknown clauses (DSL fragments not matched by any registered chip)
 * survive the round-trip via {@link ChipState.unknownClauses} so the
 * UI can render them as read-only "raw" pills — the user always sees
 * everything that's filtering the inbox, even when chips don't cover
 * every clause.
 *
 * `ChipDef.clauses` now hold JQL predicate strings (tesela-vp9.3, decision
 * 5 in `.docs/ai/phases/2026-07-07-jql-authoring-spec.md`) — e.g.
 * `"status IS NULL"` instead of the old colon-DSL `"-has:status"` — so a
 * clause can be several whitespace-separated words. `chipsFromDsl`
 * therefore detects active chips PARSE-AWARE (via `view-dsl.ts`'s
 * `clausesActiveInDsl`, which compares parsed predicates, not raw tokens)
 * and strips claimed chips' spans out (via `toggleClausesInDsl`) before
 * falling back to whitespace tokenization for the remaining dynamic groups
 * (`tag-in:`/`-page:`/`-block:`) and any truly unknown clauses — those stay
 * single-token colon-DSL and are unaffected by the JQL migration.
 */
// Relative (not `$lib`) — mirrors view-dsl.ts's own import of
// query-language.ts so the node test runner can resolve this without the
// SvelteKit alias map.
import { clausesActiveInDsl, toggleClausesInDsl } from "../../views/view-dsl.ts";

/** A toggleable filter exposed in the chip toolbar. */
export type ChipDef = {
  /** Stable identifier for chip-state maps. Never user-visible. */
  id: string;
  /** Short label rendered on the chip. */
  label: string;
  /** Glyph rendered next to the label (emoji or single Unicode mark). */
  glyph: string;
  /** Compact one-line explanation, used as `title` / tooltip. */
  hint: string;
  /**
   * JQL predicate fragment(s) the chip contributes when active (e.g.
   * `"status IS NULL"`) — each element is a whole predicate, not a single
   * token. Every chip today contributes exactly one; the array shape stays
   * open for a future chip whose "on" state needs more than one predicate
   * ANDed together. Order matters for round-tripping — keep the list
   * canonical.
   */
  clauses: readonly string[];
  /**
   * Whether the chip is on by default in a freshly-seeded Inbox
   * (matters only when no saved `inbox` note exists yet — the seed
   * DSL is computed from these defaults).
   */
  defaultOn: boolean;
  /**
   * Display category — controls grouping in the chip picker. Doesn't
   * affect query semantics.
   */
  category: "scope" | "type" | "tags" | "dates";
};

export const CHIP_REGISTRY: readonly ChipDef[] = [
  // ── scope (what counts as a triage item) ────────────────────────────
  {
    id: "untriaged",
    label: "Untriaged",
    glyph: "📥",
    hint: "Only blocks without a status:: property",
    clauses: ["status IS NULL"],
    defaultOn: true,
    category: "scope",
  },
  {
    id: "notHeading",
    label: "No headings",
    glyph: "🧱",
    hint: "Hide markdown section headings (### …) — they're dividers, not tasks",
    clauses: ["is != heading"],
    defaultOn: true,
    category: "scope",
  },
  {
    id: "notDailyPage",
    label: "No daily pages",
    glyph: "📅",
    hint: "Hide blocks on YYYY-MM-DD daily notes — journal captures aren't triage items",
    clauses: ["on != daily-page"],
    defaultOn: true,
    category: "scope",
  },
  {
    id: "notSystemPages",
    label: "No system pages",
    glyph: "⚙️",
    hint: "Hide blocks on Tag / Property / Query / Template pages",
    clauses: ["on != system-pages"],
    defaultOn: true,
    category: "scope",
  },
  // ── dates (optional refinements; off by default) ─────────────────────
  {
    id: "hasScheduled",
    label: "Has scheduled",
    glyph: "🕒",
    hint: "Only blocks with a scheduled:: date",
    clauses: ["scheduled IS NOT NULL"],
    defaultOn: false,
    category: "dates",
  },
  {
    id: "hasDeadline",
    label: "Has deadline",
    glyph: "⚑",
    hint: "Only blocks with a deadline:: date",
    clauses: ["deadline IS NOT NULL"],
    defaultOn: false,
    category: "dates",
  },
  // ── tags ─────────────────────────────────────────────────────────────
  {
    id: "untagged",
    label: "Untagged",
    glyph: "🏷️",
    hint: "Only blocks without any tags",
    clauses: ["tag IS NULL"],
    defaultOn: false,
    category: "tags",
  },
];

/**
 * Live state of the chip toolbar — drives both rendering and DSL
 * composition. The fields beyond `active` capture the two dynamic
 * pieces of the saved query: a multi-select Types group (OR via a
 * single `tag-in:` clause) and per-row exclusion lists (Hide-this-
 * page / Hide-this-block, expressed as `-page:` / `-block:` clauses).
 */
export type ChipState = {
  /** Static chip on/off, keyed by `ChipDef.id`. */
  active: Record<string, boolean>;
  /**
   * Type names the user has multi-selected to include. Composed into
   * a single `tag-in:Name1,Name2,…` clause on save; an empty array
   * means the chip-group is off (no clause emitted).
   */
  activeTypes: string[];
  /** Page ids the user has explicitly hidden via "Hide all from this
   * page." Each one is emitted as `-page:<id>` in the DSL. */
  hiddenPages: string[];
  /** Block ids the user has hidden one-off via "Hide this block." */
  hiddenBlocks: string[];
  /**
   * DSL clauses present in the source query that no chip-shape claims.
   * Rendered read-only in the chip bar so the user always sees every
   * clause filtering the inbox; only editable via the raw-DSL sheet.
   */
  unknownClauses: string[];
};

/**
 * Parse a raw DSL string into chip state. Clauses owned by registered
 * chips flip those chips to active; everything else (including
 * `kind:block`, which we surface only as an implicit shape, not a
 * chip) goes into `unknownClauses` so the UI can show it verbatim.
 *
 * `kind:block` is treated as the implicit baseline and stripped from
 * unknown clauses — it's always there for the Inbox by design.
 */
export function chipsFromDsl(dsl: string): ChipState {
  const active: Record<string, boolean> = {};
  // A chip's JQL clause(s) can span several whitespace-separated words
  // (`status IS NULL`), so active-detection is parse-aware (compares
  // predicates, not tokens) and claimed chips' spans are stripped out
  // (via the SAME span-removal `toggleClausesInDsl` uses) before what's
  // left is whitespace-tokenized for the still-colon-DSL dynamic groups
  // below.
  let remainingDsl = dsl;
  for (const chip of CHIP_REGISTRY) {
    const isActive = clausesActiveInDsl(dsl, chip.clauses);
    active[chip.id] = isActive;
    if (isActive) remainingDsl = toggleClausesInDsl(remainingDsl, chip.clauses);
  }
  const remaining = new Set(tokenize(remainingDsl));
  // Strip the implicit `kind:block` baseline from unknowns.
  remaining.delete("kind:block");

  // Pull out dynamic groups: tag-in:A,B,C → activeTypes; -page:X /
  // -block:X → exclusion lists. Order doesn't matter so it's fine to
  // iterate `remaining` and remove as we claim each token.
  const activeTypes: string[] = [];
  const hiddenPages: string[] = [];
  const hiddenBlocks: string[] = [];
  for (const tok of Array.from(remaining)) {
    if (tok.startsWith("tag-in:")) {
      const values = tok.slice("tag-in:".length).split(",").map((s) => s.trim()).filter((s) => s.length > 0);
      activeTypes.push(...values);
      remaining.delete(tok);
    } else if (tok.startsWith("-page:")) {
      hiddenPages.push(tok.slice("-page:".length));
      remaining.delete(tok);
    } else if (tok.startsWith("-block:")) {
      hiddenBlocks.push(tok.slice("-block:".length));
      remaining.delete(tok);
    }
  }

  return {
    active,
    activeTypes,
    hiddenPages,
    hiddenBlocks,
    unknownClauses: Array.from(remaining),
  };
}

/**
 * Build a DSL string from a chip state. Always prepends `kind:block`
 * (the Inbox is fundamentally a block query) and appends preserved
 * `unknownClauses` so a chip-only edit can't accidentally drop user-
 * authored raw clauses.
 */
export function dslFromChips(state: ChipState): string {
  const parts: string[] = ["kind:block"];
  for (const chip of CHIP_REGISTRY) {
    if (state.active[chip.id]) {
      parts.push(...chip.clauses);
    }
  }
  if (state.activeTypes.length > 0) {
    parts.push(`tag-in:${state.activeTypes.join(",")}`);
  }
  for (const p of state.hiddenPages) {
    parts.push(`-page:${p}`);
  }
  for (const b of state.hiddenBlocks) {
    parts.push(`-block:${b}`);
  }
  for (const u of state.unknownClauses) {
    parts.push(u);
  }
  return parts.join(" ");
}

/**
 * The default DSL for a freshly-seeded Inbox query note. Derived from
 * the `defaultOn` flag of every chip in the registry so a change to
 * the registry automatically updates the seed.
 */
export function defaultInboxDsl(): string {
  const active: Record<string, boolean> = {};
  for (const chip of CHIP_REGISTRY) {
    active[chip.id] = chip.defaultOn;
  }
  return dslFromChips({
    active,
    activeTypes: [],
    hiddenPages: [],
    hiddenBlocks: [],
    unknownClauses: [],
  });
}

/**
 * Split a DSL string into whitespace-separated tokens, preserving
 * `key:value` shapes. The parse_query parser in `tesela-core` is
 * tolerant of unknown / malformed tokens, so we just split here and
 * let the server side validate.
 */
function tokenize(dsl: string): string[] {
  return dsl
    .split(/\s+/)
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}
