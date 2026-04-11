/**
 * Navigation history store for back/forward with [ and ] keys.
 */
let history = $state<string[]>([]);
let currentIndex = $state(-1);

export function pushNavigation(path: string) {
  // Don't push duplicate consecutive entries
  if (history[currentIndex] === path) return;
  // Truncate forward history when navigating to a new page
  history = [...history.slice(0, currentIndex + 1), path];
  currentIndex = history.length - 1;
}

export function canGoBack(): boolean {
  return currentIndex > 0;
}

export function canGoForward(): boolean {
  return currentIndex < history.length - 1;
}

export function goBack(): string | null {
  if (!canGoBack()) return null;
  currentIndex--;
  return history[currentIndex];
}

export function goForward(): string | null {
  if (!canGoForward()) return null;
  currentIndex++;
  return history[currentIndex];
}

export function getHistoryLength(): number {
  return history.length;
}
