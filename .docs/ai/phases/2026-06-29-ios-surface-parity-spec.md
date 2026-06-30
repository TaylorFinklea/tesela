# Spec: iOS surface parity — slash /p1 + inline NLP + presence on pages & past-day

Status: SPEC (2026-06-29). Product decisions LOCKED by Taylor. Implement AFTER the
T1 hardening batch (M1/iOS-presence-robustness/web-presence) lands + builds — T1's
iOS presence agent and this both edit the iOS editor files; do NOT run concurrently.

## Root cause (from the iOS-gap investigation)
iOS HAS full Swift impls of slash priority (`EditorAutocomplete.SlashVerbs.registryVerbs`
emits `/p1` → `.setProperty(priority,p1)`) AND inline NLP (`EditorAutocomplete.InlineNLP.detect`
+ `DateParser.swift`), wired end-to-end — but ONLY on the collab editor (`CollabTextView`),
which `BlockRow` uses only when `onTextSplice` is wired, which is ONLY `GrDailyView` today-section
(GrDailyView.swift:145). `GrPageView` + the yesterday/past sections use the plain SwiftUI
`legacyEditField` (`BlockRow.swift:697-731`) → NO slash, NO NLP, NO presence. None of this logic
is shared Rust core — it's hand-ported TS↔Swift (drift risk; out of scope here).

## Decisions (LOCKED)
- **Scope: pages + past-day too.** Bring the collab editor (slash + NLP) + live presence to
  `GrPageView` AND `GrDailyView` yesterday/past sections — not just today.
- **NLP: auto-lift on blur (match web).** On blur, strip the token + set the property with no tap
  (mirror web `BlockEditor.svelte:1734-1743` `detectTaskTokens`→strip→`onSetProperty`). The
  existing tap-chip becomes optional/live-preview, not the required path.

## Fix path (integration, not new core)
1. **Surface coverage (biggest lever).** Route `GrPageView` + the yesterday/past sections through
   the collab `CollabTextView` path: wire `onTextSplice` + `wireAutocompleteSources()` + the chip
   strip, instead of `legacyEditField`. Files: `Graphite/Views/GrPageView.swift` (~169),
   `GrDailyView.swift` (yesterday ~220 / past ~291), `Components/BlockRow.swift` (513-520 collab
   wiring vs 697-731 legacy). **KEY RISK / verify first:** the collab editor's `onTextSplice`
   currently routes to the TODAY-daily splice path — pages + past-day need their OWN splice/write
   path (find how a page block / a yesterday block is written today via the legacy field →
   `onSetProperty`/writeback → route the collab splice to the same engine write). A page may not
   have a per-block splice equivalent; if so, wire it (mirror today's `spliceTodayBlock` for the
   page/past-day note). Do NOT regress the today path.
2. **NLP auto-lift on blur.** Add a blur handler (CollabTextView / the autocomplete coordinator)
   that runs `InlineNLP.detect` over the block, strips the matched token, and calls
   `setBlockPropertyAndPush` for each lifted prop — no tap. Sequence carefully so the strip + the
   property write + the text writeback don't drop the user's edit. Keep the live chip as a
   non-required preview (or drop it) to match web's feel.
3. **Builtins registry fallback (do regardless).** When `rebuildPropertyRegistry`
   (`MockMosaicService.swift:2669`) yields zero type pages, fall back to
   `PropertyRegistry.buildBuiltins()` (already exists, `PropertyRegistry.swift:807`) so `/p1` + NLP
   work BEFORE Property/Tag type pages sync to the device (web is always seeded via listNotes).
4. **Fuzzy slash filter.** Replace the substring filter (`EditorAutocomplete.swift:297`
   `label.contains||id.contains`) with a fuzzy scorer mirroring web `scoreFuzzy`/
   `flattenedSlashFilter` (web/src/lib/editor/slash-filter.ts) so `/pri`, typos, etc. match.

## Verify
- xcodebuild (sim) clean; on-device: tag a block `#Task` on a PAGE and on a PAST-DAY block →
  `/p1` sets priority; type `due tomorrow` + blur → property auto-lifted; remote cursor/chip shows.
- Regression: today's daily editing + presence unchanged.

## Out of scope (flag for later)
- Shared Rust+FFI parser to stop the TS↔Swift date/priority/NLP drift (the duplicated
  `date-parser.ts`/`task-tokens.ts` vs `DateParser.swift`/`InlineNLP`). Larger, optional.
