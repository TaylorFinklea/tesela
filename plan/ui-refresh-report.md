# UI refresh report

Worktree: `.claude/worktrees/ui-refresh` on branch `worktree-ui-refresh`.

## Scope shift

Started as a targeted UI refresh (font, accent role narrowing, bullet quieting).
During clarification, scope expanded to a full theme system: 24 dark themes + 3
light themes, switchable in Settings and the command palette, persisted in
localStorage, with the role token contract now driving everything.

The targeted design fixes were applied on top of the new contract so they
travel with every theme automatically.

## Locked decisions

1. Content font: Geist Sans, loaded from Google Fonts. JetBrains Mono stays as
   the mono. Themes can override both via `--theme-font-sans` /
   `--theme-font-mono`.
2. Theme set: 27 total. Default is Tokyo Night.
3. Switcher: settings page swatch grid plus one searchable command palette
   entry per theme. Persisted in `localStorage` under `tesela:theme`.
4. Date headings move off `--primary` to `--fg-default`.
5. The "Today" pill is now a soft tinted chip (peach background at 14 percent
   alpha plus a 28 percent border) instead of a saturated peach pill.
6. Drawer tabs (Backlinks, Properties, Outline, History, Linked tasks) lose
   the amber underline. Active state is a subtle background tint plus a thin
   line border.
7. Empty page placeholder reads "i to insert" instead of
   "Click to start writing...".
8. Type chips and bullet colors now read from `--type-task`, `--type-event`,
   `--type-note`, etc., which are tuned per theme to be a notch quieter than
   each theme's max chroma value of the same hue.

## Files changed

### Token contract and themes

- `web/src/app.css` — replaced the v9 token block with a role token
  contract. Legacy `--v9-*` and shadcn-style aliases (`--background`,
  `--primary`, etc.) now reference role tokens, so the rest of the codebase
  inherits the swap with no per-component changes. Drawer tab active state
  was quieted here. Three hardcoded literals tied to the old peach were
  replaced with `color-mix(in srgb, var(--accent-primary) ...)` so theme
  switches paint correctly.
- `web/src/themes.css` (new) — 27 theme blocks, each declaring the same role
  tokens. Selectors are `:root[data-theme="<id>"]` so theme cascade beats the
  `:root` defaults declared in app.css. CSS `@import` rules must come before
  other rules in the file, which is why specificity, not source order, is
  what makes the cascade work.
- `web/src/lib/themes/index.ts` (new) — registry with id, name, mode, and
  swatch colors for the picker.
- `web/src/lib/theme.svelte.ts` (new) — `$state` store, persists to
  localStorage and updates `data-theme` plus `dark` / `light` classes on
  `<html>`.

### FOUC prevention

- `web/src/app.html` — Google Fonts link extended to load Geist alongside
  Inter Tight and JetBrains Mono. The inline init script now reads the
  persisted theme id from localStorage and sets `data-theme` plus
  `dark` / `light` classes before any module loads, so the first paint
  matches the persisted theme. The script ships allowlists of theme ids so
  it can fall back safely if storage is corrupted.

### Editor font and placeholder

- `web/src/lib/components/BlockEditor.svelte` — CodeMirror theme now reads
  `var(--theme-font-sans)` instead of the broken `'Source Sans 3'` reference.
- `web/src/lib/components/DocumentEditor.svelte` — same.
- `web/src/lib/components/BlockOutliner.svelte` — empty-page placeholder
  replaced with a quiet "i to insert" hint.

### Date headings and Today pill

- `web/src/lib/components/JournalView.svelte` — `.day-title` color moves
  from `var(--primary)` to `var(--fg-default)`. `.day.is-today .day-title`
  no longer brightens to primary; the pill is the marker. Pill uses tinted
  primary instead of solid primary.

### Settings UI

- `web/src/routes/settings/+page.svelte` — added a Theme section above the
  Vim toggle. Two-column grid of swatch tiles. Each tile shows a 2x2
  preview (bg, fg, primary, secondary), the theme name, and the mode badge
  ("dark" or "light"). Selected tile gets a primary ring.

