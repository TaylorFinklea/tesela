# Recurrence Completeness (Clients) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bring the web client and iOS app up to the recurrence grammar the Rust engine already supports — BYDAY day-sets, `weekends`, `until`/`count` end conditions — and add a skip-occurrence action.

**Architecture:** The Rust engine (`tesela_core::recurrence`, shipped) is the authority; clients only need to *produce* and *display* the `recurring::` text grammar it parses. Web: extend the `parseRecurrenceInput` recognizer, the `DatePicker` repeat sub-row, the `recurring::` display chip, add a `skip` verb. iOS: a display-only recurrence formatter. No client re-implements recurrence math — they round-trip strings.

**Tech Stack:** SvelteKit 2 / Svelte 5 runes, TypeScript, Vitest (`pnpm test:unit`); Swift / SwiftUI.

**Scope:** Clients only. The engine half shipped 2026-05-22 (`fdc9e17`..`919d8b7`).

**Reference spec:** `.docs/ai/phases/2026-05-21-recurrence-completeness-design.md` (section 6 = UI).

---

## File Structure

- `web/src/lib/date-parser.ts` — **modify.** Extend `parseRecurrenceInput` + `TRAILING_RECUR_RE` to recognize the full grammar.
- `web/src/lib/date-parser.test.mjs` — **create or modify.** Unit tests for the extended recognizer.
- `web/src/lib/api-client.ts` — **modify.** `recurBump` gains an optional `mode`.
- `web/src/lib/v4/commands.ts` — **modify.** New `skip` verb.
- `web/src/lib/components/DatePicker.svelte` — **modify.** Repeat sub-row: `weekends` preset, day-of-week toggle row, end-condition control.
- `web/src/lib/recurrence-format.ts` — **create.** Pure `formatRecurrence(value: string): string` — turns a `recurring::` string into human text. Shared by the chip and (ported) iOS.
- `web/src/lib/recurrence-format.test.mjs` — **create.** Tests for the formatter.
- `web/src/lib/components/BlockOutliner.svelte` (and/or the recurring-chip render site) — **modify.** Render the recurring chip via `formatRecurrence`, add a skip click-menu.
- `app/Tesela-iOS/Sources/Data/RecurrenceFormat.swift` — **create.** Swift port of `formatRecurrence`.
- `app/Tesela-iOS/Sources/Components/BlockRow.swift` — **modify.** Render a recurrence chip from `properties["recurring"]`.

---

## Task 1: Extend `parseRecurrenceInput` to the full grammar

**Files:**
- Modify: `web/src/lib/date-parser.ts`
- Test: `web/src/lib/date-parser.test.mjs` (create if absent)

`parseRecurrenceInput(input: string): string | null` returns a canonical `recurring::` string or `null`. It currently recognizes `daily`, `weekly`, `monthly`, `yearly`, `weekdays`, `every N days|weeks|months`. Extend it to mirror the Rust `recurrence::parse` grammar: `weekends`, BYDAY (`every mon, wed, fri`), and trailing ` until YYYY-MM-DD` / ` count N`.

- [ ] **Step 1: Write the failing tests**

