# First-Class Dates (web) — design

*2026-05-22. Brainstormed + approved. Web client only — iOS is a later, separate effort.*

## Problem

A date on a task in the web client is stored as a raw inline `[[YYYY-MM-DD]]` wiki-link in the block text (the `/date` slash command inserts one). This is the root of four symptoms the user hit:

1. The date is un-editable after it's set — it's just link text, no affordance to change it or see/alter the recurrence.
2. Recurrence can only be set by mouse (the calendar popover); typed natural language isn't applied.
3. There is only one generic "date" — no `deadline` vs `scheduled` distinction in the UI, though the engine supports both.
4. Recurrence doesn't work end-to-end: a `/date`-set date writes no `recurring::` property, so skip reports "no recurring task focused".

The Rust engine already has the right model — it reads `deadline::` / `scheduled::` / `recurring::` *block properties*. The web UI never produces them from the obvious flow. This redesign aligns the web client with that property model.

## Decisions (from brainstorming)

- **Treatment: a properties row.** A task with date/recurrence shows a compact labelled strip beneath it (option C of three mocked treatments).
- **Entry: one command + an NL parser.** A single command opens one freeform natural-language input; the parser extracts date, recurrence, and which field. (Not per-field commands, not an inline sigil.)
- **Inline `[[date]]` links are killed as a date mechanism.** The user never authors date links by hand; the inline-link-as-date is the actual source of the "too much complexity". A date becomes a typed property value, not a link.
- **No journal backlink.** A dated task does NOT auto-surface on that day's daily journal page. The daily journal stays a record of what the user wrote that day; "what's due/scheduled on a day" is answered by the agenda / query views (a separate effort). This also means the stored value carries no `[[...]]`, so the indexer creates no backlink.
- **Bare-date default field is configurable.** Typing a bare date with no `deadline`/`scheduled` keyword sets the field named by a new setting; the setting's default is `scheduled`.

## 1. Data model

A date on a block is a `date`-typed block property holding a bare ISO-8601 scalar:

```
- do this thing
  scheduled:: 2026-05-25
  recurring:: every mon, fri
  tags:: Task
```

- `deadline:: 2026-05-25` and `scheduled:: 2026-05-25` — bare `YYYY-MM-DD`, no brackets, no link semantics, no backlink-index entry.
- `recurring:: <grammar>` unchanged — the engine's extended grammar (`every mon, wed, fri`, `weekends`, ` until <date>` / ` count N`).
- `recurrence_done::` is engine-maintained (untouched by this work).
- The inline-`[[YYYY-MM-DD]]`-link-as-a-date mechanism is removed: the `/date` slash command no longer inserts a link into block text. A `[[2026-05-23]]` a user types by hand remains an ordinary page link — the date tooling simply never creates one.

## 2. The command + NL grammar

One command — the reworked `/date` slash entry. It opens a single NL text input (a calendar renders alongside as an optional visual aid; typing is the fast path). On commit it **upserts a property on the current block** (`upsertBlockProperty` already exists) — it does not insert text.

Grammar extends the existing `parseDateAndRecurrenceInput` (`web/src/lib/date-parser.ts`):

- **Date phrases** (already parsed): `today`, `tomorrow`, `yesterday`, `next friday`, `fri`, `may 23`, `2026-05-23`, `5/23`, `in 3 days`, `3d` …, optionally with a time (`fri at 10am`).
- **Field keyword** (new): a leading `deadline` / `scheduled` (and `due` → deadline) routes the date to that property. A bare date with no keyword routes to the field named by the settings default (§5).
- **Recurrence** (already parsed): a trailing recurrence phrase — `every day`, `weekdays`, `every mon, fri`, `… until 2026-09-01`, `… count 10`.
- **Combined**: `tomorrow and every saturday` → `scheduled:: <tomorrow's date>` + `recurring:: every saturday`. `deadline next friday` → `deadline::` only. `every day` with no date → `recurring:: daily` + the **default field** (§5 — `scheduled` by default) set to today, so the engine has an anchor to bump.

The command is reachable from the `/` slash menu's "Date" entry (reworked).

## 3. The properties row

Beneath any block carrying a date or recurrence, render a compact labelled strip — e.g. `SCHEDULED May 25 · DEADLINE … · REPEAT Mon, Fri`. Only the set fields render; a block with none shows no row. Treatment C from brainstorming.

- Each field is **click-to-edit**: clicking re-opens the date command, pre-filled with that field's current value, scoped to that field.
- A date value renders human-readably (`formatDateMonthDay`, already exists) and is **click-to-open-that-day's-journal** — a pure navigation affordance, no stored link.
- The `REPEAT` field shows the human-formatted recurrence (`formatRecurrence`, already built) and carries the **Skip** action (the `skipRecurrence` helper, already built).
- The row reads from the block's parsed `properties` (`deadline`/`scheduled`/`recurring`) — no new storage.

## 4. Recurrence + skip

No engine work — the recurrence engine, the `recur-bump` `complete|skip` endpoint, and the `skipRecurrence` helper all already exist. This redesign's contribution is that the UI now actually produces blocks carrying `recurring::` **plus** an anchor `scheduled::`/`deadline::`, which is what makes completion-bump and skip function. The `REPEAT` field is the in-context surface for Skip; the existing `skip` verb (Cmd+K / `:` / leader) continues to work.

## 5. Settings

A new web setting: **"Bare date sets…"** with values `deadline` | `scheduled`, default **`scheduled`**. It controls which property a keyword-less date phrase routes to. Lives in the existing web settings surface; persisted the way other web settings are.

## 6. Migration

No automatic data migration.

- Existing `deadline:: [[2026-05-23]]` properties (the old `/p` flow wrote bracketed values) keep working — the date renderer accepts bracketed or bare; new writes are bare, so values normalize on next edit.
- Existing inline `[[YYYY-MM-DD]]` links in old task text are left as ordinary plain links. We do not guess whether a given inline date meant a deadline or a scheduled date, so we do not auto-convert them. The user re-sets the date via the command if desired.

This keeps the change low-risk — no bulk rewrite of existing notes.

## 7. Out of scope

- The agenda / "today" view — its own effort; this design deliberately removes journal backlinks *because* the agenda view is the right home for "what's due today".
- iOS — the user asked to get web solid first.
- Any bulk migration of existing inline date links.
- Block kinds other than tasks setting dates — the command works on any block, but dates are a task concern in practice; no special handling.
