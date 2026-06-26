# Current State

## Branch
- `main` — **MANY fix commits, NONE pushed** (builds 48–53 + the loro upgrade). Latest: `e884edc2` (loro 1.13.6). `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored — NEVER commit). **Remind Taylor to push.**

## Active work — iOS sync stabilization (multi-day)
- [x] #1 sync liveness (backoff ~2.3h) — `e6d1d83b`, build 48.
- [x] #2 date chip hidden while editing — `5c65e9d2`, build 48.
- [x] #6 web→iOS delete (background APNs wake, relay-scoped) — `594b0403`, build 49.
- [x] #7 iOS→desktop push (bad broadcast cursor → not exported) — `56d67001`, build 51. **Confirmed working**: iOS "Last splice … applied=1 sent=1".
- [x] #8 DESKTOP CRASH-LOOP (loro 1.12 richtext OOB panic on poison frame) — contained `cdb4a0ec` (isolated-copy probe + catch_unwind + mem::forget; never fork). Desktop rebuilt+reinstalled, ran clean.
- [~] **#9 CONVERGENCE (disjoint-lineage drift)** — block `019f047a`: desktop "Brook" vs iOS "Bro" (forked, won't merge).
  - [x] **Layer 1: loro 1.12→1.13.6** `e884edc2` (fixes crash class + atomic import rollback; existing dedup/heal now CONVERGES forked twins). Full suite green. **Shipping: iOS build 53 (cutting) + desktop rebuild (pending).**
  - [ ] **Layer 2: mergeable containers** (no-data-loss root fix) — specced `phases/2026-06-26-mergeable-containers-spec.md`. HARD migration; FRESH work.
- [ ] #3 slash `/p1` deep-filter parity. [ ] #4 inline NLP (needs sim repro). [ ] #5 per-type color+logo (later).

## Blockers
- Desktop needs rebuild+reinstall on 1.13.6 (Taylor runs the /Applications install — harness blocks it).

## Open questions / next pick
- **VERIFY layer 1 heals drift:** once iOS build 53 + the 1.13.6 desktop are both installed, block `019f047a` should converge to the SAME value on both (dedup runs instead of crashing). Taylor confirms.
- Layer 2 (mergeable containers) = next major; design the fleet migration first (prototype on a real-mosaic copy).
- **#3 + #4** after convergence settles; **PUSH** (builds 48–53 + upgrade, all local).
- Repro toolkit: decisions.md 2026-06-25/26 (sim-seed via `simctl defaults`, desktop API add/del, `wrangler tail`, `tesela-server --mosaic … TESELA_RELAY_URL=…` exit 124=survived/134=abort; relay compacts poison deltas → can't recapture fixtures).
