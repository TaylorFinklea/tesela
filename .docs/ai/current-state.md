# Current State

## Branch
- `main` (ahead of origin; unpushed — incl. `4766111` cmd-registry B1–B4 merge + bench/doc commits). NOT pushed.
- Opus = orchestrator/Lead. Fleet = gpt-5.5 + minimax via pi/ralph; Claude subagents via Workflow. Evidence: `model-scorecard.md` + `model-bench.jsonl`.

## Plan
- [x] Head-to-head benchmark (5 models, blind panel) → **gpt-5.5 WON** → merged `4766111` (cmd-registry B1–B4). Evidence recorded.
- [ ] **Product session (IN PROGRESS)** — cycle focus = all 4 fleet areas in parallel (keyboard spine / app stability / editor-render / properties); Opus takes sync. Build thorough roadmap.
- [ ] Grounding pass — drive real e2e/smoke (web re-confirm + iOS real-data on a sim I drive + desktop launch) → triaged cross-platform bug inventory.
- [ ] Rebuild roadmap Now/Next/Later (Stream A+B shipped) + write tiered backlog (Lead→Opus; S/M→fleet).
- [ ] Dispatch fleet loops + Opus starts sync spine (Milestone 3).

## Blockers
- (RESOLVED) Opus CAN self-dispatch the fleet: `pi` via Bash + Claude subagents via Workflow. Only `claude --dangerously-skip-permissions` self-grant is blocked (not needed).
- Roadmap "Now" STALE — Stream A relay-hardening + Stream B Graphite cutover both fully shipped.

## Open Questions
- Product decisions pending: CF relay deploy (needs Taylor's CF acct); 3 property calls (harness-deck `tesela/20260605-properties-product-qs`); desktop vs iOS sequencing.

## Notes
- kimi reliability fail #3 (zero-line diff). Codex computer-use shipped iOS bugs (collapse/older-date/views/tags) thru build 12 — verify, don't redo.
- Report: `phases/2026-06-13-opus-return-report.md`.
