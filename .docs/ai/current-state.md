# Current State

## Branch
- `main` (ahead of origin; `08f8448` test-flakiness hardening this session — unpushed).
- Opus back as orchestrator (2026-06-13). Roles: Opus = triage/spec/review/Lead-impl; cheap fleet (minimax-m3, gpt-5.5, kimi) via pi/ralph for S/M/L. Evidence = `model-scorecard.md`.

## Plan
- [x] Recon + model review + ratings + docs-vs-code reconcile (workflow).
- [x] Fix flaky `tesela-server` tests pass-1 (`08f8448`); residual port-collision → backlog.
- [x] Web/desktop smoke (Chrome DevTools vs real-mosaic copy on :7474).
- [x] iOS shell/mock smoke (sim launches, Graphite renders).
- [ ] Product questioning session → rebuild roadmap Now/Next/Later. **NEXT.**
- [ ] Write tiered backlog (corrective actions in report) + first fleet dispatch.

Report: `phases/2026-06-13-opus-return-report.md`. Scorecard: `model-scorecard.md`.

## Blockers
- Roadmap "Now" is STALE: Stream A + Stream B both fully shipped; reality is in the report. Rebuild after product session.
- Residual test port-collision flakiness (1/3 workspace runs) → fleet Verify gates use `-p <pkg>` scope until fixed.
- Milestone 3 sync spine = all Lead/XL (Opus); CF deploy needs Taylor's CF account.

## Open Questions
- Product session pending (priorities; keyboard-first colon+slash gap; iOS deep pass).
