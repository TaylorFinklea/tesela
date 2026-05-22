# First-Class Dates (web) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make dates on task blocks first-class typed properties (`deadline::` / `scheduled::`, bare ISO scalar) set by one NL command and shown in an editable properties row — replacing the inline `[[YYYY-MM-DD]]` wiki-link.

**Architecture:** A date is a `date`-typed block property, not inline text. One command (the reworked `/date` slash entry) opens an NL input whose parse yields date + recurrence + which field; commit upserts `scheduled::`/`deadline::`/`recurring::` properties via `upsertBlockProperty`. A new `BlockDateRow` component renders those properties as a labelled, click-to-edit strip under the block. A configurable setting picks the field for a keyword-less date.

**Tech Stack:** SvelteKit 2 / Svelte 5 runes, CodeMirror 6, TypeScript, Vitest (`pnpm test:unit`).

**Reference spec:** `.docs/ai/phases/2026-05-22-first-class-dates-design.md`

---

## File Structure

- `web/src/lib/date-parser.ts` — **modify.** `ParsedDateTimeRecurrence` gains `field`; `parseDateAndRecurrenceInput` recognizes a leading `deadline`/`scheduled`/`due` keyword.
- `web/tests/unit/date-parser.test.mjs` — **modify.** Tests for the field keyword.
- `web/src/lib/preferences.svelte.ts` — **modify.** New `bareDateField` preference.
- `web/src/routes/settings/general/+page.svelte` — **modify.** A control for it.
- `web/src/lib/date-format.ts` — **create.** Pure `formatDateMonthDay` extracted from `DisplayChip.svelte`.
- `web/src/lib/date-format.test.mjs` — **create.** Tests for it.
- `web/src/lib/components/DisplayChip.svelte` — **modify.** Import the extracted formatter.
- `web/src/lib/components/DatePicker.svelte` — **modify.** `onPick` emits the parsed `field`.
- `web/src/lib/components/BlockEditor.svelte` — **modify.** The `/date` picker commit writes properties (not an inline link); the inline-insert branch is removed.
- `web/src/lib/components/BlockDateRow.svelte` — **create.** The properties row component.
- `web/src/lib/components/BlockOutliner.svelte` — **modify.** Mount `BlockDateRow`; wire its property writes to the block save path.

---

## Task 1: NL grammar — `deadline`/`scheduled` field keyword

**Files:**
- Modify: `web/src/lib/date-parser.ts`
- Test: `web/tests/unit/date-parser.test.mjs`

`parseDateAndRecurrenceInput(input, today)` returns `ParsedDateTimeRecurrence = { date, time, recurrence }`. Extend it to also report which date field the phrase targets — a leading `deadline`/`scheduled`/`due` keyword, else `null`.

- [ ] **Step 1: Write the failing tests**

Append to `web/tests/unit/date-parser.test.mjs`:

```javascript
import { parseDateAndRecurrenceInput } from "../../src/lib/date-parser.ts";

test("parseDateAndRecurrenceInput — field keyword", () => {
  const fixed = new Date(2026, 4, 22); // Fri May 22 2026
  assert.equal(parseDateAndRecurrenceInput("deadline friday", fixed)?.field, "deadline");
  assert.equal(parseDateAndRecurrenceInput("scheduled tomorrow", fixed)?.field, "scheduled");
  assert.equal(parseDateAndRecurrenceInput("due may 1", fixed)?.field, "deadline"); // `due` → deadline
  assert.equal(parseDateAndRecurrenceInput("tomorrow", fixed)?.field, null);        // bare → null
  // keyword + recurrence still parses both
  const r = parseDateAndRecurrenceInput("deadline every day", fixed);
  assert.equal(r?.field, "deadline");
  assert.equal(r?.recurrence, "daily");
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test:unit date-parser` (from `web/`; check `web/package.json` `test:unit` for the exact form — the recurrence work added tests here already).
Expected: the new test FAILS (`field` is `undefined`).

- [ ] **Step 3: Implement**

In `date-parser.ts`, change the type:

```typescript
export type ParsedDateTimeRecurrence = ParsedDateTime & {
  recurrence: string | null;
  field: "deadline" | "scheduled" | null;
};
```

Add a helper near `extractRecurrence`:

```typescript
/** Strip a leading `deadline`/`scheduled`/`due` keyword. `due` → deadline. */
function extractField(raw: string): { field: "deadline" | "scheduled" | null; rest: string } {
  const m = raw.match(/^(deadline|scheduled|due)\s+(.+)$/);
  if (!m) return { field: null, rest: raw };
  const field = m[1] === "due" ? "deadline" : (m[1] as "deadline" | "scheduled");
  return { field, rest: m[2] };
}
```

