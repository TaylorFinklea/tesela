# First-Class Dates (iOS) — design

*2026-05-22. Brainstormed + approved. iOS-only port of the web's First-Class Dates work; the iOS agenda view is a later, separate effort.*

## Problem

The Tesela iOS app surfaces no UI for dates on task blocks. The keyboard toolbar carries `.deadline` and `.schedule` enum cases as **stubs** (`Sources/Data/KeyboardToolbarItem.swift:14,16`; handler stubs in `Sources/Components/BlockRow.swift:280-284`), and `deadline::` / `scheduled::` block properties are parsed and round-tripped but never displayed (`Sources/Components/BlockRow.swift:62-92` shows only `RecurrenceChip`). Users can't set or see dates on tasks from the iOS client.

The web shipped First-Class Dates earlier today (`.docs/ai/phases/2026-05-22-first-class-dates-design.md`): a date is a bare `YYYY-MM-DD` `deadline::` / `scheduled::` block property, set via an NL command, shown in a properties row, with a configurable bare-date default field. This design brings iOS to the same model with touch-first UI.

## Decisions (from brainstorming)

- **No data-model migration.** iOS already round-trips `key:: value` block properties — `Block.properties: [BlockProperty]` in `Sources/Data/Models.swift:18-37` parses and re-emits properties unchanged. The on-disk shape ships unchanged; this redesign is purely additive UI.
- **NL field + Swift parser.** A new `DateParser.swift` ports the web's `date-parser.ts` faithfully (date phrases, times, trailing recurrence, field keyword). Touch-first iOS still benefits from typing "next fri" rather than scrolling a wheel picker for everything, and the Swift parser keeps the iOS path offline-capable.
- **Single "Date" entry point.** The two stub toolbar items (`.deadline`, `.schedule`) consolidate into one `.date` item that opens a sheet containing both an NL field and a native picker. The field (deadline vs scheduled) is resolved by the NL keyword or falls back to the `bareDateField` setting — same mental model as web's `/date` command.
- **Inline chips, not a properties row.** Date displays render alongside the existing `TagChip` / `RecurrenceChip` in `BlockRow`'s chip `HStack`, not on a separate row beneath the block. iOS already has the inline-chip pattern; mirroring web's separate row would burn vertical space against the iOS-native convention.
- **Settings parity.** A `bareDateField` `@AppStorage` setting mirrors the web's preference; same `"deadline" | "scheduled"` values, same default of `"scheduled"`.

## 1. Data model — already correct

A date on a block is a bare-ISO `date`-typed property: `scheduled:: 2026-05-25` / `deadline:: 2026-05-25` / `recurring:: weekly`. The existing `parseBlocks` (`Sources/Data/MockMosaicService.swift:996-1009`) collects properties into `block.properties` and `renderProperties` (`:1219-1237`) emits them back. No parser changes; no format changes; no migration.

The two stubbed `KeyboardToolbarItem` cases (`.deadline`, `.schedule`) are removed; a new `.date` case takes their place — single icon, single button, single sheet.

## 2. The Swift NL parser

Create `app/Tesela-iOS/Sources/Data/DateParser.swift` — a Swift port of `web/src/lib/date-parser.ts`:

- Public API: `static func parse(_ input: String, today: Date = Date()) -> ParsedDateTimeRecurrence?` returning the same shape as the TS type (`date: String /*YYYY-MM-DD*/`, `time: String? /*HH:mm*/`, `recurrence: String?`, `field: DateField?`).
- Grammar (mirror exactly): `today`/`tomorrow`/`yesterday`, weekday names, `next fri`, `may 23`, `2026-05-23`, `in 3 days`, `3d`, time suffixes (`fri at 10am`, `tom 14:30`), trailing recurrence (`every mon, fri`, `weekdays`, `every 3 days`, `every 2 weeks count 10`, `daily until 2026-12-31`), and a leading `deadline`/`scheduled`/`due` keyword.
- Tests: `app/Tesela-iOS/Tests/DateParserTests.swift` — `XCTest` cases mirroring `web/tests/unit/date-parser.test.mjs` line-for-line so the two parsers stay in lockstep. Any regression on the web side that ships a new grammar surface needs a paired Swift test.

The bare-recurrence anchor case the web final-review caught (`every monday` alone → recurrence + today anchor, field null) must be in the Swift port too.

## 3. The Date input sheet

A `DateInputSheet.swift` presented from `BlockRow` when the user taps the `.date` keyboard-toolbar button (while a block is in edit mode). Components:

