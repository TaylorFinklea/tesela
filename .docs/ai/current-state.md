# Current State

## Branch
- `main`; Ralph-loop code validated; iOS TestFlight build `1.1 (12)` uploaded; desktop `0.1.1` Developer-ID signed/notarized.
- Release commits include iOS/desktop version bumps + handoff docs only; build artifacts remain ignored.

## Plan
- [ ] A1 — Fix clippy errors. Verify: `cargo clippy --workspace -- -D warnings` (minimax-m3 ralph loop).
- [ ] B1 — Unified command registry shape + port palette/leader. Verify: `pnpm --dir web check` + manual palette/leader/colon QA (kimi-k2.7-code ralph loop).
- [ ] B2 — Keymap introspection + conflict detection. Verify: unit tests + `pnpm --dir web check` + `:keymap` QA (kimi-k2.7-code).
- [ ] B3 — Context-aware command dispatch. Verify: `pnpm --dir web check` + full keyboard QA matrix (kimi-k2.7-code).

Spec: `.docs/ai/phases/command-registry-spec.md`.

## Blockers
- Tauri DMG bundling failed in `bundle_dmg.sh`; notarized ZIP shipped instead.
- Lead/XL sync/FFI/pairing items remain reserved for Opus/Fable.

## Open Questions
- None.
