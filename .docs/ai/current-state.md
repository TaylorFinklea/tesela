# Current State
Branch: main (ahead 3, unpushed)

## Plan
- [ ] Fix tesela-73b (P0): relocation boot recovery must fail SOFT (quarantine, don't `?`-propagate into engine open). Verify: `cargo test -p tesela-sync` + new test asserting engine opens with an unrecoverable intent on disk
- [ ] Fix tesela-9ut (P1): splice guard is dead code — pass noteId not slug; audit all reserve()/isReserved() key domains. Verify: `pnpm --dir web test:unit`
- [ ] Taylor answers direction questions (widgets scope · properties parity priority · RTC gate acceptance)
- [?] Taylor physically drags a parent plus children between days and verifies persistence after relaunch

## Blockers
- Human: physical desktop drag in installed `/Applications/Tesela.app`.
- Human: 3 product questions from the 2026-07-14 audit (see roadmap Now).

## Open questions
- Widgets = in-app dashboard, OS/home-screen, or both? (Sol flagged the ambiguity; blocks widget scoping.)
- Which properties gap hurts most (relations/backlinks · per-property color+icon · global registry UI · sets/collections)?
- RTC ships dark behind a kill switch until durability gates pass — accept? (tesela-hx8)
