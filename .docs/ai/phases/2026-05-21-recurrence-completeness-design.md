# Recurrence Completeness — design

*2026-05-21. Brainstormed + approved. Closes the deferred `12.2.x` recurrence
gaps from the roadmap.*

## Goal

The recurrence engine (`tesela_core::recurrence`) ships `daily / weekly /
monthly / yearly / weekdays / every N <unit>`. Five gaps remain, all
user-confirmed as the next task-management slice:

1. **BYDAY day-sets** — `every mon, wed, fri`
2. **`until` / `count` end conditions**
3. **Skip this occurrence**
4. **Recurring on `scheduled::`**, not only `deadline::`
5. **`weekends` keyword**

## Decisions (from brainstorming)

- **Skip UX** — surfaced two ways over one backend action: a `skip` verb
  (Cmd+K / `:skip` / leader menu) **and** a "Skip to next" item in the
  `recurring::` chip menu.
- **Series end** — when a series is exhausted (`count` reached or next
  occurrence past `until`), the task **stays a normal `done` task**.
  `recurring::` remains on the block as an inert record; re-opening it does
  not roll it forward. No property is auto-stripped, no terminal flag — this
  matches how an ordinary task ends and respects files-are-truth.
- **Recurrence anchor** — completing or skipping a recurring task advances
  **every date field present** (`deadline::` and `scheduled::`), each by the
  recurrence step, preserving the gap between them. A `scheduled::`-only task
  therefore recurs on `scheduled::` with no new syntax.

## 1. Model

Replace the flat `Copy` enum with a struct. `by_weekday` is a `Vec` (not
`Copy`) and `until`/`count` are orthogonal to frequency, so the enum can't
carry them cleanly. The struct shape maps 1:1 onto `EKRecurrenceRule`
(frequency / interval / daysOfTheWeek / recurrenceEnd), keeping the Apple
Reminders round-trip clean.

```rust
pub struct Recurrence {
    pub freq: Freq,              // Daily | Weekly | Monthly | Yearly
    pub interval: u32,           // >= 1
    pub by_weekday: Vec<Weekday>,// empty = anchor on the date's own weekday/day-of-month
    pub end: Option<RecurrenceEnd>,
}
pub enum Freq { Daily, Weekly, Monthly, Yearly }
pub enum RecurrenceEnd { Until(NaiveDate), Count(u32) }
```

Collapsed mappings:
- `weekdays` → `Weekly` + `by_weekday = [Mon..Fri]`
- `weekends` → `Weekly` + `by_weekday = [Sat, Sun]`
- `every mon, wed, fri` → `Weekly` + `by_weekday = [Mon, Wed, Fri]`
- `every N days` → `Daily` + `interval = N`

**Alternative considered & rejected:** bolt new variants onto the existing
enum. `until`/`count` would have to be duplicated across every variant or
live in a parallel wrapper — messier than the struct and a worse fit for
the EK mapping.

## 2. Syntax — `recurring::` grammar

The natural-language parser extends; every form that parses today still
parses. New forms:

- **BYDAY**: `recurring:: every mon, wed, fri` — three-letter or full
  weekday names, comma-separated. Bare `every monday` (single day) also
  valid. Implies weekly cadence.
- **`weekends`**: `recurring:: weekends` — sibling of the existing
  `weekdays`.
- **End conditions**: append ` until <YYYY-MM-DD>` or ` count <N>` (N ≥ 1)
  to any recurrence string:
  - `recurring:: weekly until 2026-12-31`
  - `recurring:: every mon, fri count 12`
  - `recurring:: every 2 weeks until 2027-01-01`

`count` is rrule-standard: total occurrences including the first.
`until` is inclusive of occurrences falling on that date.

## 3. Engine behavior

`tesela_core::recurrence` — pure module, no I/O.

- **`parse(&str) -> Option<Recurrence>`** — extended grammar above.
- **`next_after(&Recurrence, anchor: NaiveDate) -> NaiveDate`** — when
  `by_weekday` is non-empty, return the soonest weekday in the set strictly
  after `anchor` (stepping into the next eligible week and honoring
  `interval` for the week count). When empty, behaves as today.
