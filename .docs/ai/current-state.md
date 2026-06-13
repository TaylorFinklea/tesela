# Current State

## Branch
- `main` (ahead of origin; unpushed — 43+ commits incl. `4766111` cmd-registry B1–B4, waves 1–4 merges, L2 spec). NOT pushed.
- Opus = orchestrator/Lead. Fleet = gpt-5.5 + minimax via pi (Bash); Claude subagents via Workflow. Evidence: `model-scorecard.md` + `model-bench.jsonl`.

## Plan
- [x] Waves 1–3 DONE (10 items). ED1 held — Lead `ED1-fix` (ViewPlugin can't line-break-replace; salvage in `.bench/wave3/logs/ed1.diff.patch`).
- [x] **Wave 4 DONE — 5/6 merged+scored:** DSK5(minimax 5 `9aec7bc9`), DSK6(gpt-5.5 5 `c4820230`), DSK7(gpt-5.5 5 `58e974bb`), PROP2(gpt-5.5 5 `9f14b254`), PROP4(minimax 4.5 `b1090353`). **PROP5 non-attempt** (minimax rate-limited 2064 → 0 lines) → re-dispatched wave5 on gpt-5.5.
- [x] **L2 slash-registry spec DONE** — `phases/2026-06-13-slash-registry-spec.md` (verb-only SlashContext, additive `run(arg?,ctx?)`, surface-gating, gut-don't-delete+grep gate). → B-impl-1..4 in backlog (unblocked, sequential senior chain). 4 keyboard-first open Qs for Taylor in spec (non-blocking).
- [x] **Wave 5 DONE — 5/5 merged+scored:** IOS1(gpt-5.5 5 `7e5bf379`), IOS2(minimax 5 `312d7e8f`), IOS3(minimax 5 `ba413d26`), ED2(gpt-5.5 5 `892e0ccc`) — combined incremental xcodebuild GREEN; all compiled first-try unaided. **PROP5**(gpt-5.5 5 `45d834cd`) — re-dispatch succeeded where minimax rate-limited; Opus flipped default ON→OFF (opt-in) + excluded its stale .docs edits.
- [x] **B-impl-1 DONE** (wave6, gpt-5.5 5/5) — registry types widened (editor?/surface/slashKey, `run(arg?,ctx?)`), ctx threaded through colon/palette/leader, `slash-context.ts` SlashContext type. Purely additive.
- [~] **B-impl-2+3 RUNNING** (wave7, gpt-5.5, bg) — `buildSlashContext()` producer + 2 pilot verbs (heading/date) wired through the registry, gut-don't-delete on their applySlash cases. Combined for a self-verifiable unit. `.bench/wave7/`. **Riskiest item (editor guarded-dispatch internals) — review carefully; defer merge to fresh focus if review can't be thorough.**
- [ ] **Then:** B-impl-4 (migrate remaining verbs + delete switch under grep gate) · **ED1-fix** (bounded, salvage in `.bench/wave3/logs/ed1.diff.patch`) · **L1** sync (HA-first — big, design = Taylor's call).
- [ ] Taylor: **H1–H4** confirms (real browser + Roshar); push when ready (18 new commits); 4 L2 keyboard Qs; PROP5 default-OFF — flip on if wanted; green-light chezmoi items.

## Scorecard tally (waves 1–5, `model-bench.jsonl` 32 rows)
- gpt-5.5 = **10/10 clean** (every item 5/5 — Rust, TS, Swift, bash; incl. the hardest UX item unaided). minimax = 8/10 quality 4.5–5 BUT **2 load-fails wave4** (prop4 errored-after-completing, prop5 zero-diff → re-dispatched to gpt-5.5). **Routing rule: minimax output is solid but it hits `2064` high-load errors under volume → gpt-5.5 for must-land/hard items, minimax for S mechanical.**

## Blockers
- None active. Roadmap "Now" STALE (Stream A/B shipped). minimax hitting 2064 high-load errors — prefer gpt-5.5 for must-land items.

## Open Questions
- L2 spec: 4 keyboard-first Qs for Taylor (editor-verb-no-block / unify slashKey+chord / typed-date-path / is-widget-a-slash-verb) — defaults baked in, non-blocking.
- CF relay deploy (needs Taylor's CF acct); desktop vs iOS sequencing.

## Notes
- Review routine proven 4×: read `_summary.txt` → per-item `diff.patch` → `git apply [--3way]` → Verify in main → commit w/ provenance → worktree remove → `model-bench.jsonl` row. Opus review IS the gate (caught ed1 runtime bug, prop2 false-flake, prop5 empty).
- Report: `phases/2026-06-13-opus-return-report.md`.
