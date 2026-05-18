# Tesela iPhone — design follow-up (v0.5)

Critical-review follow-up to the v0.4 "Tile" canvas. The original brief
([`2026-05-17-ios-design-brief.md`](./2026-05-17-ios-design-brief.md))
stands; this file only documents the 15 product decisions Taylor locked
in after reviewing v0.4, plus the visual/IA changes those decisions
demand. Read both files together.

The v0.4 canvas got the bones right: 3-tab IA, persistent capture bar,
Peek-as-segmented, position-aware tag chips, properties sheet,
sync/conflict/pair flows, 16-theme picker. The issues this follow-up
addresses are **brand drift from the web v5 client** and **a handful of
invariant blurs** that crept into the IA.

---

## Locked decisions (supersede v0.4 where they conflict)

### 1. Default theme — Prism indigo, not Tokyo Night

The web v5 client's default `--accent-primary` is `#7b8cff` (Prism
indigo). The v0.4 canvas defaulted to Tokyo Night warm orange `#ff9e64`.
**First-launch iPhone must match first-launch web.** Tokyo Night becomes
one of 16 picker options, not the default.

Action: regenerate every canvas screen with `--accent-primary: #7b8cff`
and the v5 indigo palette. The 16-theme picker still ships; Tokyo Night
sits in the list, not at the top.

### 2. Tags IA — flat list with a type-filter strip

The v0.4 Library tab subordinated Tags as a sibling segment under Pages.
This breaks the "tags are pages" invariant — tag pages are `type: tag`
markdown files, peers to all other typed pages.

Replace Library's `Pages · Tags · Recent` segmented control with:
- One flat list of all pages
- A horizontal type-filter strip across the top:
  `All · Pages · Tags · Daily · Projects · People · Queries · Workspace · Scratch`
