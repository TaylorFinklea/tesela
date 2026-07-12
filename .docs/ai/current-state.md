# Current State
Branch: main (synced with origin at plan start; local phase commits are never pushed)

## Plan — tesela-myh canonical lift → ewj.1 → real product test (2026-07-11)
- [x] P0 Lead decision/spec: canonical structural lift; `.docs/ai/phases/2026-07-11-nonbullet-canonical-lift-spec.md`. Verify: `test -s .docs/ai/phases/2026-07-11-nonbullet-canonical-lift-spec.md`
- [x] P1 core fence-aware full-coverage scanner + canonical serializer (TDD) · tier_floor: senior · complexity: M. Verify: `cargo test -p tesela-core note_tree`
- [x] P2 engine hydration/materialization/cold-reload/two-engine convergence + fence-safe indexing/classification (TDD) · tier_floor: senior · complexity: M. Verify: `cargo test -p tesela-sync -p tesela-core`
- [x] P3 web+iOS display/edit regressions for lifted heading/prose/fence blocks · tier_floor: senior · complexity: M. Verify: `pnpm --dir web check && pnpm --dir web test:unit && xcodebuild test -project app/Tesela-iOS/Tesela-iOS.xcodeproj -scheme Tesela -destination 'platform=iOS Simulator,name=iPhone 17'`
- [x] P4 ewj.1 writer seam + one shared stable note-id helper · tier_floor: senior · complexity: M. Verify: `cargo test -p tesela-core -p tesela-sync -p tesela-cli -p tesela-mcp`
- [x] P5 in-process active/temporary-engine import + idempotence/scale integration test; Lead review · tier_floor: senior · complexity: L. Verify: `cargo test -p tesela-server -p tesela-sync`
- [ ] P6 real-graph sandbox import, restart/reimport QA, report + harness-deck product test · tier_floor: senior · complexity: M. Verify: `cargo test -p tesela-server --test import_product_test && test -s .docs/ai/phases/2026-07-11-nonbullet-canonical-lift-report.md`

## Blockers
- P6 ends at Taylor's named human product check; source graph and live mosaic remain read-only.

## Open Questions
- None — Taylor delegated direction and authorized MiniMax/GLM/Ollama/GPT/Claude adversarial dispatch.
