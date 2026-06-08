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

## Part 2b — per-tag-gated detection + dates + make-task-parse (next, fresh session)

**Gating model (decided 2026-06-08 — supersedes the marker/trailing/anywhere question).** NLP detection runs ONLY on blocks with a **detect-enabled tag**, configurable per-tag, default **on for `Task`**. This lets detection be FULLY AGGRESSIVE inside task blocks (bare multi-word dates `next tuesday`, `in 3 days` all work — the reason markers were rejected: they can't express multi-word dates) while prose blocks are untouched.

### Build steps (increments)

1. **`detect_tokens` tag flag + gate (infra + retrofit Part 2a).**
   - Seed `detect_tokens: true` on the Task tag page (fixtures `crates/tesela-fixtures/src/lib.rs` Task tag; gitignored `notes/task.md`; live mosaic `~/Library/Application Support/tesela/logseq/notes/task.md`).
   - Compute `detectEnabledTags: Set<string>` (lowercased) in `BlockOutliner` from `allNotes` (tag pages where `metadata.custom.detect_tokens === true`), ALWAYS include `"task"` as a code default. Mirror how tag config is already read (`allNotes.find(note_type === "Tag")`, ~233/265).
   - Thread it into the editor via a **CodeMirror Facet** — mirror `hiddenPropertyKeysFacet` end-to-end (cm-decorations.ts:277 define → BlockEditor compartment-wrapped facet setter → read via `view.state.facet(...)`, see cm-decorations.ts:466 + the `primaryTagFacet` plumbing at :286). New `detectEnabledTagsFacet`.
   - **GATE (both highlight + lift): the block's DIRECT tags only.** `getBlockTags(doc)` (block-tags.ts — own `tags::` + inline `#tags`) ∩ `detectEnabledTags` ≠ ∅. NEVER `ParsedBlock.inherited_tags` (the #journal-child must stay prose even though it inherits #Task). Both surfaces operate on the block's own text, so inheritance can't leak — but assert it in the test.
   - Apply the gate to: the priority highlight loop in `buildDecorations` (cm-decorations) AND the Enter-handler priority lift in BlockEditor (retrofits Part 2a — `p1` then only lifts on a `#Task` block, killing the `review p1 feedback` false-positive).
   - **Verify:** parent `#Task` block detects `p1`; child `#journal` block (inherits Task) does NOT.

2. **⌘↵ make-task = "tag it AND parse it."** In the Mod-Enter make-task path (BlockEditor ~1040-1055, the `onCycleStatus`/make-task branch), after adding the Task tag, run `detectTaskTokens` on the block + emit `onSetProperty` per prop + strip — so typing `do dishes tom p1` then ⌘↵ retroactively lifts `tom`→scheduled + `p1`→priority. (Confirmed wanted.)

3. **Date detection (extend `task-tokens.ts`).** Inside enabled blocks, scan line-0 for date candidates and validate via `parseDateAndRecurrenceInput` (date-parser.ts — handles `next tuesday`, `in 3 days`, `jun 9`, ISO, AND the `due`/`deadline`/`scheduled` leading keyword via `extractField`, + recurrence). Bare date → `scheduled`; `due`/`deadline` keyword → `deadline`. `cm-tesela-date` mark (teal). Lift via `onSetProperty({key:'scheduled'|'deadline', value:'[[YYYY-MM-DD]]'})` — match BlockDateRow's `[[…]]` value shape (confirm vs `formatDateMonthDay`). Capture/discard the time component? (decide; BlockDateRow shows date only.)
   - Add the **blur lift** here too (Part 2a deferred it) with a double-lift guard (no-op when detector finds nothing), now that gating limits it to task blocks.

### Fresh-block caveat (carry over from 2a)
The property container op resolves the block by `note_id:bid`; on a just-typed block the lift can race the text-save. Realistic typing auto-saves first. Pre-existing (shared with `status::` on fresh blocks). The make-task path (step 2) commits the tag+text first, which should also establish the block before the property ops — verify it doesn't reintroduce the race.
