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

export type PropertyDefinition = {
  name: string;
  value_type: PropertyType;
  choices: string[];
  default: string | null;
};

export type PropertyRegistry = Map<string, PropertyDefinition>;

export function parsePropertyPage(note: Note): PropertyDefinition | null {
  if (note.metadata.note_type !== "Property") return null;
  const c = note.metadata.custom;
  return {
    name: note.title,
    value_type: (c.value_type as PropertyType) || "text",
    choices: Array.isArray(c.choices) ? (c.choices as string[]) : [],
    default: typeof c.default === "string" ? c.default : null,
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
