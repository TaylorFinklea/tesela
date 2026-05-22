# Agenda / Today View (web) — design

*2026-05-22. Brainstormed + approved. Web client only — iOS is a later, separate effort.*

## Problem

There is no surface that shows "everything due/scheduled today and overdue, across the whole mosaic, sorted by time" — task data is scattered across daily notes, tag pages, and the kanban. The First-Class Dates redesign (shipped earlier today) deliberately removed the journal-backlink behavior on the understanding that a dedicated agenda view would be the right home for "what's due on a day." This is that view.

## Decisions (from brainstorming)

- **Time window: scrollable-forward calendar.** Overdue at the top, then today, then every future day indefinitely as the user scrolls. (Past days are out of scope for v1.)
- **Recurrence: project forward.** A `recurring:: weekly` task with `scheduled:: 2026-05-22` appears on 5/22, 5/29, 6/5, … as the user scrolls — the agenda is a real calendar of upcoming occurrences, not just the next anchor.
- **Actions: inline status + inline reschedule.** A task's checkbox in the agenda flips `status:: done` (and the recurrence engine bumps as usual). A task's date is editable in place (click the date → open the DatePicker, or drag the row to another day).
- **Scope of blocks: tasks + events, tasks foregrounded.** Tasks (blocks with `tag:Task` or a `status::`) render with a checkbox. Non-task blocks that carry `scheduled::` (events — e.g. `standup with Maya scheduled:: 5/22 14:00`) render as plain rows in the same day buckets. Both surface; tasks are the daily-driver primary.
- **Done tasks: hidden by default, with a toggle.** The agenda is about what's ahead; the toggle (in the agenda header) reveals completed tasks struck-through.

## 1. Where it lives — a new ambient buffer

The agenda is a **workspace-singleton ambient buffer** (`{ kind: "ambient"; ambientName: "agenda" }`). This is the established pattern for Calendar / Dashboard / AI Workspace — they live alongside pages in the pane tree, can be split/tabbed, and have a registered renderer.

