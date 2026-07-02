/**
 * Keybinding + leader-tree overrides — user-rebindable config over stable
 * command ids, persisted to localStorage AND (tesela-cmdd.4) mirrored to the
 * server so a config change survives reload on a second device.
 *
 * Tri-state per channel (shortcut / chord):
 *   - key ABSENT from map → inherit compiled-in default
 *   - value is null → explicitly unbound
 *   - value is a string/array → rebound
 *
 * `hidden` lists the surfaces (palette/colon/leader/slash) a command should
 * be hidden from — absent/empty means visible everywhere it normally would
 * be. `groupLabels` overrides the leader-tree's compiled-in bucket labels
 * (`CHORD_GROUP_LABELS` in `v5/leader-tree.svelte.ts`), keyed by the
 * space-joined chord-path prefix up to and including the bucket key (e.g.
 * `"b"` for the top-level Block bucket, `"g d"` for a nested bucket) — the
 * "leader-tree regroup" half of this bead. Moving a command BETWEEN groups
 * is already covered by rebinding its chord path via `setChord`.
 *
 * This module deliberately has NO network dependency (api-client.ts is not
 * node-importable — see its own comment — and this store is imported
 * directly by `node --test` unit tests). Server sync is wired from the
 * outside via `setSyncHook`/`hydrate`, called by a glue module that DOES
 * import api-client (see `web/src/lib/stores/keymap-sync.ts`).
 */
import type { Surface } from "../command-registry.svelte.ts";

export type BindingOverride = {
  shortcut?: string | null;
  chord?: string[] | null;
  /** Surfaces this command is hidden from. Absent/empty = hidden nowhere. */
  hidden?: Surface[];
};

/** The full server-persisted shape. Field names match the Rust
 *  `KeymapConfigDto` verbatim (snake_case, no camelCase translation layer
 *  at the JSON boundary — same convention as `CommandManifestEntry`). */
export type KeymapConfig = {
  overrides: Record<string, BindingOverride>;
  group_labels: Record<string, string>;
};

const STORAGE_KEY = "tesela:keybindings";
const GROUP_LABELS_STORAGE_KEY = "tesela:leader-group-labels";

// Check if localStorage is available
const hasLocalStorage = typeof localStorage !== 'undefined';

function loadMap(): Record<string, BindingOverride> {
  if (!hasLocalStorage) return {};
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveMap(map: Record<string, BindingOverride>) {
  if (!hasLocalStorage) return;
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(map));
  } catch {
    // localStorage full or blocked
  }
}

function loadGroupLabels(): Record<string, string> {
  if (!hasLocalStorage) return {};
  try {
    const raw = localStorage.getItem(GROUP_LABELS_STORAGE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveGroupLabels(labels: Record<string, string>) {
  if (!hasLocalStorage) return;
  try {
    localStorage.setItem(GROUP_LABELS_STORAGE_KEY, JSON.stringify(labels));
  } catch {
    // localStorage full or blocked
  }
}

let map = $state<Record<string, BindingOverride>>(loadMap());
let groupLabels = $state<Record<string, string>>(loadGroupLabels());

/** Registered by `keymap-sync.ts` at app bootstrap; fired after every local
 *  mutation (but NOT from `hydrate`, which applies server state INTO this
 *  store and must not immediately bounce a PUT back). Fire-and-forget —
 *  this module never awaits it. */
let syncHook: (() => void) | null = null;
export function setSyncHook(fn: (() => void) | null): void {
  syncHook = fn;
}

export function get(id: string): BindingOverride | undefined {
  return map[id];
}

export function setShortcut(id: string, value: string | null) {
  const existing = map[id] ?? {};
  const next = { ...map, [id]: { ...existing, shortcut: value } };
  map = next;
  saveMap(next);
  syncHook?.();
}

export function setChord(id: string, value: string[] | null) {
  const existing = map[id] ?? {};
  const next = { ...map, [id]: { ...existing, chord: value } };
  map = next;
  saveMap(next);
  syncHook?.();
}

/** Set (or clear, with an empty array) the surfaces `id` is hidden from. */
export function setHidden(id: string, surfaces: Surface[]) {
  const existing = map[id] ?? {};
  const next = { ...map, [id]: { ...existing, hidden: surfaces } };
  map = next;
  saveMap(next);
  syncHook?.();
}

/** True when `id` is configured hidden on `surface`. */
export function isHidden(id: string, surface: Surface): boolean {
  return !!map[id]?.hidden?.includes(surface);
}

export function reset(id: string) {
  const { [id]: _, ...rest } = map;
  const next = { ...rest };
  map = next;
  saveMap(next);
  syncHook?.();
}

export function resetAll() {
  map = {};
  groupLabels = {};
  if (hasLocalStorage) {
    try {
      localStorage.removeItem(STORAGE_KEY);
      localStorage.removeItem(GROUP_LABELS_STORAGE_KEY);
    } catch {
      // ignore
    }
  }
  syncHook?.();
}

/** Snapshot — used by resolvers that need a plain object. */
export function snapshot(): Record<string, BindingOverride> {
  return map;
}

// ── leader-tree group-label overrides ───────────────────────────────────

/** The user-set label for the bucket at `pathKey` (e.g. `"b"`, `"g d"`), or
 *  `undefined` to fall back to the compiled-in `CHORD_GROUP_LABELS`. */
export function getGroupLabel(pathKey: string): string | undefined {
  return groupLabels[pathKey];
}

export function setGroupLabel(pathKey: string, label: string) {
  const next = { ...groupLabels, [pathKey]: label };
  groupLabels = next;
  saveGroupLabels(next);
  syncHook?.();
}

export function resetGroupLabel(pathKey: string) {
  const { [pathKey]: _, ...rest } = groupLabels;
  groupLabels = rest;
  saveGroupLabels(rest);
  syncHook?.();
}

export function groupLabelsSnapshot(): Record<string, string> {
  return groupLabels;
}

// ── server persistence (tesela-cmdd.4) ──────────────────────────────────

/** The whole config, in the server's wire shape. */
export function wholeConfig(): KeymapConfig {
  return { overrides: map, group_labels: groupLabels };
}

/** Replace local state with server-authoritative config (does NOT fire
 *  `syncHook` — this is the inbound path, not a local edit). Also refreshes
 *  the localStorage cache so a subsequent offline boot sees the latest
 *  known-good config. */
export function hydrate(config: KeymapConfig): void {
  map = config.overrides ?? {};
  groupLabels = config.group_labels ?? {};
  saveMap(map);
  saveGroupLabels(groupLabels);
}
