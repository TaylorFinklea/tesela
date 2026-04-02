# Release History

This file tracks all releases of Tesela.


## v0.20250825.0 - 2025-08-25 12:03:02 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---


## v0.20260319.0 - 2026-03-19 11:14:57 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---

## What's Changed

- update (d875c49)
- feat: Phase 7 - Lua plugin runtime with WASM stub (dfb94f6)
- feat: Phase 6 - plugin API traits and registry (64a0fd8)
- feat: Phase 5 - tesela-mcp MCP server (6ab0a29)
- feat: Phase 4 - tesela-tui Elm-style TUI (0632567)
- feat: Phase 3 - tesela-cli thin dispatcher (67b8915)
- feat: Phase 2 - SQLite+FTS5 search index and indexer (1deff89)
- feat: Phase 1 - workspace + tesela-core foundation (9b137ff)
- chore: add .worktrees to .gitignore (d58cc42)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260319.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260319.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20250925.0...v0.20260319.0

---


## v0.20250925.0 - 2025-09-25 20:33:36 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---


## v0.20260319.0 - 2026-03-19 11:14:57 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---

## What's Changed

- update (d875c49)
- feat: Phase 7 - Lua plugin runtime with WASM stub (dfb94f6)
- feat: Phase 6 - plugin API traits and registry (64a0fd8)
- feat: Phase 5 - tesela-mcp MCP server (6ab0a29)
- feat: Phase 4 - tesela-tui Elm-style TUI (0632567)
- feat: Phase 3 - tesela-cli thin dispatcher (67b8915)
- feat: Phase 2 - SQLite+FTS5 search index and indexer (1deff89)
- feat: Phase 1 - workspace + tesela-core foundation (9b137ff)
- chore: add .worktrees to .gitignore (d58cc42)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260319.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260319.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20250925.0...v0.20260319.0

---

## What's Changed

- chore: bump version to 0.20250925.0 [skip ci] (b8ba386)
- init (e0e3c79)
- Clean up TUI module and test code (90681cf)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20250925.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20250925.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20250826.0...v0.20250925.0

---


## v0.20250826.0 - 2025-08-26 00:19:54 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---


## v0.20260319.0 - 2026-03-19 11:14:57 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---

## What's Changed

- update (d875c49)
- feat: Phase 7 - Lua plugin runtime with WASM stub (dfb94f6)
- feat: Phase 6 - plugin API traits and registry (64a0fd8)
- feat: Phase 5 - tesela-mcp MCP server (6ab0a29)
- feat: Phase 4 - tesela-tui Elm-style TUI (0632567)
- feat: Phase 3 - tesela-cli thin dispatcher (67b8915)
- feat: Phase 2 - SQLite+FTS5 search index and indexer (1deff89)
- feat: Phase 1 - workspace + tesela-core foundation (9b137ff)
- chore: add .worktrees to .gitignore (d58cc42)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260319.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260319.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20250925.0...v0.20260319.0

---


## v0.20250925.0 - 2025-09-25 20:33:36 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---


## v0.20260319.0 - 2026-03-19 11:14:57 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---


## v0.20260321.0 - 2026-03-21 13:20:20 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---


## v0.20260329.0 - 2026-03-29 21:08:31 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---


## v0.20260330.0 - 2026-03-30 21:56:40 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---


## v0.20260401.0 - 2026-04-01 19:04:47 UTC


## v0.20260402.0 - 2026-04-02 00:19:24 UTC

## What's Changed

- fix: bullet threading at child level + icon alignment polish (475b173)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260402.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260402.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260401.0...v0.20260402.0

---

## What's Changed

- feat: all tags shown as right-side pills only, with × to remove (acc3d87)
- feat: always show empty block at bottom for quick capture (f492c08)
- feat: tag autocomplete shows "New tag: name" for non-existent tags (329ddac)
- feat: backup system — CLI command + auto-backup on server startup (f43f219)
- revert: remove BoardView — will rebuild from React prototype (b7edc93)
- feat: Phase 2 — Life OS kanban board with domain swimlanes (9bd4a54)
- feat: Life OS data model — Domain, Issue, Ritual, ScheduledItem types (2176139)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260401.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260401.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260330.0...v0.20260401.0

---

## What's Changed

