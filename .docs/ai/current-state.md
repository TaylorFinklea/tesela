# Current State
Branch: main (NOT pushed — Taylor reviews+pushes; build 75 bump local)

## Plan
- [x] AM: sqlite upsert race fix · tesela-baa multi-doc splice registry · engc.6 loro_engine split merged
- [x] PM: Query & Views feature set COMPLETE — epics ya4 + vp9 closed (10/10 beads; ADR 2026-07-07 "Query & Views"; specs phases/2026-07-02-typesystem-views-spec.md + 2026-07-07-jql-authoring-spec.md)
- [ ] ENV-BLOCKED closes (need Taylor's orphan kill, PIDs in chat): engc.6+epic (verify: cargo test -p tesela-sync -p tesela-sync-ffi -p tesela-server) · tesela-baa (verify: pnpm --dir web test:e2e storm spec; needs npx playwright install chromium)
- [ ] Product test deck published (harness-deck) — iOS build 75 uploaded; desktop app bundle+ZIP built locally (`target/.../Tesela.app`, `dist/desktop/` untracked); Taylor runs it; findings → next fix batch

## Blockers
- Loopback/trustd env until orphan kill (also blocks tesela-server spawn suites)

## Open Questions
- Taylor: build-74 deck (`tesela/20260703-build74-product-test`) still unanswered
- New audit findings await triage-into-Now: tesela-jow (decimal truncation, lead), tesela-0rc (untagged chip no-op)
