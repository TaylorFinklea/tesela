/**
 * Recently viewed notes — persisted in localStorage.
 */
import { browser } from "$app/environment";

const STORAGE_KEY = "tesela:recents";
const MAX_RECENTS = 10;

function loadRecents(): string[] {
  if (!browser) return [];
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return stored ? JSON.parse(stored) : [];
  } catch {
    return [];
  }
}

let recents = $state<string[]>(loadRecents());

export function getRecents(): string[] {
  return recents;
}

export function addRecent(noteId: string) {
  recents = [noteId, ...recents.filter((id) => id !== noteId)].slice(0, MAX_RECENTS);
  if (browser) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(recents));
    } catch {
      // localStorage full or blocked — ignore
    }
  }
}