### Command palette

- `web/src/lib/commands.ts` — added `theme` category. One command per
  theme (`Theme: <name>`). Keywords include the mode and id so typing
  "dark", "light", or part of a theme name surfaces matches. Theme entries
  do not show in the default palette view; they appear once the user starts
  searching, which keeps the default surface uncluttered.

## What was deliberately not changed

- Block bullet implementation. The CodeMirror decorator and the `.v9-bl .blk
  .bull` class already keyed off `--v9-rose` etc. Those are now aliases of
  `--type-*` role tokens, so bullets pick up the new lower-saturation
  values per theme without touching the bullet code.
- Vim mode chip, hint chips at the top right, breadcrumb, status bar layout,
  rail, mini-calendar layout. None of those needed structural change. They
  re-skin via the role tokens.
- Spacing and grid. No changes.
- Mockup files under `web/static/mockups/`. Those are static snapshots, not
  live-app surface.
- Right panel tab text content and per-tab body styling. Only the active-tab
  indicator was quieted.

## How the cascade works

```
:root                                    -> Tokyo Night defaults (app.css)
:root[data-theme="<id>"]                 -> theme overrides (themes.css)
.dark, .light                            -> mode-conditional rules
```

`:root` and `:root[data-theme="..."]` differ in specificity, so theme blocks
always win. The `@import "./themes.css"` in app.css must stay above the
`:root` block; CSS `@import` is required to come before other rules.

A reasonable next layer, if needed: `[data-theme="..."]` selectors that
target component classes directly for theme-specific overrides that go
beyond role tokens.

## Verification

Done in Chrome via the dev server on port 5174:

- Tokyo Night renders unchanged at first paint with no console errors.
- Switching to Catppuccin Mocha via the settings tile updates the persisted
  theme id and the live computed CSS variables (`--bg`, `--accent-primary`,
  `--fg-default` all match the Mocha palette).
- Switching to Gruvbox Dark via localStorage and reloading produces the
  expected brown-black canvas, parchment fg, gruvbox-orange accent, and
  type colors visible in the legend.
- Cmd+K opens the command palette; typing "theme" surfaces all theme
  commands, ranked.
- Mode chip (NORMAL or INSERT) still uses the active theme's primary,
  hint chips at top right and the breadcrumb still render correctly.
- svelte-check reports 0 errors. Pre-existing a11y warnings are unchanged
  except one that I fixed on the new theme tile (added `aria-label`).

## Open questions

1. Bullet brightening on focus. The brief asked for bullets to brighten on
   hover or when the containing block is focused. The current rules rely on
   `.cm-line` getting an active-line state, which in CodeMirror 6 is on by
   default but currently styled flatly. A small follow-up could route the
   bullet color through a `var(--bullet-color, var(--fg-subtle))` indirection
   and have the focused/hovered block override the variable. Not in this
   pass to keep diff focused.
2. Light-theme contrast pass. Three lights ship with sensible canonical
   values, but the warm-on-warm chrome (status bar, breadcrumb) deserves
   a second look on Latte and Dawn under sustained use. The
   `.light .v9-status` and `.light .v9-crumb` rules in themes.css are a
   first cut.
3. Theme thumbnails in settings could include a tiny preview of a block
   line plus a chip, not just a 2x2 swatch. Optional polish.
4. Per-theme font overrides are wired but unused. If you want, say, Carbonfox
   or Monokai Pro to ship with a different mono, set `--theme-font-mono`
   inside that theme's block.

## Suggested next steps

1. Spend ten minutes flipping through every theme. Confirm the type chips
   read distinct from `--accent-primary` in each one. The colors I picked
   are reasoned but not user-tested.
2. Decide whether to surface a "Themes" group in the default ⌘K view or
   keep it search-only. Currently search-only.
3. Decide whether to expose the bullet brightening behavior in this PR or
   leave for a follow-up.
