# Current State

## Branch
- `main` @ `776f1d2a` == origin/main — **pushed, clean tree** (2026-06-20). `.docs/ai/review/` + `AuthKey_*.p8` are untracked (the latter is gitignored; never commit it).

## Last session — instant cross-device sync SHIPPED + working
- **✅ APNs instant-sync WORKING end-to-end** (#72, iOS build 39, CF Worker). Edit on one device → relay deposit → content-available APNs push → other device catches up in seconds. `wrangler tail` confirmed `registered ✓` on both devices + `push → 200 OK` both directions. Full arc (P1 flush-on-background → P2a BGTask → P3 receiver+token-POST+entitlement+relay push) in `phases/2026-06-18-sync-durability-spec.md`. Builds 31–39.
- **The real blocker was a 413, not APNs**: CF Worker `wrangler.toml` shipped the 1 MiB body cap; bumped `TESELA_RELAY_MAX_BODY` → 16 MiB (`bb94742a`). That unblocked sync AND APNs (the failed tick never reached token registration).
- **Decision: CF Worker is the relay NOW** (`desktop.toml relay_url = https://tesela-relay.finklea.workers.dev`); HA self-host **parked** (Rust `tesela-relay` APNs port + HA add-on wiring are committed + ready — `#74`). Both relays zero-knowledge (adversarially verified). ⚠ APNs host = **production** (TestFlight tokens are prod regardless of the `development` entitlement string).
- Autonomous adds (2026-06-20): **dead-APNs-token prune** on 410/BadDeviceToken (both relays, `dc9a8af5`); **recurrence parser** gained biweekly/fortnightly/quarterly/`every other <unit>`/`every weekday` (`776f1d2a`); **3E code-block rendering confirmed DONE** (roadmap was stale — web+iOS already render fenced code).

## Now / pending
- [ ] **Taylor is testing instant-sync (build 39)** — awaiting his report on the feel / suspended-device wake.
- [ ] **CF Worker `wrangler deploy`** to ship the dead-token prune (NOT urgent — only matters after a device reinstall).
- [ ] **#73 desktop /g live-update deploy** — the #70 fix is committed; needs a Tauri rebuild + `/Applications` swap (Taylor's running-app env).
- [ ] **#75 multi-device clobber** (flagged, NOT urgent): an iOS edit was overwritten by a web/iPad edit. Family of the known convergence work; reproduce deliberately later — potential data loss, don't lose it.

## Blockers
- None.

## Open questions / next pick
- Next track is Taylor's pick. Strong candidates: **#75 clobber** (repro + diagnose), **3D recurring-task auto-roll** (the parser's ready), **3B block-merge-on-Backspace**. iOS editor track + the sync-durability arc are both complete.
