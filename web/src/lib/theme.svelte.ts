/**
 * Reactive theme state. The current theme id is mirrored to the
 * `data-theme` attribute on <html>, which the CSS in `themes.css` keys off.
 *
 * FOUC prevention: an inline script in `app.html` sets `data-theme` from
 * localStorage before any module loads. This module then takes ownership.
 */

import { browser } from "$app/environment";
import { DEFAULT_THEME, isKnownTheme, THEMES, type ThemeMeta } from "$lib/themes";

const STORAGE_KEY = "tesela:theme";

function readStored(): string {
  if (!browser) return DEFAULT_THEME;
  const v = localStorage.getItem(STORAGE_KEY);
  return v && isKnownTheme(v) ? v : DEFAULT_THEME;
}

class ThemeStore {
  current = $state<string>(readStored());

  meta = $derived<ThemeMeta>(
    THEMES.find((t) => t.id === this.current) ?? THEMES[0],
  );

  set(id: string): void {
    if (!isKnownTheme(id)) return;
    this.current = id;
    if (!browser) return;
    localStorage.setItem(STORAGE_KEY, id);
    const html = document.documentElement;
    html.setAttribute("data-theme", id);
    const m = THEMES.find((t) => t.id === id);
    if (m) {
      html.classList.toggle("dark", m.mode === "dark");
      html.classList.toggle("light", m.mode === "light");
    }
  }
}

export const theme = new ThemeStore();
