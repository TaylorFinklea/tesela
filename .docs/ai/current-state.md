# Current State
Branch: main (NOT pushed — Taylor reviews+pushes; build 75 bump local)

## Plan
- [x] Dictation GREEN-LIT (all 4 phases, deck answer) · P1 SHIPPED+CLOSED (tesela-v5t.1): transcribe.cpp = the server engine, whisper-rs → mutually-exclusive fallback (dual-ggml link abort — ADR 2026-07-08); E2E verified whisper/canary/parakeet
- [ ] Dictation P2 (tesela-v5t.2, streaming spine: WS + web capture) + P3 (.3, iOS FluidAudio unified streaming) now unblocked — P2 next; desktop rebuild+relaunch rides with P2's web bundle
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
