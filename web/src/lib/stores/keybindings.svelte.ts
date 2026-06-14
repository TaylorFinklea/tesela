/**
 * Keybinding overrides — user-rebindable shortcuts persisted to localStorage.
 *
 * Tri-state per channel (shortcut / chord):
 *   - key ABSENT from map → inherit compiled-in default
 *   - value is null → explicitly unbound
 *   - value is a string/array → rebound
 */
export type BindingOverride = {
  shortcut?: string | null;
  chord?: string[] | null;
};

const STORAGE_KEY = "tesela:keybindings";

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

let map = $state<Record<string, BindingOverride>>(loadMap());

export function get(id: string): BindingOverride | undefined {
  return map[id];
}

export function setShortcut(id: string, value: string | null) {
  const existing = map[id] ?? {};
  const next = { ...map, [id]: { ...existing, shortcut: value } };
  map = next;
  saveMap(next);
}

export function setChord(id: string, value: string[] | null) {
  const existing = map[id] ?? {};
  const next = { ...map, [id]: { ...existing, chord: value } };
  map = next;
  saveMap(next);
}

export function reset(id: string) {
  const { [id]: _, ...rest } = map;
  const next = { ...rest };
  map = next;
  saveMap(next);
}

export function resetAll() {
  map = {};
  if (hasLocalStorage) {
    try {
      localStorage.removeItem(STORAGE_KEY);
    } catch {
      // ignore
    }
  }
}

/** Snapshot — used by resolvers that need a plain object. */
export function snapshot(): Record<string, BindingOverride> {
  return map;
}
