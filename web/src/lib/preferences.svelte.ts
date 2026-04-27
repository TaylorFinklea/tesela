/**
 * Reactive global user preferences, persisted to localStorage.
 *
 * The shape is intentionally tiny — only knobs that affect cross-component
 * UI live here. Page-scoped state (drill, fold, expanded props) stays local
 * to its component.
 */

function load<T>(key: string, fallback: T): T {
  if (typeof localStorage === "undefined") return fallback;
  const raw = localStorage.getItem(`tesela:${key}`);
  if (raw === null) return fallback;
  try {
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function save<T>(key: string, value: T): void {
  if (typeof localStorage === "undefined") return;
  localStorage.setItem(`tesela:${key}`, JSON.stringify(value));
}

export type BulletStyle = "dot" | "arrow";

class Preferences {
  bulletStyle = $state<BulletStyle>(load<BulletStyle>("bulletStyle", "dot"));

  setBulletStyle(v: BulletStyle): void {
    this.bulletStyle = v;
    save("bulletStyle", v);
  }
}

export const prefs = new Preferences();
