# Current State
Branch: main (NOT pushed — Taylor reviews+pushes; build 75 bump local)

## Plan
- [x] Dictation modernization research DONE + fact-checked → phases/2026-07-08-dictation-transcribecpp-research.md; epic tesela-v5t (.1–.4, deps+triage set) filed; harness-deck `20260708-dictation-transcribecpp-research` AWAITS Taylor's direction answer (NVIDIA OML license, en-only streaming, phase order) — implementation gated on it
- [x] qv-web-jql "broken" answer QA'd (sandbox mosaic, live UI drive): flow SOUND — sighting = inbox→views rename mid-flight (resolved, 785/785 green) or by-design empty number-prop popup; follow-up ask `qv-web-jql-followup` appended to build-75 deck
- [x] inbox→views rename: implemented + verified
- [ ] ENV-BLOCKED closes (need Taylor's orphan kill, PIDs in chat): engc.6+epic (verify: cargo test -p tesela-sync -p tesela-sync-ffi -p tesela-server) · tesela-baa (verify: pnpm --dir web test:e2e storm spec; needs npx playwright install chromium)
- [ ] Product test deck (build 75) live — iOS uploaded; desktop bundle dist/ untracked; first answer in: qv-web-jql broken

## Blockers
- Loopback/trustd env until orphan kill (also blocks tesela-server spawn suites)

## Open Questions
- Taylor: build-75 deck — answer qv-web-jql-followup (which breakage was it?) + remaining blocks (tables, iOS, verdict) · dictation deck direction ask
- Taylor: build-74 deck (tesela/20260703-build74-product-test) still unanswered
- Audit findings await triage-into-Now: tesela-jow (decimal truncation, lead), tesela-0rc (untagged chip no-op)
