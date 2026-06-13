# Current State

## Branch
- `main` (ahead of origin; unpushed — incl. `4766111` cmd-registry B1–B4 merge + bench/doc commits). NOT pushed.
- Opus = orchestrator/Lead. Fleet = gpt-5.5 + minimax via pi/ralph; Claude subagents via Workflow. Evidence: `model-scorecard.md` + `model-bench.jsonl`.

## Plan
- [x] Head-to-head benchmark → **gpt-5.5 WON** → merged `4766111` (cmd-registry B1–B4). Evidence recorded.
- [x] Grounding triage (5-agent) → tiered backlog **`phases/2026-06-13-backlog.md`**. Properties UNBLOCKED (3 product calls already resolved 06-05); iOS reports fixed thru build 12; colon/heading "bugs" were CDP artifacts (→ H1/H2).
- [x] Cycle focus: all 4 fleet areas in parallel; Opus = sync (HA-first, defer CF).
- [x] **Fleet Waves 1+2 DONE** — 6 items merged + scored (DSK1, DSK2/3, DSK4, PROP1, B5, PROP3). gpt-5.5 = 5/5×3; minimax = 4.5–5. Loop proven 2×.
- [x] **Fleet Wave 3 DONE** — B6→gpt-5.5 (5/5 `89100bd6`), PROP6→minimax (4.5/5 `555472fa`) merged. **ED1 (gfm-table) NOT MERGED** — review caught a runtime bug (ViewPlugin can't do line-break-replacing decorations); → `ED1-fix` (Lead), parser/tests salvageable in `.bench/wave3/logs/ed1.diff.patch`.
- [ ] **Next:** iOS wave (submodule-aware worktree: IOS1/2/3, ED2) + remaining web/desktop (DSK5/6/7, PROP2/4/5). Then Opus Lead: L2 slash-registry spec → L1 sync (HA-first) → ED1-fix.
- 10 fleet items merged this session (waves 1–3); gpt-5.5 = 5/5×4, minimax = 4.5/5×4 + one held (ed1, 3.0). Opus review is the safety gate (caught ed1).
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
