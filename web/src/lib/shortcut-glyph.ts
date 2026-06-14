/**
 * Shortcut glyph normalizer — converts a KeyboardEvent to the exact glyph
 * string used by Command.shortcut in the registry (e.g. "⌘⇧K", "⌘\\").
 *
 * Modifier order: ⌃ ⌥ ⌘ ⇧ (Control, Alt, Meta, Shift).
 * Single-character keys are UPPERCASED. Bare `\` and `-` kept literal.
 * Returns null when no modifier is held (not a shortcut).
 */

const MODIFIER_ORDER = [
  { flag: "ctrlKey", glyph: "\u2303" },   // ⌃
  { flag: "altKey", glyph: "\u2325" },    // ⌥
  { flag: "metaKey", glyph: "\u2318" },   // ⌘
  { flag: "shiftKey", glyph: "\u21E7" },  // ⇧
] as const;

const MODIFIER_KEY_NAMES = new Set(["Control", "Alt", "Meta", "Shift"]);

export function eventToShortcutGlyph(e: KeyboardEvent): string | null {
  const ctrl = e.ctrlKey;
  const alt = e.altKey;
  const meta = e.metaKey;
  const shift = e.shiftKey;

  // Not a shortcut if no modifier held
  if (!ctrl && !alt && !meta) return null;

  // Ignore pure modifier key presses
  if (MODIFIER_KEY_NAMES.has(e.key)) return null;

  let prefix = "";
  if (ctrl) prefix += "\u2303";
  if (alt) prefix += "\u2325";
  if (meta) prefix += "\u2318";
  if (shift) prefix += "\u21E7";

  const key = e.key;
  let char: string;
  if (key === "\\") {
    char = "\\";
  } else if (key === "-") {
    char = "-";
  } else if (key.length === 1) {
    char = key.toUpperCase();
  } else {
    // Named keys like ArrowUp, Enter, etc. — not in our current shortcut set
    char = key;
  }

  return prefix + char;
}