- fix: align bullet icon and status icon with text baseline (aebb3b5)
- fix: SF Symbol icon pushed down 2px more to align with text center (dfe4d97)
- fix: icon and status layout — better spacing and vertical alignment (8e5c861)
- feat: full SF Symbol catalog (3000+) with color picker for tag icons (7f66ee7)
- fix: SF Symbol icons without dots (camera, star, etc.) now render (34bd260)
- feat: SF Symbol icon picker on tag pages (8e0fc2a)
- fix: thread lines start from parent bullet, icons use SF Symbols (2607f6f)
- feat: indent thread lines connecting parent bullets to children (40f75b5)
- docs: update next-steps — multi-filter + custom icons done (09686d9)
- feat: custom bullet icons per type tag (c4784ec)
- feat: 13.8 multi-property filtering on tag pages (4696607)
- fix: restore left-click drill-in with custom BulletView (4accefb)
- feat: block properties in right sidebar + bullet right-click menu (a96e3a8)
- docs: update next-steps — search count, kanban DnD, node refs done (1f07ac6)
- feat: 13.7 node references — properties that link to pages (eae8f0c)
- feat: kanban drag-and-drop moves blocks between columns (dd2a0c7)
- feat: search match count display (3/12) in bottom-right status (7827152)
- fix: wire block drill-in from tiles page (7225e76)
- fix: nested block drill-in converts local index to full-page index (a51b611)
- fix: block drill-in uses flat index instead of UUID (e777699)
- feat: back/forward navigation + Logseq-style block drill-in (a5c4cde)
- fix: re-apply search highlighting after setting searchQuery on BlockView (0caa276)
- docs: update next-steps after search highlighting + kanban (87849e2)
- feat: kanban board view on tag pages (af0f265)
- feat: yellow search highlighting on /search matches (88c56ef)
- fix: /search Enter handling via event monitor, proper dismiss order (1060455)
- fix: /search keeps matches for n/N, backlinks show full line context (111f7b1)
- feat: unlinked references in right sidebar (f9e3ad2)
- fix: tag page filters, /search bar, and right sidebar backlinks (d9918ab)
- docs: update handoff docs after Vim + filters + sidebar sprint (85bfcda)
- feat: right sidebar polish — page info, grouped backlinks, context (d77e9fa)
- feat: tag page filters — filter by property, sort by column (236e7e9)
- feat: Visual mode, /search, and dot-repeat polish (400c3ee)
- fix: # autocomplete searches all pages, not just tags (dca86a7)
- fix: guard cursorRect() against out-of-bounds during text edits (82319ab)
- fix: autocomplete keeps BlockView focused, forwards only nav keys (f92e579)
- feat: inline autocomplete for #tags and [[page references]] (fcfa178)
- docs: update handoff docs after UI polish session (e9217ab)
- feat: tag display rework + status icon fix + date picker Enter fix (7b8b450)
- feat: add docs/ai/ shared handoff workflow for AI assistants (db4359f)
- feat: keyboard-navigable select popover for properties (34a08d3)
- feat: implement priority picker and effort input slash commands (7906d8c)
- fix: track focusedBlockIndex on becomeFirstResponder (273ea03)
- fix: add slash/space menu overlays to TilesView (dd07f3b)
- fix: preserve frontmatter when content already has it in store.create() (2f024e8)
- fix: activate window before showing popovers/alerts (f641590)
- feat: Phase H — property configuration UI + BlockStyler crash fix (5d743b1)
- fix: priority colors the status icon instead of separate emoji (0a9f3e6)
- fix: /types endpoint resolves property types from property_defs (0dd4ed9)
- fix: property editing widgets — popover for selects, date picker for dates (9ab919b)
- feat: Phase G — block drill-in for properties (57bd52b)
- fix: built-in pages written directly to preserve frontmatter (e1ccd4c)
- fix: typed blocks query searches body text for inline #tags (f798ae1)
- feat: Phase F — block property indexing with Rust block parser (c997923)
- fix: tiles auto-save was silently failing — updatePage required currentPage (fcf83bd)
- fix: preserve cursor position when tags trigger rebuild (a84e95c)
- fix: tag duplication during typing — updateDisplayText used raw regex (5ae610e)
- fix: tags only finalize after space — no partial tags mid-typing (3bc30e0)
- fix: tags only match after word boundary — no mid-typing extraction (82ec90d)
- fix: inline #tags create tag pills + auto-create tag pages (630bdae)
- fix: auto-create Task, Project, Person tag pages on server startup (cbf1267)
- feat: Phase D+E — property inheritance + tag page table view (dc0096c)
- feat: Phase B+C — property indexing, /properties API, Tag page view (c92e25b)
- feat: Phase A — tags auto-create pages, clickable tag pills, built-in pages (86090af)
- feat: SQLite schema v2 — tag_defs, property_defs, block_properties tables (2106c63)
- feat: Phase 13.3 — page types with type: field + types.toml + /types API (3ff3423)
- fix: tile navigation stays in Normal mode (96b6e8f)
- fix: { and } now focus target tile after scrolling (82255ba)
- feat: { and } navigate between tiles in Normal mode (aa7ab9a)
- docs: add todo list requirement to CLAUDE.md and AGENTS.md (6570e64)
- fix: tiles only show present and past dates, not future (31b42b5)
- fix: tiles expand to content height with generous buffer (34404a8)
- feat: inline tile editing — edit daily notes directly in timeline (1154deb)
- fix: clicking active nav item returns to list view (7bcf516)
- fix: bullet always shows, status icon sits to its right (6e14ffa)
- feat: clean display separation — Logseq DB style (27f4291)
- fix: Ctrl+R redo now works — use charactersIgnoringModifiers for Ctrl (d3c5202)
- feat: structural undo/redo for block operations (fe5d41e)
- fix: undo/redo falls back to window's undoManager (37a98b5)
- fix: all Space menu commands now work via generic command dispatch (d9d458a)
- fix: Space menu key forwarding + date picker Enter in calendar mode (569db05)
- fix: slash menu task commands + J join + date picker enter (446a419)
- fix: stability sprint — error alerts, crash fixes, dead code cleanup (e91c247)
- refactor: tasks as #Task tagged blocks with status:: property (78bd60e)
- feat: natural language date input in date picker + shared DateParser (d1e93db)
- feat: natural date search in ⌘K palette (a4f78d8)
- feat: dates as page links + edit button + server date param (7f534e9)
- fix: date picker has Set button + Enter key to accept (e3d14aa)
- fix: menu dismiss, multi-line blocks, visible properties (4828f58)
- feat: Phase 13.3 — slash commands + Space leader menu (9baee80)
- feat: date picker popover for deadline and scheduled properties (d0986d2)
- feat: Phase 13.2 — full task properties: priority, deadline, scheduled, effort (71cc127)
- fix: hide TODO/DOING/DONE text prefix, add ⌘Enter todo toggle (da75a6f)
- feat: Phase 13.1 — Task MVP: todo toggle with t key (81a32ff)
- feat: Phase 12.3 — UX polish: favorites, shortcuts, cursor, persistence (790e4f8)
- feat: Phase 12.1 — Vim polish: count prefix, dot-repeat, visual mode (35fd36a)
- fix: graph edges query — match lowercase 'internal' in DB (06706f8)
- feat: Phase 11.8 — interactive force-directed graph view (5a93de4)
- feat: Phase 11.7 — block property pills below block rows (efa254c)
- fix: increase wiki-link pill and tag pill opacity for dark mode (d826069)
- feat: Phase 11.6 — wiki-link pills + right-aligned tag pills (600acbc)
- fix: block cursor visible on empty blocks and link-only blocks (d7fc1fe)
- fix: block cursor persists across block navigation and structural edits (6b552cb)
- feat: Phase 11.5 — clickable backlinks in right sidebar (da957de)
- fix: start in Insert mode when opening a page (6b7e864)
- fix: block cursor visible immediately on focus (419b132)
- fix: Vim polish — colon menu, block cursor, undo, mode indicator (9dae52d)
- feat: Phase 11.4 — wire Vim mode into block outliner (9f3ab0c)
- feat: rename Journals → Tiles + scrollable daily notes timeline (d39b92b)
- feat: Phase 11.3 — search bar in page list + sidebar filter (6507c4b)
- fix: block creation via Enter + text rendering reliability (3659d33)
- fix: make block text visible by enabling rich text rendering (db62d32)
- fix: OutlinerView coordinate system + layout timing (35cc0de)
- fix: show content for notes without block-format bodies (b1128c6)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260330.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260330.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260329.0...v0.20260330.0