First read `web/src/lib/date-parser.ts` to see the exact current `parseRecurrenceInput` body and whether `web/src/lib/date-parser.test.mjs` exists (if it does, append; if not, create it following the pattern of `web/src/lib/cm-decorations.test.mjs` — `node:test` + `node:assert`, importing from the `.ts` via the project's test runner). Add:

```javascript
import { test } from "node:test";
import assert from "node:assert/strict";
import { parseRecurrenceInput } from "./date-parser.ts";

test("parseRecurrenceInput — existing forms still parse", () => {
  assert.equal(parseRecurrenceInput("daily"), "daily");
  assert.equal(parseRecurrenceInput("every 2 weeks"), "every 2 weeks");
  assert.equal(parseRecurrenceInput("weekdays"), "weekdays");
  assert.equal(parseRecurrenceInput("garbage"), null);
});

test("parseRecurrenceInput — weekends", () => {
  assert.equal(parseRecurrenceInput("weekends"), "weekends");
});

test("parseRecurrenceInput — BYDAY day-sets", () => {
  assert.equal(parseRecurrenceInput("every mon, wed, fri"), "every mon, wed, fri");
  assert.equal(parseRecurrenceInput("every monday"), "every mon");
  // normalized Mon-first
  assert.equal(parseRecurrenceInput("every fri, mon"), "every mon, fri");
  assert.equal(parseRecurrenceInput("every mon, blarg"), null);
});

test("parseRecurrenceInput — until / count end clauses", () => {
  assert.equal(parseRecurrenceInput("weekly until 2026-12-31"), "weekly until 2026-12-31");
  assert.equal(parseRecurrenceInput("every mon, fri count 12"), "every mon, fri count 12");
  assert.equal(parseRecurrenceInput("daily count 0"), null);
  assert.equal(parseRecurrenceInput("daily until not-a-date"), null);
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd web && pnpm test:unit date-parser` (or `npx vitest run src/lib/date-parser.test.mjs` if the runner differs — check `web/package.json` `scripts.test:unit`).
Expected: the weekends/BYDAY/until-count tests FAIL.

- [ ] **Step 3: Implement the extended recognizer**

In `date-parser.ts`, rewrite `parseRecurrenceInput` to mirror the Rust parser (`crates/tesela-core/src/recurrence.rs` `parse` — read it for the canonical behavior). Structure:

```typescript
const WEEKDAY_TOKENS: Record<string, string> = {
  mon: "mon", monday: "mon",
  tue: "tue", tues: "tue", tuesday: "tue",
  wed: "wed", wednesday: "wed",
  thu: "thu", thur: "thu", thurs: "thu", thursday: "thu",
  fri: "fri", friday: "fri",
  sat: "sat", saturday: "sat",
  sun: "sun", sunday: "sun",
};
const WEEKDAY_ORDER = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"];

/** Parse a `recurring::` value into its canonical form, or null if unrecognized.
 *  Mirrors `tesela_core::recurrence::parse`. */
export function parseRecurrenceInput(input: string): string | null {
  const s = input.trim().toLowerCase().replace(/\s+/g, " ");
  if (!s) return null;

  // Split a trailing end clause: " until YYYY-MM-DD" or " count N".
  let base = s;
  let endClause = "";
  const untilIdx = s.lastIndexOf(" until ");
  const countIdx = s.lastIndexOf(" count ");
  if (untilIdx !== -1) {
    const dateStr = s.slice(untilIdx + 7).trim();
    if (!/^\d{4}-\d{2}-\d{2}$/.test(dateStr) || Number.isNaN(Date.parse(dateStr))) return null;
    base = s.slice(0, untilIdx);
    endClause = ` until ${dateStr}`;
  } else if (countIdx !== -1) {
    const n = Number(s.slice(countIdx + 7).trim());
    if (!Number.isInteger(n) || n < 1) return null;
    base = s.slice(0, countIdx);
    endClause = ` count ${n}`;
  }

  const freq = parseRecurrenceFreq(base);
  return freq === null ? null : freq + endClause;
}

/** The frequency/BYDAY portion only (no end clause). */
function parseRecurrenceFreq(base: string): string | null {
  switch (base) {
    case "daily": case "every day": return "daily";
    case "weekly": case "every week": return "weekly";
    case "monthly": case "every month": return "monthly";
    case "yearly": case "annually": case "every year": return "yearly";
    case "weekdays": return "weekdays";
    case "weekends": return "weekends";
  }
  if (base.startsWith("every ")) {
    const rest = base.slice(6);
    // BYDAY: every comma-token is a weekday.
    const tokens = rest.split(",").map((t) => t.trim());
    if (rest && tokens.every((t) => WEEKDAY_TOKENS[t] !== undefined)) {
      const days = [...new Set(tokens.map((t) => WEEKDAY_TOKENS[t]))]
        .sort((a, b) => WEEKDAY_ORDER.indexOf(a) - WEEKDAY_ORDER.indexOf(b));
      return `every ${days.join(", ")}`;
    }
    // every N <unit>
    const m = rest.match(/^(\d+) (day|days|week|weeks|month|months|year|years)$/);
    if (m) {
      const n = Number(m[1]);
      if (n < 1) return null;
      const unit = m[2];
      if (unit === "day" || unit === "days") return n === 1 ? "daily" : `every ${n} days`;
      if (unit === "week" || unit === "weeks") return n === 1 ? "weekly" : `every ${n} weeks`;
      if (unit === "month" || unit === "months") return n === 1 ? "monthly" : `every ${n} months`;
      return n === 1 ? "yearly" : `every ${n} years`;
    }
  }
  return null;
}
```

Adjust to match the file's existing export/format conventions (keep whatever the current function's exact return-normalization was for the pre-existing forms — verify `every 1 week` etc. against the old behavior so nothing regresses).

