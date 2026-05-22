/**
 * Theme registry. Each entry maps a theme id to display metadata and the
 * swatch colors used in the picker UI. The actual CSS variable overrides
 * live in `themes.css` keyed by `[data-theme="<id>"]`.
 *
 * The role-token contract every theme provides:
 *   --bg, --bg-2, --bg-3, --bg-4
 *   --line, --line-soft
 *   --fg-default, --fg-muted, --fg-subtle, --fg-faint
 *   --accent-primary, --accent-secondary
 *   --type-task, --type-event, --type-note, --type-project, --type-person, --type-query, --type-template
 *   --theme-font-sans, --theme-font-mono   (optional per-theme overrides)
 */

export type ThemeMode = "dark" | "light";

export interface ThemeMeta {
  id: string;
  name: string;
  mode: ThemeMode;
  swatch: { bg: string; fg: string; primary: string; secondary: string };
}

export const THEMES: ThemeMeta[] = [
  // === Darks ===
  { id: "prism",                name: "Prism",                  mode: "dark", swatch: { bg: "#23252f", fg: "#f4f1de", primary: "#e07a5f", secondary: "#81b29a" } },
  { id: "prism-spark",          name: "Prism Spark",            mode: "dark", swatch: { bg: "#23252f", fg: "#f4f1de", primary: "#fb5950", secondary: "#81b29a" } },
  { id: "tokyo-night",          name: "Tokyo Night",            mode: "dark", swatch: { bg: "#1a1b26", fg: "#c0caf5", primary: "#ff9e64", secondary: "#bb9af7" } },
  { id: "tokyo-night-storm",    name: "Tokyo Night Storm",      mode: "dark", swatch: { bg: "#24283b", fg: "#c0caf5", primary: "#ff9e64", secondary: "#bb9af7" } },
  { id: "catppuccin-mocha",     name: "Catppuccin Mocha",       mode: "dark", swatch: { bg: "#1e1e2e", fg: "#cdd6f4", primary: "#fab387", secondary: "#cba6f7" } },
  { id: "catppuccin-macchiato", name: "Catppuccin Macchiato",   mode: "dark", swatch: { bg: "#24273a", fg: "#cad3f5", primary: "#f5a97f", secondary: "#c6a0f6" } },
  { id: "catppuccin-frappe",    name: "Catppuccin Frappe",      mode: "dark", swatch: { bg: "#303446", fg: "#c6d0f5", primary: "#ef9f76", secondary: "#ca9ee6" } },
  { id: "gruvbox-dark",         name: "Gruvbox Dark",           mode: "dark", swatch: { bg: "#282828", fg: "#ebdbb2", primary: "#fe8019", secondary: "#d3869b" } },
  { id: "gruvbox-material",     name: "Gruvbox Material Dark",  mode: "dark", swatch: { bg: "#1d2021", fg: "#d4be98", primary: "#e78a4e", secondary: "#d3869b" } },
  { id: "rose-pine",            name: "Rose Pine",              mode: "dark", swatch: { bg: "#191724", fg: "#e0def4", primary: "#ebbcba", secondary: "#c4a7e7" } },
  { id: "rose-pine-moon",       name: "Rose Pine Moon",         mode: "dark", swatch: { bg: "#232136", fg: "#e0def4", primary: "#ea9a97", secondary: "#c4a7e7" } },
  { id: "nord",                 name: "Nord",                   mode: "dark", swatch: { bg: "#2e3440", fg: "#eceff4", primary: "#88c0d0", secondary: "#b48ead" } },
  { id: "nordic",               name: "Nordic",                 mode: "dark", swatch: { bg: "#242933", fg: "#eceff4", primary: "#88c0d0", secondary: "#b48ead" } },
  { id: "dracula",              name: "Dracula",                mode: "dark", swatch: { bg: "#282a36", fg: "#f8f8f2", primary: "#ff79c6", secondary: "#bd93f9" } },
  { id: "dracula-pro",          name: "Dracula Pro",            mode: "dark", swatch: { bg: "#22212c", fg: "#f8f8f2", primary: "#ff80bf", secondary: "#9580ff" } },
  { id: "nightfox",             name: "Nightfox",               mode: "dark", swatch: { bg: "#192330", fg: "#cdcecf", primary: "#f4a261", secondary: "#9d79d6" } },
  { id: "duskfox",              name: "Duskfox",                mode: "dark", swatch: { bg: "#232136", fg: "#e0def4", primary: "#ea9a97", secondary: "#c4a7e7" } },
  { id: "carbonfox",            name: "Carbonfox",              mode: "dark", swatch: { bg: "#161616", fg: "#f2f4f8", primary: "#ff7eb6", secondary: "#be95ff" } },
  { id: "everforest-dark",      name: "Everforest Dark",        mode: "dark", swatch: { bg: "#2d353b", fg: "#d3c6aa", primary: "#e69875", secondary: "#d699b6" } },
  { id: "kanagawa-wave",        name: "Kanagawa Wave",          mode: "dark", swatch: { bg: "#1f1f28", fg: "#dcd7ba", primary: "#ffa066", secondary: "#957fb8" } },
  { id: "kanagawa-dragon",      name: "Kanagawa Dragon",        mode: "dark", swatch: { bg: "#181616", fg: "#c5c9c5", primary: "#b6927b", secondary: "#a292a3" } },
  { id: "one-dark",             name: "One Dark",               mode: "dark", swatch: { bg: "#282c34", fg: "#abb2bf", primary: "#d19a66", secondary: "#c678dd" } },
  { id: "palenight",            name: "Material Palenight",     mode: "dark", swatch: { bg: "#292d3e", fg: "#a6accd", primary: "#f78c6c", secondary: "#c792ea" } },
  { id: "monokai-pro",          name: "Monokai Pro",            mode: "dark", swatch: { bg: "#2d2a2e", fg: "#fcfcfa", primary: "#fc9867", secondary: "#ab9df2" } },
  { id: "solarized-dark",       name: "Solarized Dark",         mode: "dark", swatch: { bg: "#002b36", fg: "#93a1a1", primary: "#cb4b16", secondary: "#6c71c4" } },
  { id: "ayu-dark",             name: "Ayu Dark",               mode: "dark", swatch: { bg: "#0b0e14", fg: "#bfbdb6", primary: "#ff8f40", secondary: "#d2a6ff" } },

  // === Lights ===
  { id: "prism-light",          name: "Prism Light",            mode: "light", swatch: { bg: "#f4f1de", fg: "#3d405b", primary: "#bd5e40", secondary: "#5c9078" } },
  { id: "tokyo-night-day",      name: "Tokyo Night Day",        mode: "light", swatch: { bg: "#e1e2e7", fg: "#3760bf", primary: "#b15c00", secondary: "#7847bd" } },
  { id: "catppuccin-latte",     name: "Catppuccin Latte",       mode: "light", swatch: { bg: "#eff1f5", fg: "#4c4f69", primary: "#fe640b", secondary: "#8839ef" } },
  { id: "rose-pine-dawn",       name: "Rose Pine Dawn",         mode: "light", swatch: { bg: "#faf4ed", fg: "#575279", primary: "#d7827e", secondary: "#907aa9" } },
];

export const DEFAULT_THEME = "prism";

export function isKnownTheme(id: string): boolean {
  return THEMES.some((t) => t.id === id);
}
