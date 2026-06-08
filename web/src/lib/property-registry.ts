/**
 * Property type registry — reads definitions from Property pages (notes with type: "Property").
 * No server changes required: value_type and choices live in frontmatter, surfaced via NoteMetadata.custom.
 */
import type { Note } from "$lib/types/Note";

export type PropertyType =
  | "text"
  | "number"
  | "select"
  | "multi-select"
  | "date"
  | "checkbox"
  | "url"
  | "email"
  | "phone"
  | "object";

export const PROPERTY_TYPE_LABELS: Record<PropertyType, string> = {
  text: "Text",
  number: "Number",
  select: "Select",
  "multi-select": "Multi-select",
  date: "Date",
  checkbox: "Checkbox",
  url: "URL",
  email: "Email",
  phone: "Phone",
  object: "Object",
};

/**
 * Phase 10.6 — chip-display config. Lives on the Property page so every
 * surface that pins this property as a chip (per-tag `display_chips`) gets
 * the same visualization. `null` for any field means "use the derived
 * default" — an icon-set property defaults to icon-only label mode, etc.
 */
export type ChipLabelMode = "full" | "short" | "icon" | "none";
export type ChipValueFormat = "value" | "month-day" | "iso" | "bars" | "truncate";

export type PropertyDefinition = {
  name: string;
  value_type: PropertyType;
  choices: string[];
  default: string | null;
  /** If true, hide this property from the block by default — only show when
   *  the user expands the block's properties via the chevron. */
  hide_by_default: boolean;
  /** If true, only render the property line when its value is non-empty. */
  hide_empty: boolean;
  /** Tabler icon name (`"calendar"`, `"clock"`, …) or raw emoji (`"📅"`).
   *  Resolved at render time by `icon-registry.resolveChipIcon`. */
  chip_icon: string | null;
  /** How to label this property in chip form. Default: `"icon"` if
   *  `chip_icon` is set, else `"full"`. */
  chip_label_mode: ChipLabelMode | null;
  /** Used when `chip_label_mode === "short"`; falls back to first 4 chars
   *  of property name. */
  chip_short_label: string | null;
  /** How to render the value. Type-aware (date → month-day; select → bars
   *  / value; etc). `null` means use the type's default. */
  chip_value_format: ChipValueFormat | null;
  /** Phase 12.2 — preferred single-letter chord key for this property,
   *  honored across the slash menu, BottomDrawer, and any other chord
   *  surface. When omitted, falls back to first-letter assignment. When
   *  declared but already claimed by an earlier sibling, the conflict is
   *  surfaced in the chord menu so the user knows to renamekey or
   *  rename one of the colliding properties. */
  chord_key: string | null;
  /** Per-choice canonical chord keys (for `select` / `multi-select`).
   *  Same fallback + conflict semantics as `chord_key`, but for the value
   *  submenu. Frontmatter shape: `value_chord_keys: { backlog: b, todo: t }`. */
  value_chord_keys: Record<string, string>;
  /** Model B — natural-language triggers for inline detection (lowercased).
   *  Meaning is value-type-driven: select → the value tokens (`["p1","p2"]`);
   *  date → leading keyword(s) (`["due","deadline"]`); number → adjacent word
   *  (`["points","pts"]`). Empty → not NL-detected (unless it's the tag's
   *  default date property). Frontmatter: `nl_triggers: ["due","deadline"]`. */
  nl_triggers: string[];
};

export type PropertyRegistry = Map<string, PropertyDefinition>;

export function parsePropertyPage(note: Note): PropertyDefinition | null {
  if (note.metadata.note_type !== "Property") return null;
  const c = note.metadata.custom;
  const labelMode = typeof c.chip_label_mode === "string" ? c.chip_label_mode as ChipLabelMode : null;
  const valueFormat = typeof c.chip_value_format === "string" ? c.chip_value_format as ChipValueFormat : null;
  // chord_key is single-letter; coerce to the first letter of whatever
  // string the user typed (so "S" / "status" / "s" all read as "s") and
  // lowercase it, since the chord menu compares lowercase keys.
  const chordKeyRaw = typeof c.chord_key === "string" ? c.chord_key : null;
  const chordKey = chordKeyRaw && chordKeyRaw.length > 0 ? chordKeyRaw[0].toLowerCase() : null;
  // value_chord_keys is `{ choice: letter }` — coerce each value to a
  // single lowercase letter and drop any non-letter entries so a typo
  // doesn't strand a choice with a broken chord.
  const valueChordKeys: Record<string, string> = {};
  const vck = c.value_chord_keys;
  if (vck && typeof vck === "object" && !Array.isArray(vck)) {
    for (const [k, v] of Object.entries(vck as Record<string, unknown>)) {
      if (typeof v !== "string" || v.length === 0) continue;
      const ch = v[0].toLowerCase();
      if (/[a-z]/.test(ch)) valueChordKeys[k.toLowerCase()] = ch;
    }
  }
  return {
    name: note.title,
    value_type: (c.value_type as PropertyType) || "text",
    choices: Array.isArray(c.choices) ? (c.choices as string[]) : [],
    default: typeof c.default === "string" ? c.default : null,
    hide_by_default: c.hide_by_default === true,
    hide_empty: c.hide_empty !== false, // default true
    chip_icon: typeof c.chip_icon === "string" ? c.chip_icon : null,
    chip_label_mode: labelMode,
    chip_short_label: typeof c.chip_short_label === "string" ? c.chip_short_label : null,
    chip_value_format: valueFormat,
    chord_key: chordKey,
    value_chord_keys: valueChordKeys,
    nl_triggers: Array.isArray(c.nl_triggers)
      ? (c.nl_triggers as unknown[]).filter((t): t is string => typeof t === "string").map((t) => t.toLowerCase())
      : [],
  };
}