- **Series-end** — `until` is stateless: if `next_after(...)` lands past the
  `until` date, the series is spent. `count` needs progress state: an
  **engine-maintained companion property `recurrence_done::`** (an integer,
  occurrences completed). Owned by the engine the same way
  `apple_reminder_synced_at::` is — the user never types it. When
  `recurrence_done + 1 >= count` on a completing occurrence, the series is
  spent: no bump, the task stays `done`, `recurrence_done` is set to `count`.
- **Multi-field anchor** — `apply_post_save_bumps` advances *each* of
  `deadline::` and `scheduled::` that is present, each via `next_after`
  against its own value (offsets are preserved naturally).
- **Skip** — advances all present date fields via `next_after`, leaves
  `status::` at `todo`, does **not** stamp `last_completed::`, but **does**
  increment `recurrence_done` (a skip consumes a `count` slot and respects
  `until`, exactly like a completion).

## 4. Skip — surfaces

One backend action, two front-ends.

- **Backend**: extend the existing `POST /api/blocks/recur-bump` with a
  `mode: "complete" | "skip"` field (default `complete` preserves current
  callers). Skip runs the engine's skip path above.
- **Verb**: register a `skip` command in the v5/v4 command registry so it
  appears in Cmd+K, the `:` colon line, and the leader menu; it acts on the
  focused task block. No-op (with a toast) if the focused block has no
  `recurring::`.
- **Chip menu**: the `recurring::` display chip gains a click menu with
  "Skip to next occurrence" calling the same endpoint.

## 5. Apple Reminders sync

`crates/tesela-server/src/reminders/darwin.rs`.

- **Push** — extend `build_recurrence_rule`: `by_weekday` →
  `EKRecurrenceDayOfWeek` array (generalize the existing `weekdays_rule`
  helper, which already builds that array for Mon–Fri); `end` →
  `EKRecurrenceEnd` (`endWithEndDate:` for `Until`, `endWithOccurrenceCount:`
  for `Count`).
- **Pull** — map `EKRecurrenceRule` back: `daysOfTheWeek` →
  `by_weekday`, `recurrenceEnd` → `RecurrenceEnd`. The diff that gates
  whether a pull overwrites Tesela compares parsed `Recurrence` values
  (already the pattern — `every 1 week` vs `weekly` must not flap).
- **Skip is Tesela-side only** — EK has no "skip one occurrence" concept. A
  skip simply writes a bumped date; that propagates as an ordinary date
  change on the next sync.

## 6. UI

- **Web** — `DatePicker.svelte` repeat sub-row gains: a custom day-set
  picker (Mon–Sun toggles) and an end-condition control (never / on date /
  after N occurrences). `parseRecurrenceInput` in `date-parser.ts` mirrors
  the extended Rust grammar. The `recurring::` chip (rendered via the
  display-chips system) gains the "Skip to next" menu item.
- **iOS** — the recurrence display chip + parser learn the new vocabulary so
  imported / synced recurring tasks render correctly. iOS recurrence
  *editing* stays minimal for v1 (display + skip only); full iOS recurrence
  editing is out of scope here.

## 7. Testing

- `recurrence.rs` unit tests: every new parse form (BYDAY, `weekends`,
  `until`, `count`, combinations), `next_after` with `by_weekday`,
  series-end arithmetic for both `until` and `count`, skip increments
  `recurrence_done`.
- `darwin.rs`: the existing EK round-trip diff test stays green; add cases
  for a BYDAY rule and an end-condition rule.
- Web: `date-parser` unit tests for the mirrored grammar.

## Out of scope

- iOS recurrence *editing* UI (display + skip only).
- `BYMONTHDAY` / `BYSETPOS` (e.g. "last Friday of the month").
- Multiple `until`+`count` on one rule (rrule allows one end condition).
- Project rollups, quick-add subtask, cross-note deps, configurable
  notification lead time — the *Task ergonomics* sub-project, a separate
  spec after this ships.
