/**
 * Deterministic per-tag color for the right-edge tag pills (the "colored
 * per-tag pills" redesign, Model A, decided 2026-06-07 — see decisions.md).
 *
 * A tag's color is a stable function of its (lowercased) name, drawn from a
 * curated palette seeded by the Graphite type-dot tokens. v1 is fully
 * deterministic + zero-config so tags are scannable across a list with no
 * setup; a future tag-page `color` frontmatter override can layer on top.
 */

export type TagColor = {
  /** Solid hue — the leading dot. */
  dot: string;
  /** Lightened hue — the label/×, for contrast on the dark tinted bg. */
  text: string;
  /** Low-alpha tint — the pill background. */
  bg: string;
};

// Curated, theme-harmonious hues (seeded from graphite/tokens.css type dots:
// task/event/note/project/person/query + the coral brand, plus a few extras
// so distinct tags get distinct colors).
const PALETTE = [
  "#E8697F", // rose / task
  "#62B8CE", // teal / event
  "#E4AE66", // amber / note
  "#7493E8", // blue / project
  "#AE90E6", // violet / person
  "#85BC63", // green / query
  "#FF6B5A", // coral (brand)
  "#E093C4", // pink
  "#6FC3A8", // mint
  "#C9A24B", // gold
] as const;

/** FNV-1a 32-bit hash → palette index. Stable across casing. */
function paletteIndex(name: string): number {
  let h = 2166136261;
  const s = name.toLowerCase();
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return Math.abs(h) % PALETTE.length;
}

// A tag PAGE may pin its own color via a `color::` frontmatter value — either a
// `#rrggbb` hex or one of these friendly names (the palette hues). When set, it
// overrides the deterministic hash so a tag's color is intentional + consistent.
const NAMED: Record<string, string> = {
  rose: "#E8697F", task: "#E8697F",
  teal: "#62B8CE", event: "#62B8CE",
  amber: "#E4AE66", note: "#E4AE66",
  blue: "#7493E8", project: "#7493E8",
  violet: "#AE90E6", purple: "#AE90E6", person: "#AE90E6",
  green: "#85BC63", query: "#85BC63",
  coral: "#FF6B5A", red: "#FF6B5A",
  pink: "#E093C4",
  mint: "#6FC3A8",
  gold: "#C9A24B", yellow: "#C9A24B",
};

/** Resolve a tag-page `color::` override to a hex, or null if unrecognized. */
function resolveOverride(color: string): string | null {
  const s = color.trim();
  if (/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(s)) return s;
  return NAMED[s.toLowerCase()] ?? null;
}

/**
 * Color for a tag pill. `override` is the tag page's `color::` value (hex or a
 * named hue); when present + valid it wins, else the color is a deterministic
 * function of the tag name.
 */
export function tagColor(name: string, override?: string | null): TagColor {
  const hex =
    (override ? resolveOverride(override) : null) ?? PALETTE[paletteIndex(name)];
  return {
    dot: hex,
    text: `color-mix(in srgb, ${hex} 72%, white)`,
    bg: `color-mix(in srgb, ${hex} 16%, transparent)`,
  };
}
