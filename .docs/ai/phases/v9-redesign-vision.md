# Tesela v9 — Full Redesign Vision

## North Star

Tesela v9 is a top-to-bottom redesign anchored on the design package at `.docs/designs/v9-columns/columns/Tesela v9 - Columns (Tokyo Night cohesive).html`. The end state replaces the current 3-region layout (left sidebar / outliner / right sidebar) with a 4-region columns layout (rail / middle list / focus / bottom drawer), adopts Tokyo Night as the only theme, and ships several new product concepts (saved-query widgets, calendar, inbox, history, linked tasks).

Vim mode, the outliner block model, drill-in, splits, ⌘K, slash commands, and leader menu all carry over. The redesign is visual+structural, not a rewrite of the editor.

## Approved Decisions (locked from product Q&A on 2026-04-29)

| Question | Decision |
|---|---|
| Scope | Full IA shift + new product features (not cosmetic-only) |
| Theme | Tokyo Night replaces all 6 current themes; theme system removed |
| Widget rail = ? | The rail IS the surface for the planned **Queries/Sets** roadmap item; building v9 ships Queries/Sets too |
| New product features (all real) | Calendar widget, Inbox widget, History tab, Linked Tasks tab |
| First slice (v9.0) | Layout shell first — rail+middle+focus+drawer skeleton, current nav mapped 1:1 into rail, Tokyo Night theme |
| Phasing principle | Plan toward the full vision; intermediate states are throwaway, don't over-design them |

## Target Layout

```
┌─────────────────────────────────────────────────────────────────────┐
│ crumb bar (32px) — breadcrumb · keyboard hints (⌘K · ^w hjkl · b)   │
├──────────┬─────────────┬────────────────────────────────────────────┤
│ rail     │ middle      │ focus                                      │
│ (232px)  │ (300px)     │ (1fr)                                      │
│          │             │                                            │
│ widgets  │ list view   │ outliner / detail                          │
│  Today   │  of active  │  type/status badges                        │
│  Tasks   │  widget     │  inline property chips                     │
│  Inbox   │  with       │  block content (vim outliner unchanged)    │
│  Calendar│  grouping   │                                            │
│  ...     │  by status/ │                                            │
│          │  date/etc.  │                                            │
│ ─────    │             │                                            │
│ mini cal │             │                                            │
│ (pinned) │             │                                            │
├──────────┴─────────────┴────────────────────────────────────────────┤
│ bottom drawer (220px, toggleable) — tabs:                           │
│ Backlinks · Properties · History · Outline · Linked tasks           │
├─────────────────────────────────────────────────────────────────────┤
│ status bar (24px) — mode · context · keys                           │
└─────────────────────────────────────────────────────────────────────┘
```

`^w h/j/k/l` traverses rail / focus / bottom / middle (extending the current 3-region pane state machine in `web/src/lib/stores/pane-state.svelte.ts`). Splits within the focus pane (multiple side-by-side outliners) are a v9.X advanced feature, not v9.0.

## Visual Language

Source: `.docs/designs/v9-columns/columns/v9-styles.css`. Lift the CSS variables wholesale into `web/src/app.css`:

```css
--bg: #1a1b26;       --bg-2: #1f2335;   --bg-3: #24283b;   --bg-4: #2a2e42;
--line: #2f334d;     --line-soft: #292e42;
--ink: #c0caf5;      --ink-2: #a9b1d6;  --ink-3: #737aa2;  --ink-faint: #545c7e;
--amber: #ff9e64;    /* primary accent */
--rose: #f7768e;     /* task */
--indigo: #7aa2f7;   /* project */
--plum: #bb9af7;     /* person */
--sage: #9ece6a;     /* query / tag */
--ochre: #e0af68;    /* recent / clock */
--teal: #7dcfff;     /* event / inbox */
--amber-2: #ffc777;  /* note / kbd hint */

--font-sans: "Inter Tight", system-ui, sans-serif;
--font-mono: "JetBrains Mono", ui-monospace, monospace;
```

Body: 13px / 1.5 line-height / Inter Tight. Mono surfaces (status, kbd, breadcrumbs, badges, property chips, list metadata): JetBrains Mono.

Kind colors apply globally — wherever a block, page, or list row is associated with a kind, use that kind's swatch:
- `task` → rose, `project` → indigo, `person` → plum, `query` → sage, `recent` → ochre, `inbox`/`event` → teal, `note` → amber-2.

