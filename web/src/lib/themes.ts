/**
 * Tesela theme system.
 * Day + Evening as the core pair, plus your favorite dark variants.
 */
export type ThemeId = "day" | "evening" | "woven" | "tile-grid" | "depth-layers" | "neon-glow";

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
    description: "Warm cream, golden light",
    isDark: false,
    vars: {
      "--background": "#faf7f2", "--foreground": "#2c2824",
      "--surface": "#f3ede4", "--surface-2": "#ebe4d8",
      "--muted": "#e8e0d4", "--muted-foreground": "#8a8078",
      "--accent": "#f0e8dc", "--accent-foreground": "#2c2824",
      "--primary": "#c4852c", "--primary-foreground": "#faf7f2",
      "--destructive": "#c45a4a", "--border": "#e8e0d4",
      "--ring": "#c4852c", "--popover": "#faf7f2", "--popover-foreground": "#2c2824",
      "--block-bg": "#ffffff", "--block-border": "#e8e0d4", "--block-radius": "8px",
      "--block-shadow": "0 1px 3px rgba(120,90,50,0.06)",
      "--focus-glow": "0 0 0 3px rgba(196,133,44,0.12)", "--thread-border": "#e8e0d4",
    },
  },
  {
    id: "evening",
    name: "Evening",
    description: "Warm charcoal, amber lamplight",
    isDark: true,
    vars: {
      "--background": "#1e1c24", "--foreground": "#e8e0d4",
      "--surface": "#17151c", "--surface-2": "#24222c",
      "--muted": "#2a2830", "--muted-foreground": "#9a918a",
      "--accent": "#2a2830", "--accent-foreground": "#e8e0d4",
      "--primary": "#d4a04a", "--primary-foreground": "#1e1c24",
      "--destructive": "#d46a5a", "--border": "rgba(255,240,210,0.08)",
      "--ring": "#d4a04a", "--popover": "#24222c", "--popover-foreground": "#e8e0d4",
      "--block-bg": "#24222c", "--block-border": "rgba(255,240,210,0.06)", "--block-radius": "8px",
      "--block-shadow": "0 1px 3px rgba(0,0,0,0.2)",
      "--focus-glow": "0 0 0 3px rgba(212,160,74,0.15)", "--thread-border": "rgba(255,240,210,0.06)",
    },
  },
  {
    id: "woven",
    name: "Woven",
    description: "Purple-tinted, textile thread borders",
    isDark: true,
    vars: {
      "--background": "#1a1822", "--foreground": "#d8cce8",
      "--surface": "#14121a", "--surface-2": "#201e28",
      "--muted": "#282630", "--muted-foreground": "#8a80a0",
      "--accent": "#282630", "--accent-foreground": "#d8cce8",
      "--primary": "#d4a04a", "--primary-foreground": "#1a1822",
      "--destructive": "#d46a5a", "--border": "rgba(200,180,240,0.06)",
      "--ring": "#d4a04a", "--popover": "#201e28", "--popover-foreground": "#d8cce8",
      "--block-bg": "transparent", "--block-border": "transparent", "--block-radius": "0px",
      "--block-shadow": "none",
      "--focus-glow": "0 0 0 3px rgba(212,160,74,0.12)", "--thread-border": "rgba(200,180,240,0.10)",
    },
  },
  {
    id: "tile-grid",
    name: "Tile Grid",
    description: "Blocks as distinct tile cards",
    isDark: true,
    vars: {
      "--background": "#141218", "--foreground": "#e4e0ea",
      "--surface": "#100e14", "--surface-2": "#1c1a22",
      "--muted": "#24222a", "--muted-foreground": "#8a86a0",
      "--accent": "#1c1a22", "--accent-foreground": "#e4e0ea",
      "--primary": "#d4a04a", "--primary-foreground": "#141218",
      "--destructive": "#d46a5a", "--border": "rgba(255,240,210,0.05)",
      "--ring": "#d4a04a", "--popover": "#1c1a22", "--popover-foreground": "#e4e0ea",
      "--block-bg": "#1c1a22", "--block-border": "rgba(255,240,210,0.06)", "--block-radius": "10px",
      "--block-shadow": "0 2px 6px rgba(0,0,0,0.25)",
      "--focus-glow": "0 0 0 3px rgba(212,160,74,0.15)", "--thread-border": "rgba(255,240,210,0.05)",
    },
  },
  {
    id: "depth-layers",
    name: "Depth Layers",
    description: "Elevated panels, architectural shadows",
    isDark: true,
    vars: {
      "--background": "#141218", "--foreground": "#e4e0ea",
      "--surface": "#18161e", "--surface-2": "#1e1c26",
      "--muted": "#26242e", "--muted-foreground": "#8a86a0",
      "--accent": "#1e1c26", "--accent-foreground": "#e4e0ea",
      "--primary": "#d4a04a", "--primary-foreground": "#141218",
      "--destructive": "#d46a5a", "--border": "rgba(255,240,210,0.05)",
      "--ring": "#d4a04a", "--popover": "#1e1c26", "--popover-foreground": "#e4e0ea",
      "--block-bg": "#1a1820", "--block-border": "rgba(255,240,210,0.04)", "--block-radius": "10px",
      "--block-shadow": "0 4px 16px rgba(0,0,0,0.3)",
      "--focus-glow": "none", "--thread-border": "rgba(255,240,210,0.05)",
    },
  },
  {
    id: "neon-glow",
    name: "Neon Glow",
    description: "Glowing amber borders, deep black",
    isDark: true,
    vars: {
      "--background": "#0e0c12", "--foreground": "#eae6e0",
      "--surface": "#0a080e", "--surface-2": "#16141c",
      "--muted": "#1e1c24", "--muted-foreground": "#8a8690",
      "--accent": "#16141c", "--accent-foreground": "#eae6e0",
      "--primary": "#e0a840", "--primary-foreground": "#0e0c12",
      "--destructive": "#e06050", "--border": "rgba(255,240,210,0.04)",
      "--ring": "#e0a840", "--popover": "#16141c", "--popover-foreground": "#eae6e0",
      "--block-bg": "transparent", "--block-border": "transparent", "--block-radius": "0px",
      "--block-shadow": "none",
      "--focus-glow": "0 0 20px rgba(224,168,64,0.20)", "--thread-border": "rgba(255,240,210,0.08)",
    },
  },
];

export function getTheme(id: string): ThemeDefinition {
  return themes.find((t) => t.id === id) ?? themes[0];
}

export function applyTheme(id: string) {
  const theme = getTheme(id);
  const root = document.documentElement;
  for (const [key, value] of Object.entries(theme.vars)) root.style.setProperty(key, value);
  if (theme.isDark) root.classList.add("dark");
  else root.classList.remove("dark");
  root.setAttribute("data-theme", theme.id);
  localStorage.setItem("tesela:mode", theme.id);
}