- [ ] **Step 4: Extend `TRAILING_RECUR_RE`**

`extractRecurrence` (around `date-parser.ts:108`) uses `TRAILING_RECUR_RE` to strip a recurrence phrase off the end of a natural-language date input. Read its current pattern and widen it so it also matches `weekends`, `every mon, wed, fri`, and an optional ` until <date>` / ` count N` suffix. Add a test in `date-parser.test.mjs`:

```javascript
import { parseDateAndRecurrenceInput } from "./date-parser.ts";
test("parseDateAndRecurrenceInput extracts an extended recurrence tail", () => {
  const r = parseDateAndRecurrenceInput("friday every mon, wed, fri count 8");
  assert.equal(r.recurrence, "every mon, wed, fri count 8");
});
```

- [ ] **Step 5: Run tests**

Run: `cd web && pnpm test:unit date-parser`
Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add web/src/lib/date-parser.ts web/src/lib/date-parser.test.mjs
git commit -m "feat(web): parse BYDAY/weekends/until/count in recurrence input"
```

---

## Task 2: `recurBump` API call gains a `mode`

**Files:**
- Modify: `web/src/lib/api-client.ts:150-156`

- [ ] **Step 1: Make the change**

The current `recurBump` is:
```typescript
recurBump: (blockId: string) =>
  post<{ bumped: boolean; next_deadline: string | null }>(
    "/blocks/recur-bump",
    { block_id: blockId },
  ),
```
Replace with:
```typescript
recurBump: (blockId: string, mode: "complete" | "skip" = "complete") =>
  post<{ bumped: boolean; next_deadline: string | null }>(
    "/blocks/recur-bump",
    { block_id: blockId, mode },
  ),
```
The server's `RecurBumpReq` already accepts an optional `mode` (defaults to `complete`), so existing zero-arg callers are unaffected.

- [ ] **Step 2: Verify it compiles**

Run: `cd web && pnpm check 2>&1 | grep -c "api-client.ts"`
Expected: `0` (no new type errors in this file; the pre-existing `VoiceCaptureButton.svelte` error is unrelated).

- [ ] **Step 3: Commit**

```bash
git add web/src/lib/api-client.ts
git commit -m "feat(web): recurBump accepts a complete|skip mode"
```

---

## Task 3: `skip` verb in the command registry

**Files:**
- Modify: `web/src/lib/v4/commands.ts`

- [ ] **Step 1: Read the registry**

Read `web/src/lib/v4/commands.ts` fully. Note: the `V4Command` shape (`id, verb?, label, glyph, category, shortcut?, keywords, argPrompt?, run`), how existing commands obtain context (focused block / focused leaf), and how `buildV4Commands()` assembles the list. Also read `web/src/lib/stores/current-block.svelte.ts` for `getFocusedBlock()` and what a focused block exposes (it has `.id` and `.properties` per `ParsedBlock`).

- [ ] **Step 2: Add the `skip` command**

Inside `buildV4Commands()`, add a command to the returned array:

```typescript
{
  id: "skip-occurrence",
  verb: "skip",
  label: "Skip to Next Occurrence",
  glyph: "⏭",
  category: "tile",
  keywords: ["skip", "recurrence", "recurring", "next", "occurrence"],
  run: async () => {
    const block = getFocusedBlock();
    if (!block || !block.properties?.recurring) {
      showToast("No recurring task focused", "warn");
      return;
    }
    const res = await api.recurBump(block.id, "skip");
    if (res.bumped) {
      queryClient.invalidateQueries({ queryKey: ["notes"] });
      showToast("Skipped to next occurrence", "success");
    } else {
      showToast("Nothing to skip", "info");
    }
  },
},
```

Use the imports/helpers the file already has — `getFocusedBlock` from `$lib/stores/current-block.svelte`, `api` from `$lib/api-client`, the app query client from `$lib/app-query-client.svelte` (`getAppQueryClient()`), and the toast helper from `$lib/stores/toast.svelte`. If `commands.ts` doesn't already import these, add the imports; match how the *modern* `web/src/lib/commands.ts` builder accesses the query client and toasts (read it for the exact helper names) so this stays consistent. If a command `run` in this file has no precedent for async + query invalidation, keep the `run` body minimal and correct rather than inventing patterns.

- [ ] **Step 3: Verify**

Run: `cd web && pnpm check 2>&1 | grep -c "commands.ts"`
Expected: `0`.
Run: `cd web && pnpm test:unit` — expected: no regressions.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/v4/commands.ts
git commit -m "feat(web): skip verb — skip a recurring task's occurrence"
```