## New Product Concepts (all in scope for the full vision)

### Saved-Query Widgets (Queries/Sets feature)
A widget = `{ id, name, kind: "query" | "calendar" | "today" | "inbox" | "recent" | "pinned" | "pages", query?: QuerySpec, icon?, color? }`.
- `QuerySpec` = filter expression (type, tag, properties, status, etc.) + optional grouping (by status / due date / property) + optional sort.
- System widgets (Today, Tasks, Projects, People, Inbox, Calendar, Recent, Pinned, Pages) ship pre-defined.
- User-authored saved queries appear as widgets in the rail's "Saved" section.
- Middle column renders the query result as a grouped list (DOING / TODAY / THIS WEEK / etc.) with priority + due-date metadata. Selection drives the focus column.

### Calendar Widget
Mini calendar pinned at the bottom of the rail. Marks days with: tasks (rose dot), events (teal), notes (amber-2). Today highlighted, click-to-jump-to-daily-note. **Requires defining "event"** — proposal: any block tagged with a future-dated `deadline` or `scheduled` property surfaces as an event. Events without an explicit `event::` tag are inferred from the date property.

### Inbox Widget
Surfaces untriaged items. **Needs a definition** — proposal: blocks/pages with no parent project, no status, and not part of a daily note. Triage = move-to-project, set-status, or mark-archived.

### History Tab (bottom drawer)
Edit history of the focused page. Backend support needed — the existing tesela-server doesn't track per-page version history today. Phase plan: introduce a per-note edit-event log table in SQLite; every PUT writes a version row; the History tab queries the latest N versions of the focused note.

### Linked Tasks Tab (bottom drawer)
Tasks elsewhere in the corpus that reference (link to / are scoped to / are tagged with) the focused page. Tractable with current FTS5 + link-graph indexes — extends the existing backlinks query to filter by `kind=task`.

### Inline Keyboard Hints (crumb bar)
Dynamic per-view help on the right side of the breadcrumb. Examples: `⌘K jump · ^w + hjkl split · b bottom`. Hints come from the active widget context + global mappings. Lightweight; mostly a static lookup table per widget kind.

### Block Kind Glyphs in Content
Block content like `TASK Try BM25 with column weights` renders the leading tag as a colored badge prefix. Replaces today's `#Task` inline link styling for primary block kind. Subtags still render as `#tag` links.

### Parent Breadcrumb in List Rows
Each row in the middle column has a sub-line showing its parent breadcrumb (`↳ Daily 04-29`, `↳ Outliner Refresh`, `↳ Claire Rodriguez`). Comes from the block's containing-page lineage in the link graph.

## What Carries Over Unchanged

- **Vim mode**: every keybinding and operator (dd, yy, p, o, O, >>, <<, V, Y, etc.). Phase 3M.x undo work all transferable.
- **Outliner block model**: block parsing, drill-in, fold, multi-block selection.
- **⌘K command palette**: stays as the universal launcher; minor visual reskin.
- **Slash commands**: unchanged behavior; visual reskin.
- **Leader menu**: unchanged behavior; visual reskin.
- **3-region pane state** (`pane-state.svelte.ts`): generalizes to 4-region (rail/middle/focus/bottom) — the regions get renamed but the chord-traversal logic stays.
- **WebSocket reactivity, save debounce, undo system** (Phase 3M-3M.2): all backend-agnostic, transferable.
- **Tag/Property/Value-as-pages model**: stays.

## Phasing — Full Vision Delivery

Each phase is independently shippable. Phase 9.0 must visually look complete (Tokyo Night, columns layout, all widgets present even if some are stubbed). Subsequent phases fill in real behavior under the same shell.

### Phase 9.0 — Columns Shell + Tokyo Night
**Goal:** the v9 layout and visual identity. No new product features. Existing nav lives inside the new shell.

