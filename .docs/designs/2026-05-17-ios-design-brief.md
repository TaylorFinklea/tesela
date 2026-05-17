# Tesela iPhone — design brief

Self-contained brief for an external designer (Claude Design) to propose a
fresh iOS app design for Tesela. The current iOS scaffold under
[`app/Tesela-iOS/`](../../app/Tesela-iOS/) is minimal (links Rust core,
prints a version string). This brief covers product context, hard
constraints, the design system the iOS app must align with, and the open
design decisions the designer should make.

---

## What Tesela is

Tesela is a local-first, file-backed knowledge tool. The on-disk format is
plain markdown — every page is a `.md` file with YAML frontmatter; the
filesystem is the source of truth. A Rust core (`tesela-core` +
`tesela-sync`) handles parsing, indexing, and peer-to-peer sync.

Vocabulary used throughout the product:
- **Mosaic** — a workspace / knowledge base (the folder containing
  `.tesela/`). Equivalent to a Logseq graph or Obsidian vault.
- **Page** — a single markdown file. User-facing term.
- **Tile / Daily** — a daily-note page (`type: daily`). One per calendar
  day, the journal-style front door.
- **Tag page** — a page with `type: tag` frontmatter. Tags are
  first-class entities, not labels (see
  [`2026-05-17-tag-system.md`](./2026-05-17-tag-system.md)).
- **Block** — a single `- ` bullet inside a page. Blocks have an id, can
  be tagged, can carry `key:: value` properties.

The web client (SvelteKit) ships the "Prism v5" chrome and is the design
benchmark. The iOS app must **share the visual language and conceptual
model** with Prism v5, but **must not** copy its interaction model
(keyboard-first, vim, multi-pane). Mobile is touch-first.

---

## Locked product decisions

These are settled. The designer should NOT re-litigate them.

### Platform decisions

- **iPhone is a separate native SwiftUI codebase.** It is NOT a webview, NOT
  a PWA, NOT a port of the web client. It is also NOT inheriting the
  frozen macOS SwiftUI app under [`app/Tesela/`](../../app/Tesela/) — that
  codebase is paused. The iOS app builds against the Rust core via UniFFI
  Swift bindings.
- **iPad** is deferred. Design for iPhone first. iPad layout will be
  decided later (probably the same app with `horizontalSizeClass == .regular`
  adaptations, but not in this round).
- **Foreground-only sync.** Apple's background-execution constraints mean
  the iPhone syncs while the app is in the foreground only. The "we are
  out of date" affordance is a real surface to design.

### Interaction decisions

- **No vim mode anywhere on iOS.** No leader keys, no chord menus, no
  modal editing. The web client's whole keyboard-first ergonomics is
  wrong for a one-thumb mobile device.
- **Long-press is the power menu.** When the user wants to do something
  beyond the primary tap target, long-press surfaces contextual actions.
  Direction: a SwiftUI `Menu` or contextual sheet, NOT a custom popover.
- **Voice as a future power affordance** (Apple Speech framework,
  on-device only). Out of scope for this design round but the data model
  should not prevent it (verbs from the web client's `:command` palette
  are the natural voice command vocabulary).
- **Daily note + quick capture is the front door.** Opening the app
  should land the user on today's daily note, ready to type. "Quick
  capture" is a separate one-tap path that prepends a block to today's
  daily.

### Format & sync decisions

- Markdown on disk is canonical. The iOS app reads/writes the same
  format as the web client and the macOS app (also paused).
- Sync is peer-to-peer over Automerge CRDTs at the block level. iPhone
  is just another peer.
- File storage is the app's sandboxed Documents directory. iCloud
  Documents is backup only — sync proper is `tesela-sync`.

### First-class iOS surfaces

- **Share sheet** — receive text/links from other apps into a daily-note
  block (or a chosen target page).
- **Shortcuts** — Apple Shortcuts integration for quick-capture and
  page-open.
- **Widgets** — at least a "today's daily note preview" widget. Possibly
  a "pinned page" widget.
- **EventKit / Apple Reminders** bridge — re-uses the macOS sync code.
  Not a first-screen concern but the data model should not block it.

---

## The Prism v5 design system (must align)

The web client's design tokens are the source of truth for color,
typography, and visual feel. The iOS app should adapt them faithfully —
not invent a new palette.

### Files to read for the visual language

