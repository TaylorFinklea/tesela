# Current State
Branch: main (NOT pushed — Taylor reviews+pushes; build 75 bump local)

## Plan
- [x] Dictation modernization research DONE + fact-checked → phases/2026-07-08-dictation-transcribecpp-research.md; epic tesela-v5t (.1–.4, deps+triage set) filed; harness-deck `20260708-dictation-transcribecpp-research` AWAITS Taylor's direction answer (NVIDIA OML license, en-only streaming, phase order) — implementation gated on it
- [ ] qv-web-jql deck answer = "Something's broken" but note text MISSING — self-QA agent reproducing web JQL flow to find it; then fix batch
- [x] inbox→views rename: implemented + verified; commit pending
- [ ] ENV-BLOCKED closes (need Taylor's orphan kill, PIDs in chat): engc.6+epic (verify: cargo test -p tesela-sync -p tesela-sync-ffi -p tesela-server) · tesela-baa (verify: pnpm --dir web test:e2e storm spec; needs npx playwright install chromium)
- [ ] Product test deck (build 75) live — iOS uploaded; desktop bundle dist/ untracked; first answer in: qv-web-jql broken

## Blockers
- Loopback/trustd env until orphan kill (also blocks tesela-server spawn suites)

## Open Questions
- Taylor: WHAT broke in web JQL authoring? (qv-web-jql answer arrived without its note)
- Taylor: build-74 deck (tesela/20260703-build74-product-test) still unanswered
- Audit findings await triage-into-Now: tesela-jow (decimal truncation, lead), tesela-0rc (untagged chip no-op)