Rewrite `parseDateAndRecurrenceInput` to run `extractField` first:

```typescript
export function parseDateAndRecurrenceInput(
  input: string,
  today: Date = new Date(),
): ParsedDateTimeRecurrence | null {
  const raw = input.trim().toLowerCase();
  if (!raw) return null;
  const { field, rest: afterField } = extractField(raw);
  const recExtracted = extractRecurrence(afterField);
  const parsed = parseDateInput(recExtracted.rest, today);
  if (!parsed) {
    // A keyword + recurrence with no date (`deadline every day`) is still
    // valid — the recurrence anchors to today downstream.
    if (recExtracted.recurrence && (field || afterField !== recExtracted.rest)) {
      const y = today.getFullYear();
      const m = String(today.getMonth() + 1).padStart(2, "0");
      const d = String(today.getDate()).padStart(2, "0");
      return { date: `${y}-${m}-${d}`, time: null, recurrence: recExtracted.recurrence, field };
    }
    return null;
  }
  return { ...parsed, recurrence: recExtracted.recurrence, field };
}
```

(If the current code already handles the no-date+recurrence case differently, keep its behaviour and only add `field` — read the file first and adapt. The key requirement: `field` is on the result.)

- [ ] **Step 4: Run tests**

Run: `pnpm test:unit date-parser` → all PASS (existing date-parser tests + the new one).

- [ ] **Step 5: Commit**

```bash
git add web/src/lib/date-parser.ts web/tests/unit/date-parser.test.mjs
git commit -m "feat(web): parse a deadline/scheduled field keyword in date input"
```

---

## Task 2: Settings — `bareDateField` preference

**Files:**
- Modify: `web/src/lib/preferences.svelte.ts`
- Modify: `web/src/routes/settings/general/+page.svelte`

A keyword-less date routes to a configurable field; default `scheduled`.

- [ ] **Step 1: Add the preference**

In `web/src/lib/preferences.svelte.ts`, add the type + state + setter, following the existing `bulletStyle` pattern exactly:

```typescript
export type BareDateField = "deadline" | "scheduled";
```

Inside `class Preferences`:

```typescript
  bareDateField = $state<BareDateField>(
    load<BareDateField>("bareDateField", "scheduled"),
  );

  setBareDateField(v: BareDateField): void {
    this.bareDateField = v;
    save("bareDateField", v);
  }
```

- [ ] **Step 2: Add the settings control**

In `web/src/routes/settings/general/+page.svelte`, read the file to see the existing control pattern (e.g. the Vim-mode toggle ~lines 87–96) and the section structure. Add a new labelled control for `bareDateField` — a two-option segmented control (Deadline / Scheduled), bound to `prefs.bareDateField` / `prefs.setBareDateField`. Match the page's existing markup conventions (the same label text size, the same control styling family as the other settings). Caption it so the meaning is clear, e.g. "A date typed without a `deadline`/`scheduled` keyword sets this field."

- [ ] **Step 3: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -cE "preferences.svelte.ts|settings/general"` → expect `0`.
Run: `pnpm build` → succeeds.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/preferences.svelte.ts web/src/routes/settings/general/+page.svelte
git commit -m "feat(web): bareDateField setting — default field for a keyword-less date"
```

---

## Task 3: Extract `formatDateMonthDay` to a shared module

**Files:**
- Create: `web/src/lib/date-format.ts`
- Create: `web/tests/unit/date-format.test.mjs`
- Modify: `web/src/lib/components/DisplayChip.svelte`

The properties row (Task 5) and `DisplayChip` both need the human date formatter. Extract it so it's shared and unit-tested.

- [ ] **Step 1: Write the failing test**

Create `web/tests/unit/date-format.test.mjs`:

```javascript
import { test } from "node:test";
import assert from "node:assert/strict";
import { formatDateMonthDay } from "../../src/lib/date-format.ts";

test("formatDateMonthDay — bare ISO and bracketed", () => {
  const yr = new Date().getFullYear();
  assert.equal(formatDateMonthDay(`${yr}-05-22`), "May 22");
  assert.equal(formatDateMonthDay(`[[${yr}-05-22]]`), "May 22");
  assert.equal(formatDateMonthDay("2025-12-31"), "Dec 31, 2025");
  assert.equal(formatDateMonthDay(`${yr}-05-22 15:30`), "May 22 3:30p");
  assert.equal(formatDateMonthDay("not-a-date"), "not-a-date");
});
```

- [ ] **Step 2: Run to verify failure**

