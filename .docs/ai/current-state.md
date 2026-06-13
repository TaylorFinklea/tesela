# Current State

## Branch
- `main` (ahead of origin; unpushed: `08f8448` test-flakiness, `856152d` recon/smoke docs, `5621529` model-eval methodology).
- Opus = orchestrator. Roles: Opus = triage/spec/review/Lead-impl; cheap fleet via pi/ralph. Evidence = `model-scorecard.md` + `model-bench.jsonl`.

## Plan
- [x] Recon + model review + docs-vs-code reconcile; web + iOS smoke; product session (balanced split; keyboard-first #1; sync = HA/on-paper; properties deferred).
- [x] Flaky `tesela-server` tests pass-1 (`08f8448`); residual port-collision → backlog.
- [x] Model-eval hardened: anti-bias head-to-head methodology + deterministic JSONL ledger.
- [ ] **AWAITING TAYLOR: launch head-to-head benchmark** — he runs `! bash /Users/tfinklea/git/tesela/.bench/run.sh > /Users/tfinklea/git/tesela/.bench/driver.log 2>&1 &` (I'm locked out of self-authorizing permission-bypass agents). 5 models, isolated worktrees, objective Verify.
- [ ] On `BENCH_COMPLETE` (`.bench/logs/_meta.txt`): blind-judge anonymized `.bench/logs/*.diff.patch` → record to `model-bench.jsonl` + scorecard → merge winner.
- [ ] Write tiered backlog (corrective actions in report) + rebuild roadmap Now/Next/Later (Stream A/B done).
- [ ] Spec command-registry completion (colon+slash all views) — Opus Lead.

Report: `phases/2026-06-13-opus-return-report.md`.

## Blockers
- Fleet dispatch can't be self-authorized by Opus (safety classifier blocks permission-bypass self-grant) → **human launches each batch via `!`**. Key orchestration-model finding.
- Roadmap "Now" STALE (Stream A/B shipped); rebuild after benchmark.

## Open Questions
- Head-to-head results pending (will it overturn the preliminary roster? Opus/Sonnet unproven as implementers).
