# Current State

## Branch
- `main` — pushed through `f7f31f32`+; newer doc/spec commits may be unpushed (delete-refresh fix, e2e test, multi-device spec, this). **Remind Taylor to push.** `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit).

## DONE this run (sync stabilization — all resolved)
- Liveness, date chip, web→iOS delete, iOS→desktop push, desktop crash-loop (loro 1.12→1.13.6), disjoint-lineage convergence, desktop delete-refresh (`38b6ac3b` + e2e `pnpm test:e2e`).
- **Convergence (Phase 0 / layer-2) CONFIRMED working**: the `.relay` rebase-catch-up already exists (iOS `catchUpFromRelaySnapshots` on pending → `import_authoritative_snapshot`). June 25/26 stuck-forks HEALED when Taylor edited them on desktop (re-broadcast → pending → catch-up). Clean days + today work. (Residual theoretical nuance: snapshot-via-normal-tick uses lossy min-TreeID dedup not rebase — deferred, #12; do NOT blind-rebase relay-inbound = ping-pong risk.)

## NOW — NORTH STAR ARC: multi-device live presence + cursors (collab)
- Spec: `phases/2026-06-27-multidevice-presence-spec.md`. loro 1.13.6 gives `EphemeralStore` (presence) + stable `Cursor` FREE (verified), not in FFI yet. Transport: WS broadcast (desktop real-time) ✅; CF relay is store-poll → iOS-over-relay needs a CF-DO WebSocket later.
- [x] **Phase 1: FFI-wrap Cursor + EphemeralStore** (#13) DONE — engine `b7d26e92` (mint/resolve_block_cursor op-anchored + cross-engine portable; EphemeralStore presence round-trip/multi-peer/LWW; 4 tests) + FFI `5b6a8bf3` (mint_cursor/resolve_cursor/set_presence/apply_presence/presence_peers + PresencePeer Record; FFI round-trip test). loro-internal = "1.13" added (EphemeralStore not in public loro). Full tesela-sync (166+) + ffi (29) green.
- [x] **Phase 2: desktop presence over WS** (#14) DONE — verified end-to-end (two-page Playwright `presence.spec.ts`: page A's caret → live `.cm-remote-cursor` on page B, over the WS, no relay/reload).
  - SERVER `cce9afa7`: presence rides the EXISTING ws_delta_tx binary fan-out by a `PRES` magic — route_inbound_binary splits PRES (fan out, never engine/persist) from TLR2. 4 ws tests.
  - WEB `e8374836`: loro/presence.ts (PRES codec) + remote-cursors.ts (peer→caret store, LWW/10s decay, per-tab id+color) + cm-remote-cursors.ts (StateField + RemoteCursorWidget; .map auto-remaps through local edits, rebuild on fresh presence) + ws-client onPresence routing + BlockEditor throttled caret publish + extension per bound block + +layout onPresence. svelte-check 0 err; web unit 514 + 11 new; e2e green.
  - ARCH locked: web uses PLAIN {bid,utf16_offset} + CodeMirror decoration auto-remap (NOT loro Cursor).
- [ ] **Phase 3: iOS presence** (#15) — THE PHYSICAL-IPHONE BOUNDARY. Autonomous prep (sim): wire iOS FFI set_presence publish on caret + render remote caret overlay in Graphite/BlockRow + inbound subscribe; iOS SHOULD use the op-anchored loro Cursor (mint_cursor) since UITextView doesn't auto-remap. Transport: hub-mode WS (iOS→Mac, reuses Phase 2 PRES fan-out) first, then CF-DO WebSocket. REAL verify needs iPhone + Mac (two devices) — Taylor.
- [ ] Phase 4: collab polish (selection ranges, peer names/labels — web widget already supports a name flag, just not populated yet; follow-mode; presence sidebar; Savanne multi-user).
- Mode: BURN THROUGH autonomously (ultracode) — Phases 0/1/2 done; Phase 3 is the device boundary Taylor set.

## Deferred polish
- iOS #3 `/p1` slash deep-filter; #4 inline NLP (sim repro). Per-type color+logo. CF-DO-WebSocket presence transport (Phase 3 decision).
