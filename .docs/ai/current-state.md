# Current State

## Branch
- `main` — pushed through `02ec3233` (origin/main up to date as of the 1.13.6 + delete-test + corrected-spec work). Newer doc/state commits may be unpushed. `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit).

## Active work — iOS sync stabilization (multi-day) — CONVERGENCE SOLVED
- [x] #1 liveness `e6d1d83b` (b48) · #2 date chip `5c65e9d2` (b48) · #6 web→iOS delete APNs `594b0403` (b49) · #7 iOS→desktop push `56d67001` (b51, confirmed sent=1).
- [x] #8 desktop crash-loop (loro 1.12 richtext OOB) — contained `cdb4a0ec`.
- [x] **#9 CONVERGENCE — RESOLVED via loro 1.12→1.13.6 (`e884edc2`).** **Taylor verified on 1.13.6 desktop + iOS build 53 (2026-06-26): drift HEALED; simultaneous same-block edits BOTH survive (shared lineage interleaves).** The disjoint-fork problem is resolved for daily use. Mergeable-containers was the WRONG plan (verified by a 5-agent workflow — it's a tree-node fork). Layer-2 rebase-on-relay-inbound (no-loss for rare forks) = DEFERRED robustness, not needed now. Spec `phases/2026-06-26-mergeable-containers-spec.md`.

## NOW — remaining
- [ ] **iOS delete needs a MANUAL refresh on desktop** (Taylor 2026-06-26). DIAGNOSED + SCOPED (task #10 has the full recipe). NOT a sync/engine bug — committed test `relay_inbound_delete_updates_peer_materialized_file` (tesela-sync-ffi) PROVES a relay-inbound delete drops the block from the peer's materialized .md (the body the API serves). Pure WEB UI reconcile bug: edits auto-show, deletes don't. **Repro wall:** a standalone Chrome client (chrome-devtools MCP) against the live desktop server renders the journal EMPTY at /g AND /p/dailies (day headers, all day textboxes `value="\n"`, no block text) though the API returns bodies + sync says Connected — the app only hydrates blocks in the Tauri webview / `pnpm dev`. So a faithful repro needs the perf-harness shape: rebuild `tesela-server` (binary was cleaned), seed/copy a mosaic, `pnpm dev` (TESELA_API_TARGET), then Playwright/Chrome at the dev `/p/dailies`; trigger via raw `DELETE /notes/{id}/blocks/{bid}` (not own-echo → `WsEvent::NoteUpdated` → faithful reconcile). **Prime suspect (unconfirmed):** `BlockOutliner.applyExternalReparse` :745 client-minted-focus skip, likely triggered by `JournalView.ensureTrailingEmpty` re-appending a client-minted bullet after a delete → whole reparse skipped. ⚠ clobber-guard surface — confirm via the faithful repro, do NOT ship speculatively. Minor (manual-refresh workaround). **Fresh focused session recommended.**
- [ ] #3 slash `/p1` deep-filter. [ ] #4 inline NLP (sim repro). [ ] #5 per-type color+logo.

## North star (Taylor 2026-06-26)
- **True multi-device + live presence/cursors (collab).** The shared-lineage convergence just fixed is the PREREQUISITE (can't show meaningful cursors on forked docs). Future build: a presence channel (cursor/selection broadcast over relay/WS) + UI. Aligns with `project_emacs2_northstar` (RTC).

## Blockers / next pick
- None. Next: reproduce + fix the iOS-delete desktop auto-refresh. Then #3/#4.