export function buildRegistry(notes: Note[]): PropertyRegistry {
  const m = new Map<string, PropertyDefinition>();
  for (const n of notes) {
    const def = parsePropertyPage(n);
    if (def) m.set(def.name.toLowerCase(), def);
  }
  return m;
}

/**
 * Returns choices for a select property, minus any choices hidden for a specific tag.
 * hiddenMap keys are property names (any case); values are hidden choice strings.
 */
export function getVisibleChoices(
  def: PropertyDefinition,
  hiddenMap: Record<string, string[]>,
): string[] {
  const hidden = new Set(
    (hiddenMap[def.name] ?? hiddenMap[def.name.toLowerCase()] ?? []).map((v) =>
      v.toLowerCase(),
    ),
  );
  return def.choices.filter((c) => !hidden.has(c.toLowerCase()));
}

/**
 * Parses hidden choices from a tag page's metadata.custom.
 * Keys are stored as hidden_{PropertyName}: ["val1", "val2"].
 */
export function parseHiddenChoices(custom: Record<string, unknown>): Record<string, string[]> {
  const result: Record<string, string[]> = {};
  for (const [key, val] of Object.entries(custom)) {
    if (key.startsWith("hidden_") && Array.isArray(val)) {
      result[key.slice(7)] = val as string[];
    }
  }
  return result;
}

/** Maps tagName (lowercase) → parent tagName (lowercase). Built from Tag pages with `extends:` in frontmatter. */
export type InheritanceMap = Map<string, string>;

export function buildInheritanceMap(notes: Note[]): InheritanceMap {
  const m = new Map<string, string>();
  for (const n of notes) {
    if (n.metadata.note_type !== "Tag") continue;
    const ext = n.metadata.custom.extends;
    if (typeof ext === "string" && ext.trim()) {
      m.set(n.title.toLowerCase(), ext.trim().toLowerCase());
    }
  }
  return m;
}

/** Returns the full ancestor chain for a tag, starting with itself. Cycle-safe (max 10 hops). */
export function resolveTagChain(tagName: string, inheritance: InheritanceMap): string[] {
  const chain: string[] = [tagName.toLowerCase()];
  let current = tagName.toLowerCase();
  for (let i = 0; i < 10; i++) {
    const parent = inheritance.get(current);
    if (!parent || chain.includes(parent)) break;
    chain.push(parent);
    current = parent;
  }
  return chain;
}

/**
 * Resolves a tag's full property definition list, walking the extends chain
 * and looking each property name up against the registry of Property pages.
 * Deduplicated by lowercased property name.
 */
export function getTagPropertyDefs(
  tagName: string,
  notes: Note[],
  registry: PropertyRegistry,
  inheritance: InheritanceMap,
): PropertyDefinition[] {
  const seen = new Set<string>();
  const out: PropertyDefinition[] = [];
  for (const tag of resolveTagChain(tagName, inheritance)) {
    const tagPage = notes.find(
      (n) => n.title.toLowerCase() === tag && n.metadata.note_type === "Tag",
    );
    if (!tagPage) continue;
    const tagProps = tagPage.metadata.custom.tag_properties;
    if (!Array.isArray(tagProps)) continue;
    for (const propName of tagProps as string[]) {
      const def = registry.get(String(propName).toLowerCase());
      if (def && !seen.has(def.name.toLowerCase())) {
        seen.add(def.name.toLowerCase());
        out.push(def);
      }
    }
  }
  return out;
}

/**
 * Updates or inserts a key in YAML frontmatter.
 * value must already be serialized (e.g. `"select"` or `["a", "b"]`).
 */
export function updateFrontmatterKey(content: string, key: string, value: string): string {
  if (!content.startsWith("---")) return content;
  const closeIdx = content.indexOf("\n---", 3);
  if (closeIdx === -1) return content;

  const fmBody = content.slice(4, closeIdx); // between "---\n" and "\n---"
  const after = content.slice(closeIdx);
  const re = new RegExp(`^${escapeRe(key)}:.*$`, "m");
  const newLine = `${key}: ${value}`;

  const newFm = re.test(fmBody) ? fmBody.replace(re, newLine) : fmBody.trimEnd() + "\n" + newLine;
  return "---\n" + newFm + after;
}

/** Removes a frontmatter key line entirely. */
export function removeFrontmatterKey(content: string, key: string): string {
  const re = new RegExp(`^${escapeRe(key)}:.*\n?`, "m");
  return content.replace(re, "");
}

/** Serializes a string array as inline YAML: ["a", "b", "c"] */
export function serializeStringArray(arr: string[]): string {
  return `[${arr.map((s) => `"${s}"`).join(", ")}]`;
}

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