Run: `pnpm test:unit date-format` → FAIL (module missing).

- [ ] **Step 3: Create the module**

Create `web/src/lib/date-format.ts` with the function currently in `DisplayChip.svelte` (verbatim — read `DisplayChip.svelte` and move it; it accepts `[[YYYY-MM-DD]]`-with-optional-time and bare ISO, returns `"May 22"` / `"May 22, 2025"` / `"May 22 3:30p"`, and returns the trimmed input unchanged when it doesn't match):

```typescript
/** Human-readable rendering of a date property value. Accepts a bare
 *  `YYYY-MM-DD` (optionally ` HH:mm`) or a `[[YYYY-MM-DD]]`-wrapped value.
 *  Unrecognized input is returned trimmed-but-unchanged. */
export function formatDateMonthDay(v: string): string {
  const m =
    v.trim().match(/^\[\[(\d{4})-(\d{2})-(\d{2})\]\](?:\s+(\d{2}):(\d{2}))?$/) ||
    v.trim().match(/^(\d{4})-(\d{2})-(\d{2})(?:\s+(\d{2}):(\d{2}))?$/);
  if (!m) return v.trim();
  const [, y, mo, d, hh, mm] = m;
  const date = new Date(Number(y), Number(mo) - 1, Number(d));
  const month = date.toLocaleString("en-US", { month: "short" });
  const day = Number(d);
  const thisYear = new Date().getFullYear();
  const datePart = Number(y) === thisYear ? `${month} ${day}` : `${month} ${day}, ${y}`;
  if (!hh) return datePart;
  let h = Number(hh);
  const ampm = h >= 12 ? "p" : "a";
  h = h % 12 || 12;
  const minStr = mm === "00" ? "" : `:${mm}`;
  return `${datePart} ${h}${minStr}${ampm}`;
}
```

- [ ] **Step 4: Point `DisplayChip` at it**

In `DisplayChip.svelte`, delete the local `formatDateMonthDay` function and `import { formatDateMonthDay } from "$lib/date-format";` instead. Behaviour is unchanged.

- [ ] **Step 5: Run tests + verify**

Run: `pnpm test:unit date-format` → PASS. Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -c "DisplayChip.svelte"` → `0`. Run: `pnpm build` → succeeds.

- [ ] **Step 6: Commit**

```bash
git add web/src/lib/date-format.ts web/tests/unit/date-format.test.mjs web/src/lib/components/DisplayChip.svelte
git commit -m "refactor(web): extract formatDateMonthDay to a shared, tested module"
```

---

## Task 4: The date command writes properties, not an inline link

**Files:**
- Modify: `web/src/lib/components/DatePicker.svelte`
- Modify: `web/src/lib/components/BlockEditor.svelte`

The `/date` slash command currently inserts a raw `[[YYYY-MM-DD]]` into block text. Rework it so the picker's commit upserts `scheduled::`/`deadline::` (+ `recurring::`) block *properties* — bare ISO, no brackets — routing the field by the NL keyword or the `bareDateField` setting.

- [ ] **Step 1: Extend `DatePicker`'s `onPick` to emit the parsed field**

In `DatePicker.svelte`: the `onPick` prop signature becomes
`onPick: (iso: string, time: string | null, recurrence: string | null, field: "deadline" | "scheduled" | null) => void;`

The component already parses its NL input via `parseDateAndRecurrenceInput` (which now returns `field`). At the two commit sites — the Enter handler and the grid-click handler — pass the field:
- Enter commit: pass the `field` from the latest NL parse (the component holds the parsed result; if it keeps `selectedRecurrence` it should also keep `selectedField` — add a `let selectedField = $state<"deadline"|"scheduled"|null>(null)` updated wherever the NL input is parsed, mirroring how `selectedRecurrence` is tracked).
- Grid click commit: there is no NL keyword for a pure calendar click — pass `null`.

Read `DatePicker.svelte` to see exactly how the NL parse result is currently stored (the recurrence work added `committedRecurrence`/`endClause` etc.) and mirror that for `field`.

- [ ] **Step 2: Rewrite the `BlockEditor` `onPick` handler**

In `BlockEditor.svelte`, the current handler is:

```typescript
onPick={(iso, _time, recurrence) => {
  if (view && datePickerCursor >= 0) {
    if (datePickerPropertyKey) {
      const doc = view.state.doc.toString();
      let next = upsertBlockProperty(doc, datePickerPropertyKey, `[[${iso}]]`);
      if (recurrence !== null) {
        next = upsertBlockProperty(next, "recurring", recurrence);
      }
      view.dispatch({ changes: { from: 0, to: doc.length, insert: next }, selection: { anchor: datePickerCursor } });
      onChange(next);
    } else {
      const doc = view.state.doc.toString();
      const before = doc.slice(0, datePickerCursor);
      const after = doc.slice(datePickerCursor);
      const inserted = `[[${iso}]]`;
      const next = before + inserted + after;
      view.dispatch({ changes: { from: 0, to: doc.length, insert: next }, selection: { anchor: before.length + inserted.length } });
      onChange(next);
    }
    view.focus();
  }
  showDatePicker = false;
  datePickerCursor = -1;
  datePickerPropertyKey = null;
}}
```

Replace it with — the inline-insert `else` branch is **deleted**, and values are written **bare** (no `[[]]`):

```typescript
onPick={(iso, _time, recurrence, field) => {
  if (view && datePickerCursor >= 0) {
    const doc = view.state.doc.toString();
    // `/p` path passes an explicit property key; the `/date` path resolves
    // the field from the NL keyword, falling back to the user's setting.
    const key = datePickerPropertyKey ?? field ?? prefs.bareDateField;
    let next = upsertBlockProperty(doc, key, iso);
    if (recurrence !== null) {
      next = upsertBlockProperty(next, "recurring", recurrence);
    }
    view.dispatch({
      changes: { from: 0, to: doc.length, insert: next },
      selection: { anchor: Math.min(datePickerCursor, next.length) },
    });
    onChange(next);
    view.focus();
  }
  showDatePicker = false;
  datePickerCursor = -1;
  datePickerPropertyKey = null;
}}
```

Add `import { prefs } from "$lib/preferences.svelte";` if not already imported. The `applySlash("date")` handler is unchanged — it still strips the slash and opens the picker; only the commit changed.

- [ ] **Step 3: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -cE "DatePicker.svelte|BlockEditor.svelte"` → `0`.
Run: `pnpm build` → succeeds. Run: `pnpm test:unit` → no regressions.
Reason through: `/date` then typing `deadline friday` → commit upserts `deadline:: 2026-05-29` on the block; typing `tomorrow` → upserts `scheduled:: <date>` (the default); a bare calendar click → `scheduled::` (default, `field` null). No `[[...]]` written.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/components/DatePicker.svelte web/src/lib/components/BlockEditor.svelte
git commit -m "feat(web): date command writes deadline/scheduled properties, not inline links"
```

---

## Task 5: The properties row — render

**Files:**
- Create: `web/src/lib/components/BlockDateRow.svelte`
- Modify: `web/src/lib/components/BlockOutliner.svelte`

A compact labelled strip beneath a task block, rendering its date/recurrence properties.

- [ ] **Step 1: Create `BlockDateRow.svelte` (display)**

Create `web/src/lib/components/BlockDateRow.svelte`. Props: `block` (the `ParsedBlock`) — it reads `block.properties` (the `{ [key]: string }` dict) and `block.id`. Render a horizontal strip; for each of `scheduled`, `deadline`, `recurring` that is present and non-empty, render a labelled field:

- `SCHEDULED` / `DEADLINE` — label + `formatDateMonthDay(value)` (import from `$lib/date-format`). The date text is a button: clicking navigates to that day's daily page. To navigate, dispatch the same event the existing wiki-link click path uses — read `cm-decorations.ts` / how a `[[page]]` click opens a page (there is an `openPage`/navigation path; reuse it). If the navigation hook isn't trivially reachable from a plain component, use an `<a href={`/p/${isoDate}`}>` or the app's router `goto` — match how other components navigate to a page.
- `REPEAT` — label + `formatRecurrence(value)` (import from `$lib/recurrence-format`) + a small "skip" button calling `skipRecurrence(block.id)` (import from `$lib/recurrence-actions`).

If none of the three properties is present, render nothing (`{#if}` guard). Style it as a compact muted strip — match the visual weight of the existing `DisplayChip` row in `BlockOutliner` (small text, muted label color). Keep this component display-only for now; editing lands in Task 6.

- [ ] **Step 2: Mount it in `BlockOutliner.svelte`**

`BlockOutliner.svelte` renders the per-block chip region around lines 1395–1422 (the `<div class="shrink-0 flex items-center gap-1 …">` with blocked/rollup/`DisplayChip`). The properties row is a *full-width row beneath the block line*, not a trailing chip — read the block-rendering markup to find the element that wraps one block's line, and mount `<BlockDateRow {block} />` directly after it (so it appears on its own line under the task). Import `BlockDateRow`. Guard so it only renders for blocks that have a `scheduled`/`deadline`/`recurring` property (the component already self-guards, but avoid mounting it for every plain block — gate on `block.properties.scheduled || block.properties.deadline || block.properties.recurring`).

- [ ] **Step 3: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -cE "BlockDateRow|BlockOutliner.svelte"` → `0`.
Run: `pnpm build` → succeeds. Run: `pnpm test:unit` → no regressions.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/components/BlockDateRow.svelte web/src/lib/components/BlockOutliner.svelte
git commit -m "feat(web): properties row — render date/recurrence under a task block"
```

---

## Task 6: The properties row — click-to-edit

**Files:**
- Modify: `web/src/lib/components/BlockDateRow.svelte`
- Modify: `web/src/lib/components/BlockOutliner.svelte`

Make each field in the row editable: click → a `DatePicker` opens pre-filled → on commit the property is upserted into the block and saved.

- [ ] **Step 1: Understand the block save path**

Read `BlockOutliner.svelte` to find how a single block's text is mutated and persisted (the per-block content/save mechanism — how editing a block's text flows to disk; the recurrence work / JournalView used a debounced `onContentChange` per note, BlockOutliner will have an analogous per-block save). You need a function that, given a block and new raw text, persists it. Note its exact name and signature.

- [ ] **Step 2: Wire editing**

In `BlockDateRow.svelte`: each field's value becomes a button that, on click, opens the standalone `DatePicker` component (import it) positioned near the field, pre-filled (`initialDate` from the current value, `initialRecurrence` for the repeat field). On `onPick`, build the new block text with `upsertBlockProperty(block.rawText, key, iso)` (+ `recurring`) and call up to `BlockOutliner` via a prop callback `onEditBlock: (blockId: string, newText: string) => void`. `BlockOutliner` implements that callback by routing `newText` through the block save path found in Step 1.

For the `/p`-style explicit-key edit, the field key is known (`scheduled`/`deadline`/`recurring`) — pass it so the `DatePicker` commit targets that property. (`DatePicker`'s `onPick` field arg can be ignored here since the row already knows the key.)

If the block save path in `BlockOutliner` turns out to be hard to reach cleanly from a child component, report DONE_WITH_CONCERNS describing the seam rather than forcing a fragile wire-up — but the editable row is the user's #1 requirement ("no way to edit this date after it's set"), so prefer to make it work.

- [ ] **Step 3: Verify**

Run: `cd web && npx svelte-check --threshold error --tsconfig ./tsconfig.json 2>&1 | grep -cE "BlockDateRow|BlockOutliner.svelte"` → `0`.
Run: `pnpm build` → succeeds. Run: `pnpm test:unit` → no regressions.
Reason through: clicking `SCHEDULED May 25` opens the picker on May 25; picking May 27 upserts `scheduled:: 2026-05-27` and the row re-renders; clicking `REPEAT` lets you change the recurrence.

- [ ] **Step 4: Commit**

```bash
git add web/src/lib/components/BlockDateRow.svelte web/src/lib/components/BlockOutliner.svelte
git commit -m "feat(web): properties row fields are click-to-edit"
```

---

## Self-Review

**Spec coverage (design spec §1–7):** §1 data model — Task 4 writes bare `deadline::`/`scheduled::`, no `[[]]`; §2 command + NL grammar — Task 1 (field keyword) + Task 4 (command writes properties); §3 properties row — Task 5 (render) + Task 6 (edit); §4 recurrence + skip — Task 5 (REPEAT field + `skipRecurrence`), engine already done; §5 settings — Task 2; §6 migration — none needed (Task 3's `formatDateMonthDay` accepts bracketed *and* bare, so old `deadline:: [[..]]` still renders); §7 out of scope — agenda view and iOS are not in any task. Covered.

**Type consistency:** `field: "deadline" | "scheduled" | null` is defined in Task 1 and consumed by `DatePicker.onPick` (Task 4) and the `BlockEditor` handler (Task 4). `formatDateMonthDay` (Task 3) is imported by `DisplayChip` (Task 3) and `BlockDateRow` (Task 5). `upsertBlockProperty(rawText, key, value)` is the existing signature used by Tasks 4 and 6. `bareDateField` (Task 2) is read as `prefs.bareDateField` in Task 4.

**Placeholder scan:** Tasks 5 and 6 contain "read the file to find X" steps for the page navigation hook and the block save path — intentional: those depend on undocumented local wiring the implementer must confirm against real code, not guess. All code-bearing steps carry real code.

**Ordering:** Task 1 (field) and Task 2 (setting) precede Task 4 (which consumes both). Task 3 (extract formatter) precedes Task 5 (which imports it). Task 5 (render) precedes Task 6 (edit).
