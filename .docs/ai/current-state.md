# Current State

## Branch
- `main` — pushed through `f7f31f32`+; newer doc/spec commits may be unpushed (delete-refresh fix, e2e test, multi-device spec, this). **Remind Taylor to push.** `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit).

## DONE this run (sync stabilization — all resolved)
- Liveness, date chip, web→iOS delete, iOS→desktop push, desktop crash-loop (loro 1.12→1.13.6), disjoint-lineage convergence, desktop delete-refresh (`38b6ac3b` + e2e `pnpm test:e2e`).
- **Convergence (Phase 0 / layer-2) CONFIRMED working**: the `.relay` rebase-catch-up already exists (iOS `catchUpFromRelaySnapshots` on pending → `import_authoritative_snapshot`). June 25/26 stuck-forks HEALED when Taylor edited them on desktop (re-broadcast → pending → catch-up). Clean days + today work. (Residual theoretical nuance: snapshot-via-normal-tick uses lossy min-TreeID dedup not rebase — deferred, #12; do NOT blind-rebase relay-inbound = ping-pong risk.)

## NOW — NORTH STAR ARC: multi-device live presence + cursors (collab)
- Spec: `phases/2026-06-27-multidevice-presence-spec.md`. loro 1.13.6 gives `EphemeralStore` (presence) + stable `Cursor` FREE (verified), not in FFI yet. Transport: WS broadcast (desktop real-time) ✅; CF relay is store-poll → iOS-over-relay needs a CF-DO WebSocket later.
- [x] **Phase 1: FFI-wrap Cursor + EphemeralStore** (#13) DONE — engine `b7d26e92` (mint/resolve_block_cursor op-anchored + cross-engine portable; EphemeralStore presence round-trip/multi-peer/LWW; 4 tests) + FFI `5b6a8bf3` (mint_cursor/resolve_cursor/set_presence/apply_presence/presence_peers + PresencePeer Record; FFI round-trip test). loro-internal = "1.13" added (EphemeralStore not in public loro). Full tesela-sync (166+) + ffi (29) green.
- [~] **Phase 2: desktop presence over WS** (#14) — server presence frame (≠ TLR2) + in-memory broadcast + echo-suppress; web renders remote carets as CodeMirror decorations. ARCH DECISION: web path uses PLAIN {block_id, utf16_offset} + CodeMirror decoration auto-remap (NOT the loro Cursor — that's for iOS Phase 3 where UITextView doesn't auto-remap). Verify: Playwright e2e two-page. [ ] Phase 3: iOS (sim → CF-DO WS; physical iPhone final verify). [ ] Phase 4: collab polish.
- Mode: Taylor said BURN THROUGH testing autonomously (ultracode) until the physical iPhone is genuinely needed.

## Deferred polish
- iOS #3 `/p1` slash deep-filter; #4 inline NLP (sim repro). Per-type color+logo. CF-DO-WebSocket presence transport (Phase 3 decision).