- Remove all 6 existing themes; lift `v9-styles.css` tokens into `web/src/app.css`.
- Replace `+layout.svelte` shell with the 4-region grid (rail + middle + focus + bottom + status bar + crumb bar).
- Map current sidebar nav into the rail (Today, Timeline, Graph, Pages, Properties, Favorites, Recent — kind-glyphs from the v9 palette).
- Migrate right-sidebar contents (backlinks, properties, forward links) into the bottom drawer as tabs (Backlinks, Properties, Outline). Bottom drawer toggleable via `b` chord.
- Apply Tokyo Night palette + JetBrains Mono / Inter Tight fonts to all existing components (BlockEditor, BlockOutliner, CommandPalette, LeaderMenu, etc.).
- Inline keyboard hints in the crumb bar (static for v9.0 — same hints regardless of view).
- `^w h/j/k/l` traverses rail / middle / focus / bottom drawer. `b` toggles bottom drawer.
- Drop the kanban split (we'll bring it back later as a focus-pane split, not a main-area swap).

**Out of v9.0:** widget query system, calendar, inbox, history, linked tasks, block kind glyphs, parent breadcrumbs, dynamic keyboard hints, focus-pane splits.

### Phase 9.1 — Saved-Query Widgets (Queries/Sets)
- `QuerySpec` data model in `tesela-core` + corresponding TypeScript types.
- Server endpoint to execute a query against the index → list of pages/blocks.
- Widget registry (system + user-authored) — stored as YAML pages following the everything-is-a-page principle.
- Middle column renders query results with grouping + sort.
- Block kind glyphs (TASK prefix etc.) in the focus pane.
- Parent breadcrumb in list rows.
- The v9.0 rail nav items are reframed as system query widgets (Today = `kind:daily date:today`, Pages = `kind:page`, etc.).

### Phase 9.2 — Calendar + Inbox Widgets
- Calendar widget: define "event" as any block with a `scheduled` or `deadline` property. Day cells show dot markers from a per-day query. Click-to-jump to daily note.
- Inbox widget: define "untriaged" with one or two predicates we like; ship a triage flow (set-status / move-to-project / archive) accessible via leader-menu and slash commands.

### Phase 9.3 — History + Linked Tasks Tabs
- Per-note version log in SQLite; PUT handler writes a version row.
- History tab queries last N versions of focused note; allows preview / revert.
- Linked Tasks tab: backlinks filtered by `kind:task`, shown with status + due date.

### Phase 9.4 — Polish
- Dynamic per-view keyboard hints in the crumb bar.
- Mini calendar polish (legend, multi-day navigation, keyboard shortcuts to advance month).
- Focus-pane splits (multiple side-by-side outliners).
- Drag-to-rearrange widget rail (or keyboard-only equivalent).

## Files to Reference

- Design source: `.docs/designs/v9-columns/columns/Tesela v9 - Columns (Tokyo Night cohesive).html`
- v9 CSS tokens: `.docs/designs/v9-columns/columns/v9-styles.css`
- v9 React reference: `.docs/designs/v9-columns/columns/v9-app.jsx`
- Other variants for inspiration: `.docs/designs/v9-columns/columns/v6-styles.css`, `v7-styles.css`, `v8-styles.css`
- Preview: `.docs/designs/v9-columns/columns/v9-preview.jpg`

## Roadmap Update

After this vision is approved, update `.docs/ai/roadmap.md`:
- Add a new top-level "Phase 9: Redesign" section above current Phase 3 Power Features.
- Move "Queries / Sets" from Phase 3 into Phase 9.1.
- Move "Right sidebar: pin pages for split view" / "Right sidebar: keyboard property editing" into the v9.x backlog (right sidebar is replaced by bottom drawer).
- Drop "Phase 4: Distribution / Tauri Wrap" reordering — still belongs after Phase 9.
- Note: the 6 existing themes will be removed in Phase 9.0.

## Open Questions for Future Phases

- "Inbox" definition needs nailing down before Phase 9.2.
- "Event" as a primitive vs. inferred from date property — probably inferred, but worth revisiting at Phase 9.2 design time.
- History tab UX: timeline vs. diff vs. side-by-side preview — design decision deferred to Phase 9.3.
- Mobile/responsive: out of scope for v9; redesign assumes desktop-first.

## Carry-Forward from Phase 3M.2

Two open issues from the just-shipped 3M.2 work that should be revisited AFTER v9.0:
- **Cmd+Z bleed-through in vim mode** (memory: `project_post_redesign_followups.md`)
- **Cancel-and-flush vs redo race** (memory: `project_post_redesign_followups.md`)

Neither blocks v9.0; both should be on the v9.0 verification checklist.
