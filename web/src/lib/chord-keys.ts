/**
 * Phase 12.2 — single-letter chord assignment shared by every chord-menu
 * surface (slash menu's `/p` submenu, top-level slash, BottomDrawer property
 * jump, BottomDrawer value commit).
 *
 * Algorithm: a two-pass greedy.
 *   1. Walk the items in order. Honor each item's `preferred` key when set
 *      and not yet claimed.
 *   2. For items without a preferred key (or whose preferred collided),
 *      walk the label's letters and pick the first unclaimed one. Falls
 *      back to digits 1-9, then `?`.
 *
 * Conflicts are not silently resolved: the second comer that wanted a
 * letter already taken gets `conflictWith` filled in with the name of the
 * earlier owner so the chord menu can render a "taken by X" warning.
 */

export type ChordItem = { name: string; preferred?: string | null };
export type ChordAssignment = { name: string; key: string; conflictWith?: string };
export type ChordOptions = {
  /**
   * Letters that callers want to keep off-limits regardless of preferred
   * declarations (e.g. BottomDrawer's nav keys j/k/h/l). Items declaring a
   * reserved letter as their preferred chord get treated as "no preferred"
   * and fall through to first-letter assignment without surfacing a
   * conflict (since there's no sibling to point at).
   */
  reserved?: ReadonlySet<string>;
};

export function assignChords(items: ChordItem[], opts: ChordOptions = {}): ChordAssignment[] {
  const reserved = opts.reserved ?? EMPTY;
  const taken = new Map<string, string>(); // key → owner name
  const out: ChordAssignment[] = [];

  // Preferred keys are case-preserving: `T` (Shift+t) and `t` are distinct
  // chord keys, matching how the runtime menu compares e.key. Fallback keys
  // are always lowercase for predictability.
  const passOneIdx: (string | null)[] = items.map((it) => {
    if (!it.preferred) return null;
    const k = it.preferred[0] ?? null;
    if (!k || !/[A-Za-z]/.test(k) || reserved.has(k)) return null;
    if (taken.has(k)) return null;
    taken.set(k, it.name);
    return k;
  });

  for (let i = 0; i < items.length; i++) {
    const it = items[i];
    const claimed = passOneIdx[i];
    if (claimed) {
      out.push({ name: it.name, key: claimed });
      continue;
    }
    let conflictWith: string | undefined;
    if (it.preferred) {
      const k = it.preferred[0];
      if (k && taken.has(k)) conflictWith = taken.get(k);
    }
    const key = pickFallback(it.name, taken, reserved, it.name);
    out.push(conflictWith ? { name: it.name, key, conflictWith } : { name: it.name, key });
  }
  return out;
}

const EMPTY: ReadonlySet<string> = new Set();

/**
 * Well-known reserved keys per chord surface. Exposed so config UIs can
 * warn the user when they declare a chord that would be ignored at one of
 * these surfaces. Keep these in sync with their owning components:
 *   - SLASH_RESERVED_CHORDS: `i` is the chord menu's filter trigger (see
 *     ChordMenu.svelte). Property pages declaring `chord_key: i` will
 *     fall through silently in the slash menu.
 *   - DRAWER_RESERVED_CHORDS: BottomDrawer nav keys (j/k/h/l/x/g). A
 *     value_chord_keys entry using one of these would shadow nav.
 *   - BUILTIN_SLASH_CHORDS: hard-coded chord keys for the slash menu's
 *     non-property verbs (Task, Heading, …). A property declaring one
 *     of these as its chord_key collides with the verb at the top level.
 */
export const SLASH_RESERVED_CHORDS: ReadonlySet<string> = new Set(["i"]);
export const DRAWER_RESERVED_CHORDS: ReadonlySet<string> = new Set(["j", "k", "h", "l", "x", "g"]);
export const BUILTIN_SLASH_CHORDS: ReadonlyMap<string, string> = new Map([
  ["t", "Task"],
  ["T", "Tag picker"],
  ["h", "Heading"],
  ["l", "Link"],
  ["d", "Date"],
  ["q", "Query"],
  ["c", "Collection"],
  ["m", "Template"],
  // Phase C dropped:
  //   - `w` (New widget) — widget now lives in the leader's `n` bucket.
  //   - `p` (All properties) — `p` is now the single context-aware
  //     Properties entry in the slash tree (not a top-level builtin).
]);

function pickFallback(label: string, taken: Map<string, string>, reserved: ReadonlySet<string>, ownerName: string): string {
  const lower = label.toLowerCase();
  for (const ch of lower) {
    if (/[a-z]/.test(ch) && !taken.has(ch) && !reserved.has(ch)) {
      taken.set(ch, ownerName);
      return ch;
    }
  }
  for (let i = 1; i <= 9; i++) {
    const k = String(i);
    if (!taken.has(k)) {
      taken.set(k, ownerName);
      return k;
    }
  }
  return "?";
}
