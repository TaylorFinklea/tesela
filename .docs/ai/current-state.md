# Current State

## Branch
- `main` (ahead of origin; unpushed — 43+ commits incl. `4766111` cmd-registry B1–B4, waves 1–4 merges, L2 spec). NOT pushed.
- Opus = orchestrator/Lead. Fleet = gpt-5.5 + minimax via pi (Bash); Claude subagents via Workflow. Evidence: `model-scorecard.md` + `model-bench.jsonl`.

## Plan
- [x] Waves 1–3 DONE (10 items). ED1 held — Lead `ED1-fix` (ViewPlugin can't line-break-replace; salvage in `.bench/wave3/logs/ed1.diff.patch`).
- [x] **Wave 4 DONE — 5/6 merged+scored:** DSK5(minimax 5 `9aec7bc9`), DSK6(gpt-5.5 5 `c4820230`), DSK7(gpt-5.5 5 `58e974bb`), PROP2(gpt-5.5 5 `9f14b254`), PROP4(minimax 4.5 `b1090353`). **PROP5 non-attempt** (minimax rate-limited 2064 → 0 lines) → re-dispatched wave5 on gpt-5.5.
- [x] **L2 slash-registry spec DONE** — `phases/2026-06-13-slash-registry-spec.md` (verb-only SlashContext, additive `run(arg?,ctx?)`, surface-gating, gut-don't-delete+grep gate). → B-impl-1..4 in backlog (unblocked, sequential senior chain). 4 keyboard-first open Qs for Taylor in spec (non-blocking).
- [~] **Wave 5 RUNNING** (bg) — iOS IOS1/ED2→gpt-5.5, IOS2/IOS3→minimax (verify=true; Opus runs INCREMENTAL xcodebuild in main at review — fresh worktree = cold whisper.cpp) + PROP5→gpt-5.5 (real pnpm verify). `.bench/wave5/`.
- [ ] **Next (Opus Lead):** review/merge/score wave 5 → then **L1** sync (min key/pairing + cursor migration, HA-first; big, fresh-context) and/or **ED1-fix** (bounded, salvage ready) and/or **B-impl-1** (slash-registry impl).
- [ ] Taylor: **H1–H4** confirms (real browser + Roshar); push when ready; 4 L2 keyboard Qs; green-light chezmoi items.

## Scorecard tally (waves 1–4)
- gpt-5.5 = **7/7 clean** (all 5/5). minimax = 6/8 (4.5–5 on output; **2 load-fails this wave** — prop4 errored-after-completion, prop5 zero-diff). minimax reliability-under-load is the emerging signal.

## Blockers
- None active. Roadmap "Now" STALE (Stream A/B shipped). minimax hitting 2064 high-load errors — prefer gpt-5.5 for must-land items.

## Open Questions
- L2 spec: 4 keyboard-first Qs for Taylor (editor-verb-no-block / unify slashKey+chord / typed-date-path / is-widget-a-slash-verb) — defaults baked in, non-blocking.
- CF relay deploy (needs Taylor's CF acct); desktop vs iOS sequencing.

## Notes
- Review routine proven 4×: read `_summary.txt` → per-item `diff.patch` → `git apply [--3way]` → Verify in main → commit w/ provenance → worktree remove → `model-bench.jsonl` row. Opus review IS the gate (caught ed1 runtime bug, prop2 false-flake, prop5 empty).
- Report: `phases/2026-06-13-opus-return-report.md`.