---

## Task 4: DatePicker repeat sub-row — day-sets + end conditions

**Files:**
- Modify: `web/src/lib/components/DatePicker.svelte`

The repeat sub-row (`DatePicker.svelte:313-362`) has preset buttons (`daily/weekly/monthly/yearly/weekdays`) + a custom-text toggle. State: `selectedRecurrence: string | null`, `PRESETS` array, `customRecurrenceOpen`, `customRecurrenceInput`. Add `weekends`, a day-of-week toggle row, and an end-condition control.

- [ ] **Step 1: Read the component**

Read `DatePicker.svelte` fully — the repeat-row markup, the `$state` declarations, and how `selectedRecurrence` reaches `onPick(iso, time, recurrence)`.

- [ ] **Step 2: Add `weekends` to `PRESETS`**

Change the `PRESETS` array (line ~55) to `["daily", "weekly", "monthly", "yearly", "weekdays", "weekends"]`. The existing preset-button `{#each}` renders it with no other change.

- [ ] **Step 3: Add the day-of-week toggle row**

Add component state:
```typescript
const WEEKDAYS = [
  { key: "mon", label: "M" }, { key: "tue", label: "T" }, { key: "wed", label: "W" },
  { key: "thu", label: "T" }, { key: "fri", label: "F" }, { key: "sat", label: "S" },
  { key: "sun", label: "S" },
];
let pickedDays = $state<Set<string>>(new Set());
```
Render a toggle row below the preset buttons: a button per `WEEKDAYS` entry, `class:active={pickedDays.has(d.key)}`. Clicking toggles the key in `pickedDays` (reassign a new `Set` for reactivity) and then sets `selectedRecurrence` to the BYDAY string `every <days joined Mon-first>` (or, if `pickedDays` becomes empty, leaves `selectedRecurrence` to whatever a preset last set). When a preset button is clicked, clear `pickedDays`. On mount, if `initialRecurrence` is a BYDAY string (`/^every (mon|tue|wed|thu|fri|sat|sun)(,|$)/`), pre-fill `pickedDays` from it.

- [ ] **Step 4: Add the end-condition control**

Add state:
```typescript
let endMode = $state<"never" | "until" | "count">("never");
let endUntil = $state(""); // YYYY-MM-DD
let endCount = $state(1);
```
Render a small 3-segment control (Never / Until / After) below the day row. "Until" reveals a `<input type="date">` bound to `endUntil`; "After" reveals a `<input type="number" min="1">` bound to `endCount` with an "× occurrences" label. A `$derived` `endClause` produces `""` / ` until <endUntil>` / ` count <endCount>`. The value committed via `onPick` is `selectedRecurrence ? selectedRecurrence + endClause : null` — i.e. the end clause only attaches when a frequency is chosen. On mount, parse any end clause out of `initialRecurrence` to pre-fill `endMode`/`endUntil`/`endCount`.

- [ ] **Step 5: Keep the custom-text escape hatch consistent**

The existing custom-text input runs `parseRecurrenceInput`. Leave it — it now accepts the full grammar (Task 1). When the custom input yields a value, clear `pickedDays` and reset `endMode` to `never` (the custom string carries its own end clause). No other change.

- [ ] **Step 6: Verify**

Run: `cd web && pnpm check 2>&1 | grep -c "DatePicker.svelte"` → `0`.
Run: `cd web && pnpm build` → succeeds.
Manually reason through: picking Mon/Wed/Fri + "After 12" must make `onPick` receive `"every mon, wed, fri count 12"`, which `parseRecurrenceInput` round-trips and the Rust engine parses.

- [ ] **Step 7: Commit**

```bash
git add web/src/lib/components/DatePicker.svelte
git commit -m "feat(web): DatePicker day-set picker + end-condition control"
```

---

## Task 5: `recurring::` chip — human formatting + skip menu

**Files:**
- Create: `web/src/lib/recurrence-format.ts`
- Create: `web/src/lib/recurrence-format.test.mjs`
- Modify: the recurring-chip render site (find it: `cd web && rg -n "recurring" src/lib/components`)

