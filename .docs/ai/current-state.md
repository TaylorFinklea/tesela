# Current State
Branch: feat/block-drag-dailies (worktree; based on main d12e6d6b)

## Plan — tesela-b54 block subtree relocation
- [x] Approved interaction, persistence, failure, and verification design. Verify: `test -s .docs/ai/phases/2026-07-12-block-subtree-relocation-spec.md`
- [x] Taylor approved the committed spec. Verify: human review
- [x] Code-grounded implementation plan written. Verify: `test -s .docs/ai/phases/2026-07-12-block-subtree-relocation-plan.md`
- [ ] Tasks 1-4: pure contract, ownership, engine relocation, recovery. Verify: `cargo test -p tesela-sync engine::loro_engine::tests::relocation`
- [ ] Tasks 5-7: server route, web API/command, pointer + keyboard UI. Verify: `cargo test -p tesela-server --test block_subtree_move && pnpm --dir web check && pnpm --dir web test:unit`
- [ ] Task 8: E2E, rendered QA, full gates, report, bead close. Verify: `cargo test --workspace && cargo clippy --workspace -- -D warnings && pnpm --dir web test:e2e`

## Blockers
- None.

## Open questions
- None for approved v1 scope; concurrent same-root moves to different notes explicitly deferred.
