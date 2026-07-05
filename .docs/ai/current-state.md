# Current State
Branch: arena/tesela-cmdd.6

## Plan
- [x] tesela-cmdd.6: De-version the live behavior layer: lib/v4,lib/v5 renames + token migration — Verify: `pnpm --dir web check && pnpm --dir web test:unit && pnpm --dir web build` · tier_floor: senior · complexity: M
  - Done: `lib/commands`, `lib/leader`, quarantined `lib/legacy/v4-token-aliases.css`; role-token migration enforced by unit contract; verify green.

## Blockers
- none

## Open Questions
- none