- [ ] **Step 1: Write the failing formatter tests**

Create `web/src/lib/recurrence-format.test.mjs`:
```javascript
import { test } from "node:test";
import assert from "node:assert/strict";
import { formatRecurrence } from "./recurrence-format.ts";

test("formatRecurrence — simple + every-N", () => {
  assert.equal(formatRecurrence("daily"), "Daily");
  assert.equal(formatRecurrence("every 2 weeks"), "Every 2 weeks");
  assert.equal(formatRecurrence("weekdays"), "Weekdays");
  assert.equal(formatRecurrence("weekends"), "Weekends");
});
test("formatRecurrence — BYDAY", () => {
  assert.equal(formatRecurrence("every mon, wed, fri"), "Mon, Wed, Fri");
});
test("formatRecurrence — end clauses", () => {
  assert.equal(formatRecurrence("weekly until 2026-12-31"), "Weekly until Dec 31, 2026");
  assert.equal(formatRecurrence("daily count 10"), "Daily, 10×");
});
test("formatRecurrence — unrecognized passes through", () => {
  assert.equal(formatRecurrence("blarg"), "blarg");
});
```

- [ ] **Step 2: Run to verify failure**

Run: `cd web && pnpm test:unit recurrence-format` → FAIL (module missing).

- [ ] **Step 3: Implement `formatRecurrence`**

Create `web/src/lib/recurrence-format.ts`. `formatRecurrence(value: string): string` — split the end clause, format the frequency portion, append a human end clause:

```typescript
const DAY_LABEL: Record<string, string> = {
  mon: "Mon", tue: "Tue", wed: "Wed", thu: "Thu", fri: "Fri", sat: "Sat", sun: "Sun",
};

/** Human-readable rendering of a `recurring::` value. Unrecognized input
 *  is returned unchanged (never throws). */
export function formatRecurrence(value: string): string {
  const s = value.trim().toLowerCase().replace(/\s+/g, " ");
  if (!s) return value;

  let base = s;
  let endText = "";
  const untilIdx = s.lastIndexOf(" until ");
  const countIdx = s.lastIndexOf(" count ");
  if (untilIdx !== -1) {
    base = s.slice(0, untilIdx);
    const date = new Date(s.slice(untilIdx + 7).trim() + "T00:00:00");
    endText = Number.isNaN(date.getTime())
      ? ""
      : ` until ${date.toLocaleDateString(undefined, { month: "short", day: "numeric", year: "numeric" })}`;
  } else if (countIdx !== -1) {
    base = s.slice(0, countIdx);
    endText = `, ${s.slice(countIdx + 7).trim()}×`;
  }

  const freq = formatFreq(base);
  return freq === null ? value : freq + endText;
}

function formatFreq(base: string): string | null {
  switch (base) {
    case "daily": return "Daily";
    case "weekly": return "Weekly";
    case "monthly": return "Monthly";
    case "yearly": return "Yearly";
    case "weekdays": return "Weekdays";
    case "weekends": return "Weekends";
  }
  if (base.startsWith("every ")) {
    const rest = base.slice(6);
    const tokens = rest.split(",").map((t) => t.trim());
    if (rest && tokens.every((t) => DAY_LABEL[t] !== undefined)) {
      return tokens.map((t) => DAY_LABEL[t]).join(", ");
    }
    const m = rest.match(/^(\d+) (days?|weeks?|months?|years?)$/);
    if (m) return `Every ${m[1]} ${m[2]}`;
  }
  return null;
}
```

- [ ] **Step 4: Run tests**

Run: `cd web && pnpm test:unit recurrence-format` → PASS.

- [ ] **Step 5: Render the chip via `formatRecurrence` + add a skip menu**

Locate where a block's `recurring::` property renders as a chip (Step-0 `rg`). Route its displayed text through `formatRecurrence(value)`. Add a click affordance: clicking the recurring chip opens a one-item popover/menu "Skip to next occurrence" that calls `api.recurBump(blockId, "skip")` then invalidates `["notes"]` and toasts — reuse the exact pattern from the Task 3 `skip` verb (DRY: if the verb's `run` body is non-trivial, extract a shared `skipRecurrence(blockId)` helper into `recurrence-format.ts`'s sibling or a small `$lib/recurrence-actions.ts` and call it from both the verb and the chip). Follow the existing chip component's interaction patterns (how other chips, if any, handle clicks) — if chips are currently render-only and adding a popover is substantial, keep the menu minimal (a small absolutely-positioned div) and report it as DONE_WITH_CONCERNS noting the chip-interaction system is thin.