- Recent/Pinned live as a sticky eyebrow at the top of the "All" filter
  (or a chip on the strip — designer's call)
- The strip scrolls horizontally on small screens

### 3. Onboarding — pair-flow is the primary CTA

Taylor's actual onboarding is "I already have a mosaic on my desktop,
sync it to this iPhone." Lead with that:
- Primary CTA: **"Join existing mosaic"** → opens pair-device flow
  (T-S6 lifted out of Settings to be the post-onboarding screen)
- Secondary CTA: "Create a new mosaic" (small text button)
- T-S6 stays accessible from Settings → Sync for later device pairs

### 4. Pair flow — symmetric P2P language only

The locked sync architecture
([`project_sync_architecture.md`](../../crates/tesela-sync/)) is
symmetric Automerge CRDT + Lamport stamps. No source-of-truth device,
no hosted relay.

Remove from all sync surfaces (T-S3, T-S6):
- "Use this iPhone as the source of truth" → "Pair this iPhone with another device"
- Peer roles `host` / `peer` / `relay` → drop the role labels entirely
- The "Tailscale funnel" peer labeled `relay` → relabel as
  "remote peer (Tailscale)" or drop the badge

### 5. Ambient buffers — surfaced via a "Workspace" filter chip

The web v5 client has four ambient buffers (workspace singletons not
tied to a page): `calendar`, `today-in-progress`, `workspace-dashboard`,
`ai-workspace` (placeholder). v0.4 omitted all four.

Mobile gets them behind a single "Workspace" chip in the Library
type-filter strip. Tapping the chip opens a small card grid:
- **Calendar** — month view; tapping a day opens that daily page
- **In Progress** — list of in-progress tasks across the workspace
- **Dashboard** — pinned widgets recycled from web's dashboard
- **AI** — placeholder card with "coming in a later phase" + one
  disabled teaser action

### 6. Capture bar — fuses with the verb palette

Web has a single `⌘K` entry point: type to capture-or-search, prefix
with `:` to run verbs. Mobile should mirror that mental model in the
persistent capture bar:
- Default: typing prepends a block to today's daily (current behavior)
- `:` prefix: switches to palette mode — verb chip strip appears
  above the bar (`:scratch`, `:promote`, `:daily`, `:rename-slug`, …),
  the Send button becomes "Run"
- Escape (or deleting the `:`) returns to capture mode

Search tab consequently drops its trailing "Verbs" section.

### 7. Peek shape — page body always on top

v0.4 made `page` the first segment of the segmented control. That puts
the page-buffer and derived-buffers on the same plane, which blurs the
v5 invariant (page = one filesystem file; derived = pure function of a
reference).

Cleaner shape:
- Page title + frontmatter chrome at top
- Page body always renders below the title (no segment switch needed)
- A collapsible segmented control sits below the body with
  **derived-only** segments: `backlinks · outline · props · tasks · graph`
- Tap a segment → reveals that derived view; tap again → collapse
- Same shape works on the tag page composite (where `description`
  is the always-on body and segments add derived lenses)

### 8. PageTagsChips strip — mirror web parity

The web v5 client renders frontmatter `tags: [...]` as a removable chip
strip with `+` picker at the top of every body-text page (Phase 12).
v0.4 hid this behind the properties sheet.

Surface it on iOS directly under the page title:
- Each tag = chip with `×` button to remove
- Trailing `+` button opens a filter+create picker
- Mirrors `web/src/lib/components/v4/PageTagsChips.svelte` exactly

### 9. Scratch — verb to create, filter chip to browse

Mobile mirrors desktop's verb + tree-toggle pattern:
- `:scratch` in the capture bar → creates a scratch page, opens it
- "Scratch" filter chip in Library's type-filter strip → browses
  existing scratches
- Filter chip's empty state offers a "+ start a scratch" button

### 10. Always-dark for v0.4

Light themes (Tokyo Night Day, Catppuccin Latte, Rosé Pine Dawn, etc.)
defer to a later release. v0.4 ships all 16 dark themes; iOS ignores
the system Light/Dark setting for now.

### 11. Voice — one block default, opt-in split

- Default: each recording session lands as a single block prepended
  to today
- Settings → Voice → "Split on long pauses" toggle: opt-in to splitting
  on pauses > 1.5s
- No auto-detection; user-explicit

### 12. Voice configuration — top-level Settings section

Move voice configuration **out of** Settings → Bridges into its own
top-level Settings → Voice section:
- Parakeet v3 model status, language, auto-punctuation, split toggle
- Bridges retains only cross-app integrations (Apple Calendar,
  Reminders, Shortcuts, Share extension, Files, API/x-callback)

### 13. Modified marker — sync-state, not file-write

Continuous-save is assumed invisible. The `●` indicator only appears
when **both** of these are true:
- Sync is offline (no peers reachable)
- Local edits haven't been seen by any peer yet

Place the dot in the page-title chrome (a small `●` to the left of
the title). Disappears the moment any peer becomes reachable and the
change log replays.

### 14. Multi-page nav — Safari-style page-swipe stack

Replace "back-button only" with a card-stack model:
- Tapping a wiki-link or tag pushes a new page card onto the stack
- The `< Library` chevron pops one card
- **Swipe up from the bottom edge** (above the tab bar, or with the
  tab bar dimming) reveals a horizontal carousel of all open page cards
  (Safari-tabs / iOS-app-switcher shape)
- Tap a card to jump; swipe a card up to close it
- Stack persists across app launches

Gesture-conflict resolution with the tab bar is yours to figure out —
the simplest answer is "the swipe-up starts on the chrome above the
tab bar," but a `⌄` chip at the very top of every page card that
expands into the carousel is also fine.

### 15. Density — Settings toggle

Add Settings → Appearance → Density with three options:
- **Comfortable** (default): 15pt body / 1.5 line-height
- **Compact**: 13pt / 1.45
- **Compact+**: 12pt / 1.4

All other type-scale roles step in proportion. Default Comfortable on
first run.

---

## Visual checklist for the regenerated canvas

When you regenerate the canvas, please verify:

- [ ] `--accent-primary` is `#7b8cff` everywhere; warm orange shows up
  only inside Tokyo Night themed previews
- [ ] Library has the horizontal type-filter strip; no Pages/Tags/Recent
  segmented control
- [ ] Onboarding screen leads with "Join existing mosaic"
- [ ] T-S3 peer rows have no `host` / `peer` / `relay` badges
- [ ] T-S6 pair flow uses symmetric language; no "source of truth"
- [ ] "Workspace" filter chip exists in Library with the 4-card grid
- [ ] Capture bar shows palette mode (verb chip strip above bar)
  in at least one screen
- [ ] Peek segmented control is derived-only; page body always above it
- [ ] PageTagsChips strip is visible on the page-view screen
- [ ] `:scratch` verb chip + "Scratch" filter chip both present
- [ ] No light-mode swatches; all themes dark
- [ ] Voice has its own Settings section
- [ ] At least one page-view screen shows the `●` modified marker in
  the offline-with-pending-edits state
- [ ] Page-swipe stack carousel is mocked up (overlay state)
- [ ] Density picker exists in Settings → Appearance with three options

---

## Still open (your call)

These ride on craft, not architecture. Make a decision; flag it in your
next memo card:

1. The capture-bar palette-mode visual treatment — chip strip vs.
   inline-completion dropdown vs. autocomplete-style table
2. Where Recent/Pinned actually surface in the new flat Library — eyebrow
   at top of "All", or chips on the type-filter strip
3. Type-filter strip on small screens — horizontal scroll vs. two-row
   wrap vs. "more" overflow menu
4. Swipe-up page stack: does the tab bar disappear during the carousel,
   or dim? Or does a small `⌄` affordance live at the very top of every
   page card?
5. Modified marker `●` position — left of title, in the title chrome
   eyebrow, or in the top-right sync-indicator slot
6. Settings → Voice page layout (model status card + toggles vs. nested
   table)
