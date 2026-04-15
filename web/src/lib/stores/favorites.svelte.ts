/**
 * Favorite notes — persisted in localStorage.
 */
import { browser } from "$app/environment";

const STORAGE_KEY = "tesela:favorites";

function loadFavorites(): string[] {
  if (!browser) return [];
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    return stored ? JSON.parse(stored) : [];
  } catch {
    return [];
  }
}

let favorites = $state<string[]>(loadFavorites());

export function getFavorites(): string[] {
  return favorites;
}

export function isFavorite(noteId: string): boolean {
  return favorites.includes(noteId);
}

export function toggleFavorite(noteId: string) {
  if (favorites.includes(noteId)) {
    favorites = favorites.filter((id) => id !== noteId);
  } else {
    favorites = [noteId, ...favorites];
  }
  if (browser) {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(favorites));
    } catch {
      // localStorage full or blocked — ignore
    }
  }
}
