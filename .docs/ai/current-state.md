# Current State

## Branch
- `main` — pushed through `02ec3233` (origin/main up to date as of the 1.13.6 + delete-test + corrected-spec work). Newer doc/state commits may be unpushed. `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit).

## Active work — iOS sync stabilization (multi-day) — CONVERGENCE SOLVED
- [x] #1 liveness `e6d1d83b` (b48) · #2 date chip `5c65e9d2` (b48) · #6 web→iOS delete APNs `594b0403` (b49) · #7 iOS→desktop push `56d67001` (b51, confirmed sent=1).
- [x] #8 desktop crash-loop (loro 1.12 richtext OOB) — contained `cdb4a0ec`.
- [x] **#9 CONVERGENCE — RESOLVED via loro 1.12→1.13.6 (`e884edc2`).** **Taylor verified on 1.13.6 desktop + iOS build 53 (2026-06-26): drift HEALED; simultaneous same-block edits BOTH survive (shared lineage interleaves).** The disjoint-fork problem is resolved for daily use. Mergeable-containers was the WRONG plan (verified by a 5-agent workflow — it's a tree-node fork). Layer-2 rebase-on-relay-inbound (no-loss for rare forks) = DEFERRED robustness, not needed now. Spec `phases/2026-06-26-mergeable-containers-spec.md`.

## NOW — remaining
- [x] **iOS delete needs a MANUAL refresh on desktop — FIXED `38b6ac3b`.** Pure WEB reconcile bug (engine proven correct). `BlockOutliner.applyExternalReparse` own-echo fast-path compared `targetBody` to the STALE `lastSentBody` (only advances on local save); an inbound ADD diverged the render, then an inbound DELETE restoring `lastSentBody` was skipped. Fix: compare the CURRENT render `buildFullContent(blocks).bodyOnly === targetBody` + keep the mid-typing guard; `lastExternalBody` untouched (PUT base). Verified RED→GREEN via a live Chrome repro + self-contained Playwright `pnpm test:e2e` (`51407e0b`). decisions.md 2026-06-27. **SHIPPING: web rebuilt → desktop `cargo tauri build` running → Taylor reinstalls /Applications (harness blocks the write).**
- [ ] #3 slash `/p1` deep-filter. [ ] #4 inline NLP (sim repro). [ ] #5 per-type color+logo.

## North star (Taylor 2026-06-26)
- **True multi-device + live presence/cursors (collab).** The shared-lineage convergence just fixed is the PREREQUISITE (can't show meaningful cursors on forked docs). Future build: a presence channel (cursor/selection broadcast over relay/WS) + UI. Aligns with `project_emacs2_northstar` (RTC).

## Blockers / next pick
- None. Next: #3 (`/p1` slash deep-filter) + #4 (inline NLP, sim repro first). Then the north-star multi-device-cursors arc.
- **PUSH** — commits since last push: the delete-refresh fix + e2e test + docs.
