/**
 * Phase 10.6 — chip icon registry.
 *
 * Property pages can declare `chip_icon` in frontmatter to set the icon
 * shown on display chips. The string is resolved in two modes:
 *
 *   1. Tabler name match (e.g. `"calendar"` → `IconCalendar`). Curated
 *      list — adding an icon means importing it here. Keeps bundle
 *      explicit instead of pulling Tabler's full pack.
 *
 *   2. Anything else is treated as raw text (emoji, single character,
 *      short letter prefix). Useful for zero-config use of emoji like
 *      `"📅"` or `"⚑"` without touching code.
 *
 * Either way the chip stays a single inline element; only the rendering
 * branch differs (Svelte component vs text span).
 */
import {
  IconCalendar,
  IconClock,
  IconFlag,
  IconTag,
  IconHourglass,
  IconBookmark,
  IconHash,
  IconLink,
  IconMail,
  IconPhone,
  IconUser,
  IconStar,
} from "@tabler/icons-svelte";

// Tabler's Svelte 5 typings are still in flux (the `IconCalendar` etc.
// values are typed as legacy SvelteComponent classes, not the new
// `Component` shape). The runtime is fine; we just relax the map type.
export const TABLER_ICONS: Record<string, unknown> = {
  calendar: IconCalendar,
  clock: IconClock,
  flag: IconFlag,
  tag: IconTag,
  hourglass: IconHourglass,
  bookmark: IconBookmark,
  hash: IconHash,
  link: IconLink,
  mail: IconMail,
  phone: IconPhone,
  user: IconUser,
  star: IconStar,
};

/**
 * Resolve a chip-icon string into either a Tabler component (for known
 * names) or a raw emoji/text fallback (for anything else). The caller
 * picks one branch — exactly one of `component` / `emoji` is non-null
 * when the input is non-null.
 */
export function resolveChipIcon(name: string | null): {
  component: unknown | null;
  emoji: string | null;
} {
  if (!name) return { component: null, emoji: null };
  const c = TABLER_ICONS[name.toLowerCase()];
  if (c) return { component: c, emoji: null };
  return { component: null, emoji: name };
}
