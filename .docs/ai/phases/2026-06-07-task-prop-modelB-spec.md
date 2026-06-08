# Task properties — Model B "detect-inline, lift-below" (spec)

Decided 2026-06-07 (harness-deck `20260607-task-property-ux`). Decision log: decisions.md 2026-06-08.

## Goal

As you type a task block, detected tokens (priority `p1`–`p4`, natural-language dates, a deadline variant) highlight **inline**; on **commit** they lift OUT of the prose into structured block properties (priority / scheduled / deadline) shown in the below-block strip (Part 1). Todoist smart-add feel.

## Status

- **Part 1 (display) — DONE** (`616762e`): `priority.ts` flags, `BlockDateRow` strip shows ⚑P1, `BlockOutliner` ROW_OWNED_KEYS dedup. Flag is display-only.
- **Part 2a (priority detect+lift) — THIS phase.** Completes the "set priority by typing" gap.
- **Part 2b (date detect+lift) — next.**

## Reuse (from scope wf_ea5bd325)

- **Dates:** `date-parser.ts` `parseDateInput(input, today=new Date()) → {date:'YYYY-MM-DD', time}|null` covers today/tomorrow/weekday/`jun 9`/ISO/relative. `parseDateAndRecurrenceInput` adds the `deadline|due|scheduled` leading-keyword `field` + recurrence. Pass explicit `today` in tests.
- **Priority:** `priority.ts` `priorityLevel` / `PRIORITY_CYCLE`.
- **Inline highlight:** add regex `Decoration.mark`s in the existing `teselaDecorations` ViewPlugin / `buildDecorations` (`cm-decorations.ts`), mirroring the `TAG_RE` loop (~503). Non-atomic. Two CSS surfaces: `teselaDecorationTheme` (~955) **and** `graphite-editor.css` (~92).

## ⚠ Lift WRITE seam — structured, not text (P1.13)

Do **NOT** call `upsertBlockProperty` / write a `priority:: ` text line — that dual-writes and duplicates (server materializes the container; documented reverted bug, BlockOutliner ~1165). The live seam is:

- `onSetProperty?.({key, value})` (BlockEditor ~602) → `setBlockPropertyStructured(block.id, key, value)` (BlockOutliner ~1171) — container op, optimistic update. Empty value → clear.

The **strip** side (remove the token from text) mirrors the Model-A chip commit: `view.dispatch({changes:{from:0,to:doc.length,insert:stripped}, selection})` and let the normal change path persist (for LoroText-bound blocks the `updateListener`→`onLoroText` path handles it — do NOT also call `onChange` for bound blocks).

## Lift TRIGGER seam

- **Primary — Enter:** inside `blockKeymap` Enter handler (BlockEditor ~1659). After `const doc = v.state.doc.toString()` and BEFORE the split branch: run the detector on `doc` line-0, emit `onSetProperty` per prop, compute `stripped`, then run the EXISTING split logic on `stripped` (recompute cursor offsets, or restrict lift to cursor-at-line-0-end).
- **Secondary — blur:** in the blur handler (~1485), before `onBlur()`: read doc, run detector, if changed dispatch the strip + emit `onSetProperty`. No-op when detector finds nothing (avoids double-lift after Enter).
- **Gate:** only when `!showAutocomplete && !showSlashMenu`. Scope detection to **line 0** (prose), never `tags::`/`status::` continuation lines.

## Part 2a — priority detect+lift (build now)

- **Detector** (new `lib/task-tokens.ts`, pure + testable): scan line-0 for `\bp[1-4]\b` (case-insensitive). Return `{stripped, props:[{key:'priority', value:'p1'}]}`. If multiple, last wins; strip all matches. (Dates added in 2b.)
- **Highlight:** `priorityInlineMark` class `cm-tesela-priority` in `cm-decorations.ts`; color from `priority.ts` FLAGS (per-level: P1 red / P2 amber / P3 blue / P4 muted). Add to both CSS surfaces.
- **Wire** the lift (Enter + blur) calling the detector + `onSetProperty`.
- **Verify (e2e, Playwright):** type "ship it p1" → Enter → block shows ⚑P1 flag + "p1" stripped from prose; "p2"/"p3" likewise; a bare "p" or "p9" does nothing; `report.p1` mid-word not matched.

## Part 2b — date detect+lift (next)

- Detector scans line-0 for date candidates (`today|tomorrow|<weekday>|<month> <day>|YYYY-MM-DD|in N days|…`), validates each via `parseDateInput` (only real dates lift/highlight). Bare date → `scheduled`; deadline variant → `deadline`.
- **OPEN DECISION (2b):** deadline syntax — `!<date>` prefix (mock-up) vs the existing `due`/`deadline` leading keyword (already in `extractField`). Lean: support the `due`/`deadline` keyword (reuses parser) + maybe `!`. Decide before building 2b.
- False-positive risk: bare NL words (tomorrow/weekday) in normal prose. Highlight gives feedback; consider trailing-edge-only or a sigil if noisy.
- `cm-tesela-date` mark; lift via `onSetProperty({key:'scheduled'|'deadline', value: '[[YYYY-MM-DD]]'})` (match BlockDateRow's `[[…]]` value shape — confirm against `formatDateMonthDay`).