- New directory: `web/src/lib/ambients/agenda/` with `index.ts` (the `AmbientRenderer`) + `Agenda.svelte` (the component).
- Registered in `web/src/lib/renderers/register.ts` (the existing list of 4 ambients gains a 5th).
- Opened via a new `:agenda` verb added to the `AMBIENTS` array in `web/src/lib/v4/commands.ts` — reachable from ⌘K / `:agenda` / leader, same way the other ambients are.
- The existing thin `today-in-progress` ambient stays in the codebase for now (a separate cleanup can retire it later; not in this spec's scope).

## 2. Data — projected blocks across a window

The agenda renders **expanded occurrences** of blocks within a `[from, to]` window. Three sources flow into the same render:

1. **Tasks** — undone task blocks (`tag:Task` or `has:status -status:done`) that fall in the window.
2. **Events** — non-task blocks with `scheduled::` that fall in the window.
3. **Projected recurrences** — for each block with `recurring::`, expand future occurrences (using the engine's `next_after` / `advance`) into individual agenda rows, bounded by the block's `until`/`count` and the visible window.

Each agenda row is a tuple `{ block_id, source_note_id, occurrence_date, occurrence_time | null, kind: "task" | "event", overdue: bool, recurrence: string | null, is_anchor: bool }`. `is_anchor` is `true` for the block's current `deadline::`/`scheduled::` value (or for any non-recurring row) and `false` for projected future occurrences of a recurring block — the renderer uses this to gate which rows are markable-done.

### 2a. New server endpoint

Add `POST /agenda` (or `GET /agenda?from=YYYY-MM-DD&to=YYYY-MM-DD`) in `tesela-server`. The server is the right home for projection: the canonical Rust recurrence engine is already there, the SQLite index already has `deadline::`/`scheduled::`/`recurring::` indexed, and the client doesn't need a JS mirror of the recurrence math. The endpoint:

- Reads blocks where `has:deadline OR has:scheduled` within the window (the existing query engine handles this).
- For each block with `recurring::`, calls `recurrence::next_after` repeatedly to expand occurrences within the window, anchored to the block's `scheduled::` (or `deadline::` if no `scheduled::`).
- Drops occurrences past `until` or beyond `count` (using `advance` — already the gate the engine uses).
- Returns the flat list of `AgendaRow` records sorted by `(occurrence_date, occurrence_time, block_id)`.

A reasonable default window for the initial fetch: `[today - 0 days, today + 60 days]`. Scrolling further forward extends the window via additional fetches.

### 2b. Why a dedicated endpoint, not client-side projection

Projection in JS would duplicate the recurrence engine (the Rust side is the source of truth), require shipping `recurring::` plus anchors for every recurring block, and re-implement `next_after`/`advance` semantics. A server endpoint mirrors the existing `calendar_marks(from, to)` pattern, keeps the recurrence math in one place, and is a small handler. (`calendar_marks` returns *counts*; the agenda endpoint returns the expanded *rows*.)

## 3. The component — `Agenda.svelte`

A single scrollable column rendered in the ambient slot, structured as:

```
[ Overdue ]          ← sticky-ish section if non-empty; rows ⚑ in orange tint
[ Today · Fri May 22 ]
  □ ⚑/🕒 [time]  task text                in [[source]]    ↻ <recurrence>
  · 🕒 [time]    event text               in [[source]]
[ Tomorrow · Sat May 23 ]
  □ 🕒  groceries                          in [[shopping]]  ↻ weekly
[ Mon May 25 ]
  □ 🕒  weekly review                      in [[reviews]]   ↻ every monday
[ Tue May 26 — empty ]
[ Wed May 27 ]
  □ ⚑  draft Q3 plan                       in [[planning]]
…
```

Per the mockup the user approved:

- **Overdue section** at top, only when non-empty. Rows show their original date with ⚑ tinted orange.
- **Day headers** — `Today · <weekday, mon d>` / `Tomorrow · …` / `<weekday, mon d>` for further days. Days with no items render as a muted "Day — empty" line so the scroll has texture (skip-rendering would make the scroll feel uneven). Optionally collapse runs of empty days later (out of scope for v1).
- **Row** — `<checkbox if task> <icon: ⚑ deadline | 🕒 scheduled> <time-or-date> <text> <source pill> <recurrence chip>`.
  - The **checkbox** is only rendered for task rows AND only when `is_anchor === true`. Projected future occurrences of a recurring task show no checkbox — you can't complete a single future instance without a per-occurrence-override feature (out of scope, see §6). Clicking the checkbox on a non-recurring task or on a recurring task's current anchor toggles `status::` via `upsertBlockProperty(..., "status", "done")` and saves through the existing block-save path; the recurrence engine bumps the anchor on done, so a recurring task "moves" to its next date.
  - The **icon** distinguishes deadline (⚑, orange when overdue) from scheduled (🕒). A block with both renders both — but the agenda projects one row per (block, kind) per day; in practice most blocks carry one or the other.
  - The **time** is `HH:MM` when the block has one (`scheduled:: 2026-05-22 14:00`), else the date label (`May 22`). Rows with a time sort first within a day; date-only rows follow.
  - The **source pill** (`in [[page-title]]`) is click-to-open the source note — uses the canonical `gotoNote` navigation (the same hook `BlockDateRow` uses).
  - The **recurrence chip** shows the human form (`formatRecurrence` — already built) when the block is recurring, so the user knows this row is a projected occurrence.
- **Header** — small toolbar at top: a "hide done" toggle (off by default; on reveals done tasks struck-through within the visible window) and a date-range indicator.
- **Infinite scroll** — when the user scrolls near the bottom of the loaded window, extend the window forward and fetch more.

## 4. Interactions

- **Mark done** — only the current-anchor row of a task (recurring or not) has a clickable checkbox. Clicking it does `upsertBlockProperty(blockText, "status", "done")` → save → the engine bumps recurring blocks to their next occurrence (`apply_post_save_bumps`); the row disappears (or strikes through if "show done" is toggled on) and the recurring task re-appears at its new anchor. Projected future rows of a recurring task are read-only — you can't tick a future instance.
- **Reschedule — click the date** → opens the `DatePicker` for that block in the same pre-filled mode `BlockDateRow` uses → on commit, the underlying block's `scheduled::`/`deadline::` is upserted. The agenda re-fetches and re-renders. For a *projected* recurring occurrence, editing the date on a projected row asks: "edit just this occurrence" (an exception — not in v1) or "edit the series" (changes the underlying block). **v1 ships only "edit the series"** (single occurrence overrides are a separate, larger feature).
- **Reschedule — drag a row to another day** — defer this to v1.1. Click-the-date covers the same need with less DnD complexity.
- **Click the text / source pill** — opens the source note in the focused split (`gotoNote(source_note_id)`).
- **Skip a recurring task** — only on the current-anchor row (matching the mark-done semantics). A small `⏭` button next to the recurrence chip calls `skipRecurrence(block_id)` (the existing helper from the recurrence-clients work). Projected future rows show no skip button.

## 5. Component decomposition

- `Agenda.svelte` — top-level ambient component. Owns the date-window state, the "hide done" toggle, the infinite-scroll handler, the data fetch.
- `AgendaDay.svelte` — one day section (header + rows or "empty" placeholder).
- `AgendaRow.svelte` — one row (checkbox + icon + time + text + source + recurrence chip). Reuses `formatDateMonthDay` and `formatRecurrence` (already shared modules).

## 6. Out of scope

- Past days (scrolling backward through history).
- Per-occurrence overrides on recurring tasks ("change just this Monday's review").
- Drag-to-reschedule (click-the-date covers the need).
- Settings (no agenda-specific preferences in v1).
- iOS — the iOS agenda is a separate effort.
- Bulk operations (multi-select).
- Retiring the `today-in-progress` ambient (small cleanup, separate change).