- [`web/src/app.css`](../../web/src/app.css) — the **canonical token
  contract**. Defines the role names: `--bg`, `--bg-2`, `--bg-3`,
  `--line`, `--line-soft`, `--fg-default`, `--fg-muted`, `--fg-subtle`,
  `--fg-faint`, `--accent-primary` (#7b8cff indigo by default),
  `--accent-secondary` (#f0a45c amber). Plus type-semantic colors
  (`--type-task`, `--type-event`, `--type-note`, etc.).
- [`web/src/themes.css`](../../web/src/themes.css) — palette overrides
  per theme (Tokyo Night, Catppuccin, Gruvbox, Rosé Pine, …). Each
  theme remaps the role tokens via `[data-theme="id"]` selectors.
- [`web/src/lib/icon-registry.ts`](../../web/src/lib/icon-registry.ts)
  — Tabler icon set used in the web. Use the same iconography on iOS
  (Tabler ships SwiftUI bindings via `tabler-icons-swift`).

### Typography

- Display / body: **Inter Tight** (sans). Fallback: SF Pro Text.
- Monospace: **JetBrains Mono** (for code, block IDs, schema versions,
  tag chips on the web). On iOS, the chip aesthetic may not need a
  literal mono font — use the designer's judgment, but keep visual
  parity for "monospaced-looking" affordances.
- Optional serif display: **Newsreader** (used in v5 reference design).
  Probably not needed on iOS — the designer can decide.

### Visual feel

The default theme is Tokyo Night (dark). Other themes shipped:
Catppuccin, Gruvbox, Rosé Pine, plus a light "workbench" palette
(`.docs/designs/v9-columns/v5/styles-v5.css`). The iOS app should pick
**one default theme** to ship with (probably system-appearance-aware:
light + dark) and the rest can come later. The designer chooses which
single palette to anchor first.

The web's chrome is **calm and informational**: thin hairlines, lots of
breathing room around blocks, monospaced chips for metadata, no shadows
except subtle ones in overlays. Carry this restraint to iOS — avoid
heavy iOS-default backgrounds and over-styled cards.

---

## Conceptual model — translating Prism v5 to iOS

The web client has a complex shape that does NOT map directly to iOS.
The designer's job is to find iOS-native idioms that preserve the
**concepts** without copying the **layout**.

### Web Prism v5 concepts (read [`2026-05-15-prism-v5-chrome.md`](./2026-05-15-prism-v5-chrome.md) for full spec)

The web client has three "buffer kinds" that mount in a binary pane
tree:

- **Page buffer** — renders one page (note, daily, query, scratch,
  tag, …). The page-type renderer dispatches by frontmatter `type:`.
- **Derived buffer** — pure function of a Reference (a page, a tag,
  or a query DSL). Examples: `backlinks-of-page`, `outline-of-page`,
  `instances-of-tag`. Read-only, can follow the most-recently-focused
  page or be pinned to a fixed reference.
- **Ambient buffer** — workspace-level singletons (calendar,
  today-in-progress, workspace dashboard, AI workspace).

Plus a **left sidebar** with five switchable surfaces (notes tree,
search, recent, pinned, tags) and overlays (⌘K palette, ⌘I Peek,
⌘G fullscreen graph, ⌘, settings).

### Translation hints (designer makes the calls)

The iOS app probably collapses the binary pane tree into **one focused
page at a time, full-screen**. The questions the designer must answer:

- **Sidebar surfaces** on iOS → are these a **bottom tab bar**? A
  **left sheet that swipes in from the edge**? Both?
- **Command palette (⌘K equivalent)** → search field on the home
  screen? Pull-to-search? A persistent button? Voice button? Whatever
  this is, it must be **one tap away from anywhere**.
- **Peek (⌘I equivalent — surfaces backlinks / outline / properties
  of the focused page)** → a swipe-up sheet from the bottom of a page?
  A long-press menu? Page metadata is critical context; this needs a
  clear, fast affordance.
- **Buffer kinds on a single screen** → does the user navigate
  between page / derived / ambient via tab bar? Stack push? Page tabs
  at the top of the screen? The web tab-strip will NOT translate
  directly.
- **Daily note as front door** → does the app launch into today's
  daily directly (no list-of-pages intermediate)? How does the user
  scroll back to yesterday's daily? Vertical infinite scroll like the
  web's `JournalView`? A horizontal date picker?
- **Tag pages on iOS** → the web's tag page is a composite
  (description on top, instances table on bottom). On iOS this might
  be a vertical stack with a tab-segmented control to switch between
  description and instances. Or a sticky description header above
  an infinite instances list. Designer call.

### iOS-native idioms to lean on

- **Pull-to-refresh** (sync now, refresh recent changes)
- **Swipe gestures** on blocks (swipe right = indent? swipe left =
  delete? designer call)
- **Long-press = power menu** (block actions, page actions)
- **Floating action button** for quick capture (or pull-down from
  daily-note top — designer call)
- **Drag handles** on blocks for reordering (the web outliner doesn't
  have these; the iPhone might need them)
- **Sheet presentations** for context-rich actions (page settings,
  tag picker, sync status)
- **Tab bar** for the few persistent surfaces (probably 3–4 tabs max:
  Daily, Pages, Tags, More — designer call)

---

## Open design decisions (please ask product questions)

The designer should propose visual designs and **ask Taylor product
questions** on these. Do not assume answers. The phrasing of the
question matters — frame each as a single-select or multi-select with
2–4 options so Taylor can answer quickly.

These are the questions worth asking, ordered roughly by how much they
shape the rest of the design:

1. **Bottom tab structure** — what are the 3–5 persistent tabs at the
   bottom (or top) of the app? Examples: Daily / Pages / Tags / Search
   / Settings. Or fewer.
2. **Quick capture mechanism** — floating "+" button bottom-right?
   Pull-down from top of daily? Volume-button shortcut? Long-press on
   the app icon (Apple's quick action)?
3. **How to surface derived context (backlinks / outline) on iOS** —
   swipe-up sheet? Tap a chip on the page header? Separate "info" tab?
4. **How to navigate between dailies** — vertical infinite scroll
   through past days (like the web)? Date picker at top? Horizontal
   swipe day-by-day?
5. **How does the user open the command palette / search** — magnifying
   glass icon on every screen? Pull-down on tab content? Persistent
   bottom-bar search affordance?
6. **Theme picking** — ship Tokyo Night dark + a light theme by
   default, with system-appearance-aware switching? Or one theme to
   start?
7. **Vim-power-user concession (long-press menu vocabulary)** — should
   the long-press contextual menu surface the same verb names as the
   web client (`:scratch`, `:promote`, `:rename-slug`, `:convert-to-
   tag`) for vocabulary continuity, or should iOS use friendly labels
   ("New scratch", "Save as page", "Rename tag", "Convert to tag")?
8. **Tag chip rendering on a block** — same trailing-cluster chip
   style as the web (clickable pill at the end of the block), or
   right-aligned (Logseq DB style) regardless of position in source?
9. **EventKit bridge surface** — visible as a dedicated tab or buried
   in settings until used?
10. **Sync status indicator** — top of every screen (subtle bar)?
    Pull-down reveal? Settings-only?

---

## Hard constraints (must hold)

- One source of truth: markdown on disk via the Rust core.
- No HTTP server inside the iPhone app. The app calls Rust directly
  through UniFFI Swift bindings.
- No vim. No leader chords. No multi-pane.
- Daily note as launch destination. Quick capture is one tap.
- All chrome stays within the Tokyo-Night-derived palette of
  [`web/src/app.css`](../../web/src/app.css). No iOS-default blues.
- Tag system semantics from
  [`2026-05-17-tag-system.md`](./2026-05-17-tag-system.md):
  `#tag` is a classification/reference; trailing-cluster tokens
  render as chips on the web (decide what they look like on iOS).
- Iconography from Tabler (`tabler-icons-swift` package) for
  cross-platform glyph parity. No SF Symbols-only design.

---

## Out of scope for this design round

- iPad layout
- macOS / desktop SwiftUI app
- Watch app
- Android
- Sync UI beyond a basic status indicator and a "sync now" affordance
- Settings beyond what the MVP needs (theme picker, account, sync
  toggle)

---

## What the deliverable looks like

The designer should produce:
1. **A small set of key screens** (probably 6–10): launch / today's
   daily, page view (note, tag, query), pages list, tag list, search,
   command-palette equivalent, long-press menu, settings, sync state.
2. **A short style memo** mapping the web's role tokens to SwiftUI
   `Color` values and choosing typography sizes / weights for each
   text style.
3. **Answers to (or follow-up questions on) the 10 product questions
   above.**

Output format: editable artboards / Figma-style frames are fine. The
final pass will need a SwiftUI implementation plan written by a separate
session.
