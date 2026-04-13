/**
 * Tesela "Warm Study" theme system.
 * Two polished themes: Day (warm cream) and Evening (warm charcoal).
 */

export type ThemeId = "day" | "evening";

export interface ThemeDefinition {
  id: ThemeId;
  name: string;
  description: string;
  isDark: boolean;
  vars: Record<string, string>;
}

export const themes: ThemeDefinition[] = [
  {
    id: "day",
    name: "Day",
    description: "Warm cream, golden light, like quality paper",
    isDark: false,
    vars: {
      "--background": "#faf7f2",
      "--foreground": "#2c2824",
      "--surface": "#f3ede4",
      "--surface-2": "#ebe4d8",
      "--muted": "#e8e0d4",
      "--muted-foreground": "#8a8078",
      "--accent": "#f0e8dc",
      "--accent-foreground": "#2c2824",
      "--primary": "#c4852c",
      "--primary-foreground": "#faf7f2",
      "--destructive": "#c45a4a",
      "--border": "#e8e0d4",
      "--ring": "#c4852c",
      "--popover": "#faf7f2",
      "--popover-foreground": "#2c2824",
      "--block-bg": "#ffffff",
      "--block-border": "#e8e0d4",
      "--block-radius": "10px",
      "--block-shadow": "0 1px 3px rgba(120,90,50,0.06)",
      "--focus-glow": "0 0 0 3px rgba(196,133,44,0.12)",
      "--thread-border": "#e8e0d4",
    },
  },
  {
    id: "evening",
    name: "Evening",
    description: "Warm charcoal, amber lamplight, cozy depth",
    isDark: true,
    vars: {
      "--background": "#1e1c24",
      "--foreground": "#e8e0d4",
      "--surface": "#17151c",
      "--surface-2": "#24222c",
      "--muted": "#2a2830",
      "--muted-foreground": "#9a918a",
      "--accent": "#2a2830",
      "--accent-foreground": "#e8e0d4",
      "--primary": "#d4a04a",
      "--primary-foreground": "#1e1c24",
      "--destructive": "#d46a5a",
      "--border": "rgba(255,240,210,0.08)",
      "--ring": "#d4a04a",
      "--popover": "#24222c",
      "--popover-foreground": "#e8e0d4",
      "--block-bg": "#24222c",
      "--block-border": "rgba(255,240,210,0.06)",
      "--block-radius": "10px",
      "--block-shadow": "0 1px 3px rgba(0,0,0,0.2)",
      "--focus-glow": "0 0 0 3px rgba(212,160,74,0.15)",
      "--thread-border": "rgba(255,240,210,0.06)",
    },
  },
];

export function getTheme(id: string): ThemeDefinition {
  return themes.find((t) => t.id === id) ?? themes[0];
}

export function applyTheme(id: string) {
  const theme = getTheme(id);
  const root = document.documentElement;

  // Set CSS variables
  for (const [key, value] of Object.entries(theme.vars)) {
    root.style.setProperty(key, value);
  }

  // Toggle dark class for Tailwind's dark: variant
  if (theme.isDark) {
    root.classList.add("dark");
  } else {
    root.classList.remove("dark");
  }

  root.setAttribute("data-theme", theme.id);
  localStorage.setItem("tesela:mode", theme.id);
}
