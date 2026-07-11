# Current State
Branch: main (NOT pushed — Taylor reviews+pushes; well ahead of origin: cycle-2 + the 2026-07-10 fleet cycle)

## Plan — GPT-5.6 fleet cycle (2026-07-10, Fable-led) — DONE; awaiting Taylor's product test
- [x] 6 parity features shipped+merged (Lead-reviewed, web 805 green): FTS-in-⌘K (8zd.10), attachments route (8zd.1), paste-upload (8zd.2), PDF (8zd.4), rail (8zd.13), sync-dot (ewj.8), block-move (8zd.15)
- [x] Security: attachment CSP-sandbox+nosniff (automated-review catch, live-verified). tesela-myh interim reseed gate SHIPPED (Sol, TDD) — still open tracking the durable wt5-class fix + gating nnm.3
- [x] Lead specs 8zd.5/.7/.3: Terra draft → Sol REJECT (28 findings) → Terra revise (0 contested) → on main, implementation-ready
- [x] Desktop rebuilt+relaunched on the fresh bundle (running). Server-side of all 6 features mechanically verified live (attachments/upload/traversal/FTS/relay-status). Scorecard+bench: 11 dispatches logged; digest regenerated
- [?] AWAITING TAYLOR: product test `tesela/20260710-fleet-product-test` (6 UI checks) + dictation retest after build-76 model-download failure fix
- [ ] Next wave (on Taylor's go): implement 8zd.5/.7/.3 from the approved specs — ALL ewj.1-gated (import-engine adoption lands first)

## Blockers
- ewj.1 (import sole-writer) gates the whole parity wave + block-refs; still open, Lead-review-required
- tesela-64g: sigterm_triggers_validated_backup now fails CONSISTENTLY — poisons `cargo test -p tesela-server`; workspace fmt drift blocks `cargo fmt --all --check` (bead filed)

## Open Questions
- Taylor: product-test verdict (which of 6 pass) · push main · greenlight next parity wave
- Taylor: next-build live dictation retest (download + 640 vs 1120ms default) · device relay-topology check · phone-Logseq-since-Jun-16