- [ ] **Step 6: Verify**

Run: `cd web && pnpm check` (no new errors in touched files), `pnpm build` (succeeds), `pnpm test:unit` (no regressions).

- [ ] **Step 7: Commit**

```bash
git add web/src/lib/recurrence-format.ts web/src/lib/recurrence-format.test.mjs web/src/lib/components/
git commit -m "feat(web): human-format the recurring chip + skip-occurrence menu"
```

---

## Task 6: iOS recurrence display

**Files:**
- Create: `app/Tesela-iOS/Sources/Data/RecurrenceFormat.swift`
- Modify: `app/Tesela-iOS/Sources/Components/BlockRow.swift`

iOS treats block `properties` as an opaque `[BlockProperty]` (`key`/`value`). This task adds a display-only formatter and renders a recurrence chip — no editing, no skip on iOS in this pass.

- [ ] **Step 1: Create the formatter (with tests)**

Create `app/Tesela-iOS/Sources/Data/RecurrenceFormat.swift` — a Swift port of `formatRecurrence` (Task 5). `enum RecurrenceFormat { static func human(_ value: String) -> String }`: split a trailing ` until YYYY-MM-DD` / ` count N`, format the frequency (`daily`→"Daily", `weekdays`→"Weekdays", `weekends`→"Weekends", `every mon, wed, fri`→"Mon, Wed, Fri", `every N units`→"Every N units"), append ` until <Mon D, YYYY>` / `, N×`; unrecognized input returns unchanged. If the iOS project has a test target (check `app/Tesela-iOS/project.yml` / for an existing `*Tests` group), add a Swift Testing or XCTest case mirroring the Task 5 test cases; if there is no test target, skip the test and note it in the report.

- [ ] **Step 2: Render the chip in `BlockRow.swift`**

`BlockRow.swift` renders trailing tags as `TagChip`s (lines ~61-67). Add, alongside the tags `HStack`, a recurrence chip: if `properties` contains a `recurring` key, render a small chip (reuse `TagChip`'s visual style or a sibling lightweight chip view) showing a repeat glyph (SF Symbol `arrow.triangle.2.circlepath` or `repeat`) + `RecurrenceFormat.human(value)`. Display-only — no tap action. Match the existing chip sizing/theming (`theme.fgMuted` etc.).

- [ ] **Step 3: Build**

Run: `xcodebuild -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'generic/platform=iOS Simulator' build 2>&1 | grep -E "BUILD (SUCCEEDED|FAILED)"`
Expected: `BUILD SUCCEEDED`.

- [ ] **Step 4: Commit**

```bash
git add app/Tesela-iOS/Sources/Data/RecurrenceFormat.swift app/Tesela-iOS/Sources/Components/BlockRow.swift
git commit -m "feat(ios): display-only recurrence chip on task blocks"
```

---

## Self-Review

**Spec coverage (design spec §6):** `parseRecurrenceInput` mirrors the grammar — Task 1. DatePicker day-set picker + end-condition control — Task 4 (`weekends` preset, day toggles, end control). `recurring::` chip + skip menu — Task 5. `skip` verb — Task 3 (uses Task 2's `mode`). iOS recurrence display — Task 6. All §6 items covered.

**Type consistency:** `parseRecurrenceInput` (Task 1) and `formatRecurrence` (Task 5) both split the end clause with the same ` until `/` count ` convention the Rust engine emits. `recurBump(blockId, mode)` (Task 2) is the signature Task 3 and Task 5 both call. The canonical strings Task 1 produces are exactly what Task 5 formats and the Rust engine parses.

**Placeholder scan:** Task 3 and Task 5 say "read the file / match existing patterns" for the parts that depend on undocumented local conventions (the command registry's context access, the chip component's interaction model) rather than inventing signatures — intentional, the implementer confirms against real code. All code-bearing steps carry real code.

**Ordering:** Task 2 (api `mode`) precedes Tasks 3 and 5 (both call it). Task 5's chip skip reuses Task 3's skip logic — the plan calls for extracting a shared `skipRecurrence` helper if non-trivial.