---

## What's Changed

- "Claude Code Review workflow" (9d52653)
- "Claude PR Assistant workflow" (ae284b7)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260329.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260329.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260321.0...v0.20260329.0

---

## What's Changed

- fix: wire PageListView and GraphView into ContentArea (76df988)
- chore: remove test notes from repo, ignore notes/ and attachments/ (9cf3993)
- feat: Phase 11.2 — WebSocket fix + block outliner editor (52066df)
- docs: add single-command-per-Bash rule to CLAUDE.md and AGENTS.md (ae7348f)
- feat: make Tesela desktop minimally functional for QA (3147aac)
- fix: update axum route syntax from :id to {id} for axum 0.8 (f171b26)
- feat: Phase 11 — native macOS SwiftUI app foundation (11.1) (d522717)
- feat: Phase 10 — tesela-server local REST/WebSocket API (dacb6fe)
- docs: require QA checklist after every TUI feature implementation (0a500ac)
- feat: Phase 9 — tag filtering, live search, inline editing (8a0ea22)
- feat: add Nerd Font icons throughout TUI (4be6865)
- feat: TUI daily-driver polish — delete confirmation, search nav, timestamps (b43ef30)
- feat: open daily note on startup, help is now a modal overlay (dfecd82)
- feat: TUI visual polish — theme, rounded borders, fuzzy highlighting (2ca9506)
- fix: use UPDATE+INSERT instead of INSERT OR REPLACE in upsert_note (e352b01)
- feat: Phase 8 - daily driver TUI, CI, legacy cleanup (ec5fd4a)
- docs: update RELEASE.md for v0.20260319.0 [skip ci] (5a37510)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260321.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260321.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20260319.0...v0.20260321.0

