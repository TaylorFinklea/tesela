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
 */

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
   * DSL fragment(s) the chip contributes when active. Most chips are a
   * single token; the `notSystemPages` chip is a four-token group
   * because the "system pages" concept covers four note-type values.
   * Order matters for round-tripping — keep the list canonical.
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
    clauses: ["-has:status"],
    defaultOn: true,
    category: "scope",
  },
  {
    id: "notHeading",
    label: "No headings",
    glyph: "🧱",
    hint: "Hide markdown section headings (### …) — they're dividers, not tasks",
    clauses: ["-is:heading"],
    defaultOn: true,
    category: "scope",
  },
  {
    id: "notDailyPage",
    label: "No daily pages",
    glyph: "📅",
    hint: "Hide blocks on YYYY-MM-DD daily notes — journal captures aren't triage items",
    clauses: ["-on:daily-page"],
    defaultOn: true,
    category: "scope",
  },
  {
    id: "notSystemPages",
    label: "No system pages",
    glyph: "⚙️",
    hint: "Hide blocks on Tag / Property / Query / Template pages",
    clauses: ["-on:system-pages"],
    defaultOn: true,
    category: "scope",
  },
  // ── dates (optional refinements; off by default) ─────────────────────
  {
    id: "hasScheduled",
    label: "Has scheduled",
    glyph: "🕒",
    hint: "Only blocks with a scheduled:: date",
    clauses: ["has:scheduled"],
    defaultOn: false,
    category: "dates",
  },
  {
    id: "hasDeadline",
    label: "Has deadline",
    glyph: "⚑",
    hint: "Only blocks with a deadline:: date",
    clauses: ["has:deadline"],
    defaultOn: false,
    category: "dates",
  },
  // ── tags ─────────────────────────────────────────────────────────────
  {
    id: "untagged",
    label: "Untagged",
    glyph: "🏷️",
    hint: "Only blocks without any tags",
    clauses: ["-has:tag"],
    defaultOn: false,
    category: "tags",
  },
];

/** Map of `ChipDef.id` → whether the chip is currently active. */
export type ChipState = {
  active: Record<string, boolean>;
  /**
   * DSL clauses present in the source query that no registered chip
   * claims. Rendered read-only in the chip bar so the user always
   * sees every clause filtering the inbox.
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
  const tokens = tokenize(dsl);
  const active: Record<string, boolean> = {};
  for (const chip of CHIP_REGISTRY) {
    active[chip.id] = false;
  }
  // Walk chip registry; a chip is active iff EVERY one of its clauses
  // appears in the token list (handles multi-clause chips). When a
  // chip claims its clauses, remove them from `remaining` so they
  // don't end up in `unknownClauses`.
  const remaining = new Set(tokens);
  for (const chip of CHIP_REGISTRY) {
    if (chip.clauses.every((c) => remaining.has(c))) {
      active[chip.id] = true;
      for (const c of chip.clauses) remaining.delete(c);
    }
  }
  // Strip the implicit `kind:block` baseline from unknowns.
  remaining.delete("kind:block");
  return {
    active,
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
  return dslFromChips({ active, unknownClauses: [] });
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
