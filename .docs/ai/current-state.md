# Current State

## Branch
- `main` — **MANY fix commits, NONE pushed** (builds 48–52 of work). Latest: `cdb4a0ec` (Loro-panic containment). `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit). **Remind Taylor to push.**

## Active work — iOS sync stabilization (multi-day)
- [x] **#1 sync liveness** (backoff ~2.3h + foreground couldn't wake parked loop) — `e6d1d83b`, build 48.
- [x] **#2 date chip hidden while editing** — `5c65e9d2`, build 48.
- [x] **#6 web→iOS delete (background APNs wake, not relay-scoped)** — `594b0403`, build 49.
- [x] **#7 iOS→desktop push (dirty note silently not exported — bad broadcast cursor)** — `56d67001`, build 51 (diagnosed via build-50 "Last splice" = applied=1 sent=0).
- [x] **#8 DESKTOP CRASH-LOOP (Loro 1.12 richtext apply panic on a poison inbound frame)** — `cdb4a0ec`. Inbound apply now probes each frame on an INDEPENDENT copy under catch_unwind + skips poison (never `fork()` — shares the poisonable LoroMutex). **Desktop rebuilt (`cargo tauri build`) → installed to /Applications by Taylor → running on the CF relay, stays up (inbound_cursor advancing, last_error null).** decisions.md 2026-06-26. **iOS build 52 cutting** (same `tesela-sync` fix → iOS poison-safe).
- [ ] **#3 slash `/p1` deep-filter parity** — port web `flattenedSlashFilter` to iOS `SlashVerbs`.
- [ ] **#4 inline NLP not firing** — needs SIM repro; don't speculative-fix (build-47 gate). ⚠
- [ ] **#5 per-type color+logo** — roadmap'd (later).

## Blockers
- None (desktop recovered; iOS build cutting).

## Open questions / next pick
- **Poison note `e9624f2c…` is FROZEN** — its inbound frame is skipped (never applies) by the containment. Root = a loro 1.12 concurrent-richtext-merge bug (OOB diff). Follow-up: loro upgrade OR avoid the concurrent-same-block-splice pattern; possibly surfaced by the build-51 snapshot-fallback. Not data loss elsewhere.
- **Verify build 51** (iOS push): edit Today → reaches desktop, "Last splice" sent≥1.
- **Verify build 52** (iOS): no crash on the poison frame; sync stays healthy.
- **#3 + #4** next; **push** the backlog (builds 48–52).
- Repro toolkit (reusable) in decisions.md 2026-06-25/26: sim-seed via `simctl defaults`, desktop API add/delete, `wrangler tail`, and `tesela-server --mosaic … TESELA_RELAY_URL=…` to repro relay-apply crashes (exit 124=survived, 134=abort).
