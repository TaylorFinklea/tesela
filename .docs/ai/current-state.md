# Current State
Branch: main (ahead 8, unpushed)

## Plan
- [x] Fix tesela-73b (P0): relocation boot recovery fails SOFT. Verify: `cargo test -p tesela-sync -p tesela-server` — 292 + 107 green, clippy clean. Committed `afae5d02`
- [ ] Week-1 correctness floor, in order: tesela-gqd + tesela-fdd + tesela-zip (one relay-durability train) → tesela-9ut → tesela-h8m + tesela-507 → tesela-6hu. Verify: each bead's `verify_cmd`
- [?] Taylor verifies parent-plus-children drag and relaunch persistence in installed `ef750d55`

## Blockers
- Human: physical Safari + `/Applications/Tesela.app` drag, then relaunch persistence.

## Open questions
- None. Taylor's 2026-07-14 calls: perfected daily driver · desktop+iOS equally · RTC full build behind the tesela-hx8 dark-ship gate · widgets both (in-app first) · all four properties gaps · P0 first.
