/**
 * Property type registry — reads definitions from Property pages (notes with type: "Property").
 * No server changes required: value_type and choices live in frontmatter, surfaced via NoteMetadata.custom.
 */
import type { Note } from "$lib/types/Note";
import type { Visibility } from "$lib/types/Visibility";

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
  /** Per-type visibility (`on_new`/`on_set`/`hidden`), resolved from the
   *  type's `property_overrides` (`show`) or derived from `hide_by_default`
   *  when no override exists. `null` only when this PropertyDefinition was
   *  produced outside the per-type resolver (i.e. straight off `buildRegistry`,
   *  not `getTagPropertyDefs`) — mirrors the Rust `Option<Visibility>`. */
  show: Visibility | null;
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
  /** Per-choice display color (for `select` / `multi-select`) — keyed by the
   *  choice value (lowercased), valued as a CSS color (hex `#7CB342`, an
   *  `rgb(...)`, or a theme token like `var(--primary)`). Absent / empty map =
   *  today's default muted chip. Back-compat sibling key on the Property page:
   *  `choice_colors: { done: "#7CB342", blocked: "#E8697F" }`. Phase 4. */
  choice_colors: Record<string, string>;
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
  // choice_colors is `{ choice: cssColor }` — keys lowercased so a lookup
  // matches regardless of the value's stored case; values kept verbatim (any
  // non-empty string is a valid CSS color or theme token). A non-object or a
  // non-string entry is dropped rather than erroring (Property page = user
  // content, mirror of the value_chord_keys tolerance above).
  const choiceColors: Record<string, string> = {};
  const cc = c.choice_colors;
  if (cc && typeof cc === "object" && !Array.isArray(cc)) {
    for (const [k, v] of Object.entries(cc as Record<string, unknown>)) {
      if (typeof v === "string" && v.trim() !== "") choiceColors[k.toLowerCase()] = v.trim();
    }
  }
  return {
    name: note.title,
    value_type: (c.value_type as PropertyType) || "text",
    choices: Array.isArray(c.choices) ? (c.choices as string[]) : [],
    default: typeof c.default === "string" ? c.default : null,
    // `show` is set to a concrete Visibility only on the per-type resolver
    // path (`getTagPropertyDefs`); a bare registry def carries null, exactly
    // like the Rust `PropertyDef.show` produced by `get_all_property_defs`.
    show: null,
    hide_by_default: c.hide_by_default === true,
    hide_empty: c.hide_empty !== false, // default true
    chip_icon: typeof c.chip_icon === "string" ? c.chip_icon : null,
    chip_label_mode: labelMode,
    chip_short_label: typeof c.chip_short_label === "string" ? c.chip_short_label : null,
    chip_value_format: valueFormat,
    chord_key: chordKey,
    value_chord_keys: valueChordKeys,
    choice_colors: choiceColors,
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
 * A resolved per-type property override (mirror of the Rust `PropOverride`,
 * `sqlite.rs:31-41`). `choices` REPLACEs the global choice list; `hide_choices`
 * SUBTRACTs from the (possibly replaced) list; `default`/`show` override the
 * global config. `choices === null` means "no choices override" (mirror of
 * `Option<Vec<String>>`); `hide_choices` defaults to `[]`.
 */
export type PropOverride = {
  choices: string[] | null;
  default: string | null;
  show: Visibility | null;
  hide_choices: string[];
};

/**
 * Parse one override object (`{choices: [...], show: "on_new", default: "todo",
 * hide_choices: [...]}`) into a PropOverride. Mirrors `parse_prop_override`
 * (`sqlite.rs:46-77`): unknown/malformed fields are ignored rather than
 * erroring — a Tag page is user content. A non-object value yields an empty
 * override.
 */
export function parsePropOverride(v: unknown): PropOverride {
  const empty: PropOverride = { choices: null, default: null, show: null, hide_choices: [] };
  if (!v || typeof v !== "object" || Array.isArray(v)) return empty;
  const obj = v as Record<string, unknown>;
  const strArray = (val: unknown): string[] =>
    Array.isArray(val) ? val.filter((e): e is string => typeof e === "string") : [];
  const showRaw = typeof obj.show === "string" ? obj.show : null;
  const show: Visibility | null =
    showRaw === "on_new" || showRaw === "on_set" || showRaw === "hidden" ? showRaw : null;
  return {
    // `choices` present (any type) → Some; coerced to a string array (mirror
    // of `obj.get("choices").map(str_array)` — a present-but-non-array
    // `choices` becomes `Some([])`, matching the Rust `str_array` fallback).
    choices: "choices" in obj ? strArray(obj.choices) : null,
    default: typeof obj.default === "string" ? obj.default : null,
    show,
    hide_choices: strArray(obj.hide_choices),
  };
}

/**
 * Build the resolved override map for a tag by walking its rows in
 * child→parent order. Keys are `lower(prop)`; FIRST-INSERT-WINS so a child
 * override beats a parent's (a distinct pass from the name dedup — §3.5).
 * Mirrors `build_overrides` (`sqlite.rs:87-119`).
 *
 * Each row is `(property_overrides object, hidden_{Prop} pairs)` in the chain
 * walk order (child first). The `hide_choices` subtract list is ADDITIVE: both
 * child and parent `hide_choices`/`hidden_{Prop}` accumulate (deduped by value),
 * while the OTHER fields (choices/default/show) honor first-insert-wins.
 *
 * The legacy `hidden_{Prop}` fold is the ONE intentional Rust/TS asymmetry: the
 * Rust DB layer has no `hidden_{Prop}` column (so `legacy_hidden_pairs()` is a
 * no-op shim there), but the TS registry reads frontmatter directly and MUST
 * fold those keys into the same property's subtract list — both engines end up
 * subtracting identically (§3.3, prompt note).
 */
export function buildOverrides(
  rows: Array<{ overrides: Record<string, unknown>; hidden: Record<string, string[]> }>,
): Map<string, PropOverride> {
  const map = new Map<string, PropOverride>();
  for (const { overrides, hidden } of rows) {
    // property_overrides.{Prop} — first-insert-wins (child rows come first).
    if (overrides && typeof overrides === "object" && !Array.isArray(overrides)) {
      for (const [prop, val] of Object.entries(overrides)) {
        const key = prop.toLowerCase();
        if (!map.has(key)) map.set(key, parsePropOverride(val));
      }
    }
    // Legacy hidden_{Prop}: alias for property_overrides.{Prop}.hide_choices.
    // Additive subtract regardless of first-insert-wins on the other fields.
    for (const [prop, vals] of Object.entries(hidden)) {
      const key = prop.toLowerCase();
      let entry = map.get(key);
      if (!entry) {
        entry = { choices: null, default: null, show: null, hide_choices: [] };
        map.set(key, entry);
      }
      for (const h of vals) {
        if (!entry.hide_choices.includes(h)) entry.hide_choices.push(h);
      }
    }
  }
  return map;
}

/**
 * Apply a resolved override to a single PropertyDefinition, returning a new
 * def (the registry def is shared, so never mutate it). Mirrors `apply_override`
 * (`sqlite.rs:143-176`) precedence EXACTLY:
 *   a. choices REPLACE (if override.choices is non-null)
 *   b. then SUBTRACT hide_choices (only when there IS a choice list — a global
 *      `choices: []` registry default is an empty list, so subtract is a no-op
 *      there; mirrors Rust `if let Some(vals)`).
 *   c. default override wins.
 *   d. show: override wins; else derive — hide_by_default → "hidden", else "on_new".
 * `show` is ALWAYS set to a concrete Visibility on this path.
 */
export function applyOverride(
  def: PropertyDefinition,
  over: PropOverride | undefined,
  hideByDefault: boolean,
): PropertyDefinition {
  let choices = def.choices;
  if (over) {
    if (over.choices !== null) {
      choices = over.choices.slice();
    }
    if (over.hide_choices.length > 0) {
      const hidden = new Set(over.hide_choices);
      choices = choices.filter((c) => !hidden.has(c));
    }
  }
  const show: Visibility = over?.show ?? (hideByDefault ? "hidden" : "on_new");
  return {
    ...def,
    choices,
    default: over?.default ?? def.default,
    show,
  };
}

/**
 * Resolves a tag's full property definition list, walking the extends chain
 * and looking each property name up against the registry of Property pages.
 * Deduplicated by lowercased property name.
 *
 * Mirror of the Rust `get_resolved_tag_def` (`sqlite.rs:708-823`): membership
 * (`tag_properties`) is the union along the chain deduped child-first; the
 * per-type override merge is a SEPARATE pass (`buildOverrides`, child-wins
 * first-insert) applied per resolved property via `applyOverride`. The merged
 * result equals what the Rust resolver returns for the same input.
 */
export function getTagPropertyDefs(
  tagName: string,
  notes: Note[],
  registry: PropertyRegistry,
  inheritance: InheritanceMap,
): PropertyDefinition[] {
  const chain = resolveTagChain(tagName, inheritance);

  // SEPARATE override pass (§3.5): walk rows child→parent, first-insert-wins.
  // The TS side folds BOTH property_overrides AND the legacy hidden_{Prop}
  // frontmatter keys (the one intentional Rust/TS asymmetry — see buildOverrides).
  const overrideRows: Array<{
    overrides: Record<string, unknown>;
    hidden: Record<string, string[]>;
  }> = [];
  for (const tag of chain) {
    const tagPage = notes.find(
      (n) => n.title.toLowerCase() === tag && n.metadata.note_type === "Tag",
    );
    if (!tagPage) continue;
    const c = tagPage.metadata.custom;
    const rawOverrides =
      c.property_overrides && typeof c.property_overrides === "object" && !Array.isArray(c.property_overrides)
        ? (c.property_overrides as Record<string, unknown>)
        : {};
    overrideRows.push({ overrides: rawOverrides, hidden: parseHiddenChoices(c) });
  }
  const overrides = buildOverrides(overrideRows);

  // Membership pass: union along the chain, deduped child-first.
  const seen = new Set<string>();
  const out: PropertyDefinition[] = [];
  for (const tag of chain) {
    const tagPage = notes.find(
      (n) => n.title.toLowerCase() === tag && n.metadata.note_type === "Tag",
    );
    if (!tagPage) continue;
    const tagProps = tagPage.metadata.custom.tag_properties;
    if (!Array.isArray(tagProps)) continue;
    for (const propName of tagProps as string[]) {
      const key = String(propName).toLowerCase();
      if (seen.has(key)) continue;
      seen.add(key);
      const over = overrides.get(key);
      const def = registry.get(key);
      if (def) {
        // Property page exists — apply override to its global config.
        out.push(applyOverride(def, over, def.hide_by_default));
      } else {
        // §3.5c — no global Property page: a text-stub def (value_type "text",
        // empty choices, null default) still receives the override.
        const stub: PropertyDefinition = {
          name: String(propName),
          value_type: "text",
          choices: [],
          default: null,
          show: null,
          hide_by_default: false,
          hide_empty: true,
          chip_icon: null,
          chip_label_mode: null,
          chip_short_label: null,
          chip_value_format: null,
          chord_key: null,
          value_chord_keys: {},
          choice_colors: {},
          nl_triggers: [],
        };
        out.push(applyOverride(stub, over, false));
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

  // Function replacement (NOT a string): a string second-arg to String.replace
  // interprets `$&`, `` $` ``, `$'`, `$N` in the replacement — and `value` is
  // arbitrary user text (a property choice, default, or plural like "$$$"),
  // which would otherwise splice the matched line / surrounding frontmatter
  // into itself and corrupt the file.
  const newFm = re.test(fmBody) ? fmBody.replace(re, () => newLine) : fmBody.trimEnd() + "\n" + newLine;
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

/**
 * Serialize a per-choice color map to single-line compact JSON — valid FLOW
 * YAML, so it round-trips through the server's gray_matter `pod_to_json` back
 * into `metadata.custom.choice_colors`. Written under the `choice_colors`
 * Property-page key via `updateFrontmatterKey`. Empty/whitespace values are
 * dropped; returns `null` when nothing remains so the caller can
 * `removeFrontmatterKey` instead of writing `choice_colors: {}`. The key case
 * is preserved as passed (the parse lowercases on read). Phase 4.
 */
export function serializeChoiceColors(map: Record<string, string>): string | null {
  const cleaned: Record<string, string> = {};
  for (const [choice, color] of Object.entries(map)) {
    if (typeof color === "string" && color.trim() !== "") cleaned[choice] = color.trim();
  }
  if (Object.keys(cleaned).length === 0) return null;
  return JSON.stringify(cleaned);
}

/**
 * The raw, on-disk shape of one `property_overrides.{Prop}` entry as edited by
 * the config UI. Distinct from the resolved `PropOverride`: every field is
 * OPTIONAL and only written when the user has actually overridden it. An empty
 * object `{}` means "inherit everything" and should be dropped from the map
 * rather than serialized, so we never bake an inherited value into an override
 * (spec §3.5 tail). `choices: []` (present, empty) is treated as "no override"
 * — an empty list inherits the global, per §3.1 ("empty list = inherit").
 */
export type RawPropOverride = {
  choices?: string[];
  show?: Visibility;
  default?: string;
  hide_choices?: string[];
};

/**
 * Read the raw `property_overrides` map straight off a Tag page's
 * `metadata.custom` (NOT the flattened/resolved PropertyDef). Returns a map
 * keyed by the property name AS WRITTEN (original case preserved) so the editor
 * can distinguish overridden-vs-inherited and round-trip without case drift.
 * Malformed entries are coerced to `{}`.
 */
export function parsePropertyOverridesRaw(
  custom: Record<string, unknown>,
): Record<string, RawPropOverride> {
  const out: Record<string, RawPropOverride> = {};
  const raw = custom.property_overrides;
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return out;
  for (const [prop, val] of Object.entries(raw as Record<string, unknown>)) {
    if (!val || typeof val !== "object" || Array.isArray(val)) {
      out[prop] = {};
      continue;
    }
    const obj = val as Record<string, unknown>;
    const entry: RawPropOverride = {};
    if (Array.isArray(obj.choices)) {
      entry.choices = obj.choices.filter((e): e is string => typeof e === "string");
    }
    if (
      obj.show === "on_new" ||
      obj.show === "on_set" ||
      obj.show === "hidden"
    ) {
      entry.show = obj.show;
    }
    if (typeof obj.default === "string") entry.default = obj.default;
    if (Array.isArray(obj.hide_choices)) {
      entry.hide_choices = obj.hide_choices.filter((e): e is string => typeof e === "string");
    }
    out[prop] = entry;
  }
  return out;
}

/**
 * Normalize one raw override entry: drop fields that mean "inherit" so an
 * empty/inherited value is never persisted as an override (spec §3.5 tail).
 * `choices: []` and `hide_choices: []` collapse to absent; an empty/whitespace
 * `default` collapses to absent. Returns `null` when nothing remains (the
 * whole entry should be dropped from the map).
 */
export function normalizeRawOverride(entry: RawPropOverride): RawPropOverride | null {
  const out: RawPropOverride = {};
  if (entry.choices && entry.choices.length > 0) out.choices = entry.choices;
  if (entry.show) out.show = entry.show;
  if (typeof entry.default === "string" && entry.default.trim() !== "") {
    out.default = entry.default;
  }
  if (entry.hide_choices && entry.hide_choices.length > 0) {
    out.hide_choices = entry.hide_choices;
  }
  return Object.keys(out).length > 0 ? out : null;
}

/**
 * Serialize the whole `property_overrides` map to a single-line, compact JSON
 * string — which is VALID FLOW YAML, so it round-trips through the server's
 * gray_matter parser (`pod_to_json`) back into a nested `metadata.custom` map.
 * Written under the `property_overrides` key via `updateFrontmatterKey`.
 * Entries that normalize to nothing are dropped. Returns `null` when the whole
 * map is empty — the caller should `removeFrontmatterKey` instead of writing
 * `property_overrides: {}`.
 */
export function serializePropertyOverrides(
  map: Record<string, RawPropOverride>,
): string | null {
  const cleaned: Record<string, RawPropOverride> = {};
  for (const [prop, entry] of Object.entries(map)) {
    const norm = normalizeRawOverride(entry);
    if (norm) cleaned[prop] = norm;
  }
  if (Object.keys(cleaned).length === 0) return null;
  return JSON.stringify(cleaned);
}

function escapeRe(s: string): string {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