---

## What's Changed

- update (d875c49)
- feat: Phase 7 - Lua plugin runtime with WASM stub (dfb94f6)
- feat: Phase 6 - plugin API traits and registry (64a0fd8)
- feat: Phase 5 - tesela-mcp MCP server (6ab0a29)
- feat: Phase 4 - tesela-tui Elm-style TUI (0632567)
- feat: Phase 3 - tesela-cli thin dispatcher (67b8915)
- feat: Phase 2 - SQLite+FTS5 search index and indexer (1deff89)
- feat: Phase 1 - workspace + tesela-core foundation (9b137ff)
- chore: add .worktrees to .gitignore (d58cc42)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20260319.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20260319.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20250925.0...v0.20260319.0

---

## What's Changed

- chore: bump version to 0.20250925.0 [skip ci] (b8ba386)
- init (e0e3c79)
- Clean up TUI module and test code (90681cf)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20250925.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20250925.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20250826.0...v0.20250925.0

---

## What's Changed

- chore: bump version to 0.20250826.0 [skip ci] (e9cce35)
- Pass version tag from release job to binary builds (2168ea1)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20250826.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20250826.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/v0.20250825.0...v0.20250826.0

---

## What's Changed

- Change version format to 0.YYYYMMDD.N (3beed1a)
- Bump version to 0.20250825.0 (ac15f2b)
- Update versioning strategy and roadmap sections (0413b07)
- Move installation docs from RELEASE.md to README.md (4538211)
- Replace interactive mode with TUI command (18fe790)
- update (91df520)
- update (3ba1fa7)
- init cargo (5f4a670)
- init cargo (29ff11f)
- Add macOS linking configuration for Rust tests (06e4e25)
- Update project roadmap and features documentation (3707051)
- Add extensive project documentation and roadmap (9579b7a)
- init cargo (51d60cd)
- Add work in progress section to README with learning journey context (4ed30c8)
- Initial commit (bd73c7d)

---

### Installation

#### Linux x64
```bash
curl -L https://github.com/TaylorFinklea/tesela/releases/download/v0.20250825.0/tesela-linux-x64 -o tesela
chmod +x tesela
sudo mv tesela /usr/local/bin/
```

#### From source
```bash
cargo install --git https://github.com/TaylorFinklea/tesela --tag v0.20250825.0
```

**Full Changelog**: https://github.com/TaylorFinklea/tesela/compare/...v0.20250825.0

---
