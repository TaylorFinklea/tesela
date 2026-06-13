# Current State

## Branch
- `main` (ahead of origin; unpushed — incl. `4766111` cmd-registry B1–B4 merge + bench/doc commits). NOT pushed.
- Opus = orchestrator/Lead. Fleet = gpt-5.5 + minimax via pi/ralph; Claude subagents via Workflow. Evidence: `model-scorecard.md` + `model-bench.jsonl`.

## Plan
- [x] Head-to-head benchmark → **gpt-5.5 WON** → merged `4766111` (cmd-registry B1–B4). Evidence recorded.
- [x] Grounding triage (5-agent) → tiered backlog **`phases/2026-06-13-backlog.md`**. Properties UNBLOCKED (3 product calls already resolved 06-05); iOS reports fixed thru build 12; colon/heading "bugs" were CDP artifacts (→ H1/H2).
- [x] Cycle focus: all 4 fleet areas in parallel; Opus = sync (HA-first, defer CF).
- [x] **Fleet Wave 1 DONE** — DSK1→gpt-5.5 (5/5 `a4f81b03`), PROP1→minimax (4.5/5 `7390af30`) merged + scored. Pipeline validated end-to-end.
- [ ] **Fleet Wave 2 launched** (async, non-iOS) — DSK2+3→minimax, DSK4→gpt-5.5, B5→minimax, PROP3→minimax → review+merge+score on completion → wave 3 (iOS via submodule-aware worktree + remaining web/desktop).
- [ ] Opus Lead: **L1** sync (min key/pairing + cursor migration, HA-first) + **L2** slash-as-registry spec (north-star #1).
- [ ] Taylor: **H1–H4** confirms (real browser + Roshar); product decisions as they arise.

## Blockers
- (RESOLVED) Opus CAN self-dispatch the fleet: `pi` via Bash + Claude subagents via Workflow. Only `claude --dangerously-skip-permissions` self-grant is blocked (not needed).
- Roadmap "Now" STALE — Stream A relay-hardening + Stream B Graphite cutover both fully shipped.

## Open Questions
- Product decisions pending: CF relay deploy (needs Taylor's CF acct); 3 property calls (harness-deck `tesela/20260605-properties-product-qs`); desktop vs iOS sequencing.

## Notes
- kimi reliability fail #3 (zero-line diff). Codex computer-use shipped iOS bugs (collapse/older-date/views/tags) thru build 12 — verify, don't redo.
- Report: `phases/2026-06-13-opus-return-report.md`.
