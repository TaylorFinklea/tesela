# Current State

## Branch
- `main` — pushed through `f7f31f32`+; newer doc/spec commits may be unpushed (delete-refresh fix, e2e test, multi-device spec, this). **Remind Taylor to push.** `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit).

## DONE this run (sync stabilization — all resolved)
- Liveness, date chip, web→iOS delete, iOS→desktop push, desktop crash-loop (loro 1.12→1.13.6), disjoint-lineage convergence, desktop delete-refresh (`38b6ac3b` + e2e `pnpm test:e2e`).
- **Convergence (Phase 0 / layer-2) CONFIRMED working**: the `.relay` rebase-catch-up already exists (iOS `catchUpFromRelaySnapshots` on pending → `import_authoritative_snapshot`). June 25/26 stuck-forks HEALED when Taylor edited them on desktop (re-broadcast → pending → catch-up). Clean days + today work. (Residual theoretical nuance: snapshot-via-normal-tick uses lossy min-TreeID dedup not rebase — deferred, #12; do NOT blind-rebase relay-inbound = ping-pong risk.)

## NOW — NORTH STAR ARC: multi-device live presence + cursors (collab)
- Spec: `phases/2026-06-27-multidevice-presence-spec.md`. loro 1.13.6 gives `EphemeralStore` (presence) + stable `Cursor` FREE (verified), not in FFI yet. Transport: WS broadcast (desktop real-time) ✅; CF relay is store-poll → iOS-over-relay needs a CF-DO WebSocket later.
- [~] **Phase 1: FFI-wrap Cursor + EphemeralStore** (#13) — design+verify workflow running; then implement TDD (cursor survives concurrent edit; presence round-trip + timeout; FFI round-trip). Autonomous (no device).
- [ ] Phase 2: desktop presence over WS + Playwright e2e (autonomous). [ ] Phase 3: iOS (sim → CF-DO WS; physical iPhone for final verify). [ ] Phase 4: collab polish.
- Mode: Taylor said BURN THROUGH testing autonomously (ultracode) until the physical iPhone is genuinely needed.

## Deferred polish
- iOS #3 `/p1` slash deep-filter; #4 inline NLP (sim repro). Per-type color+logo. CF-DO-WebSocket presence transport (Phase 3 decision).