- **NL `TextField`** (primary, autofocused) bound to a `@State var input: String`. As the user types, `DateParser.parse(input)` runs and the parsed values flow into the picker + the field indicator below.
- **Native `DatePicker(.graphical)`** (secondary). Bound to the parsed date — typing updates the calendar; tapping a calendar date updates the input back to its canonical ISO form. A small `DatePicker(.hourAndMinute)` strip exposes the time when the input carries one.
- **Recurrence row** — preset chips (`Daily` / `Weekdays` / `Weekly` / `Monthly` / `Yearly` / custom) plus an end-condition segmented control (`Never` / `Until` / `After N`). Same shape as the web `DatePicker`'s recurrence sub-row. Typing recurrence into the NL field updates these controls; tapping a chip updates the NL field back. One source of truth is the parsed result; both UIs are bindings.
- **Field indicator** — a small label `"Scheduled"` / `"Deadline"` showing where the value will land. Flips when the parsed `field` is non-nil (NL keyword wins); otherwise shows the `bareDateField` default.
- **Skip control** — visible only when the block being edited *already carries* a `recurring::` property (not just when the NL field happens to type a new recurrence): a "Skip to next occurrence" button that calls the existing `POST /blocks/recur-bump` with `mode: skip` (the same endpoint the web's `:skip` verb hits).
- **Cancel / Set** buttons. On `Set`: upsert the parsed property (and `recurring::` if present) onto the block — see §5.

## 4. Display — inline chips on `BlockRow`

`Sources/Components/BlockRow.swift`'s existing chip `HStack` (around `:67-76`) renders tags and the `RecurrenceChip`. Add two new sibling chip views:

- `DeadlineChip` — given `block.properties` containing a `deadline::` value, render a chip with an `exclamationmark.circle` (or `flag.fill`) SF Symbol + the human date label (e.g. `May 25`) via `DateFormat.humanMonthDay(_:)` — a new shared formatter that ports `web/src/lib/date-format.ts`'s `formatDateMonthDay` (accepts bare ISO or legacy `[[..]]`, returns `"May 25"` for current year, `"May 25, 2025"` otherwise, with optional ` 3:30p` time suffix).
- `ScheduledChip` — same shape with a `calendar` SF Symbol.

Both chips are buttons: tapping re-opens the `DateInputSheet` pre-filled for that block's value, scoped so commit writes to the same property key.

`RecurrenceChip` (`Sources/Components/TagChip.swift:50-68`) stays display-only for v1 — skip lives in the Date sheet (§3). v1.1 can add a tap-to-skip menu directly on the chip if you want it on the row.

## 5. Save path — existing `PUT /notes/{id}`

When the Date sheet commits, mutate `block.properties` (upsert the `scheduled`/`deadline`/`recurring` entries) and call `editTodayBlock(id:text:)` or `editPageBlock(pageId:blockId:text:)` (`MockMosaicService.swift:167-173, 236-241`), which renders the full block back through `renderBody` and pushes via `PUT /notes/{id}` — exactly how every other iOS edit persists today. **No new endpoint dependency.**

The new `POST /blocks/set-property` endpoint the web's Agenda work added is available, and a later optimization could switch iOS over to it for atomic property writes, but v1 keeps the simpler existing path.

The engine's `apply_post_save_bumps_with_info` runs on `PUT /notes/{id}` saves already (it's on the canonical update path), so marking a recurring task done from iOS still bumps correctly.

## 6. Settings — `bareDateField`

Add to `Sources/Data/BackendSettings.swift`:

```swift
@AppStorage("bareDateField") var bareDateField: String = "scheduled"
```

Add a `Picker("Default date field")` to `Sources/Views/Settings/SettingsView.swift` in the Capture section (`:112-122`):

```swift
Picker("Default date field", selection: $backendSettings.bareDateField) {
    Text("Scheduled").tag("scheduled")
    Text("Deadline").tag("deadline")
}
```

Same semantics as web: a date typed without a `deadline`/`scheduled`/`due` keyword routes to whichever field this setting names.

## 7. Out of scope

- **Capture-bar NL date input.** `CaptureBar`'s composer accepts text only today (`Sources/Components/CaptureBar.swift:327-341`). Adding date input to capture is a separate v1.1 — current scope is editing existing blocks via the toolbar.
- **Drag-to-reschedule.** Not on iOS in v1 (also out of scope on web).
- **Per-occurrence overrides on recurring tasks.** Same out-of-scope as web.
- **The agenda view on iOS.** The companion "what's due/scheduled" surface is a separate iOS effort.
- **Switching to `POST /blocks/set-property` for atomic writes.** Optimization for later; the existing `PUT /notes/{id}` save path works for v1.
- **Bulk migration of any pre-existing data.** Not needed — the property model is already what iOS reads and writes.
