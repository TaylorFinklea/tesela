# Current State

## Branch
- `main` @ `37635a7d` — **1 unpushed commit** ahead of origin/main (`01480753`): the recurring auto-roll (`37635a7d`). Sync-trust commits are pushed. `.docs/ai/review/` + `AuthKey_*.p8` untracked (latter gitignored; never commit it).

## Last session — instant cross-device sync SHIPPED + working
- **✅ APNs instant-sync WORKING end-to-end** (#72, iOS build 39, CF Worker). Edit on one device → relay deposit → content-available APNs push → other device catches up in seconds. `wrangler tail` confirmed `registered ✓` on both devices + `push → 200 OK` both directions. Full arc (P1 flush-on-background → P2a BGTask → P3 receiver+token-POST+entitlement+relay push) in `phases/2026-06-18-sync-durability-spec.md`. Builds 31–39.
- **The real blocker was a 413, not APNs**: CF Worker `wrangler.toml` shipped the 1 MiB body cap; bumped `TESELA_RELAY_MAX_BODY` → 16 MiB (`bb94742a`). That unblocked sync AND APNs (the failed tick never reached token registration).
- **Decision: CF Worker is the relay NOW** (`desktop.toml relay_url = https://tesela-relay.finklea.workers.dev`); HA self-host **parked** (Rust `tesela-relay` APNs port + HA add-on wiring are committed + ready — `#74`). Both relays zero-knowledge (adversarially verified). ⚠ APNs host = **production** (TestFlight tokens are prod regardless of the `development` entitlement string).
- Autonomous adds (2026-06-20): **dead-APNs-token prune** on 410/BadDeviceToken (both relays, `dc9a8af5`); **recurrence parser** gained biweekly/fortnightly/quarterly/`every other <unit>`/`every weekday` (`776f1d2a`); **3E code-block rendering confirmed DONE** (roadmap was stale — web+iOS already render fenced code).

## Now / pending
- [ ] **Taylor is testing instant-sync (build 39)** — awaiting his report on the feel / suspended-device wake.
- [ ] **CF Worker `wrangler deploy`** to ship the dead-token prune (NOT urgent — only matters after a device reinstall).
- [ ] **#73 desktop /g live-update deploy** — the #70 fix is committed; needs a Tauri rebuild + `/Applications` swap (Taylor's running-app env).
- [x] **#75 clobber RESOLVED + wake-from-suspend CONFIRMED** (2026-06-21): root cause was **config, not a code bug** — the iPhone was on **SERVER=HTTP** (writing to `127.0.0.1:7474`, a dead address on a real phone → edits silently vanished → device diverged → refresh clobbered local). Toggling to **Relay** fixed it: an iPhone edit appeared on the iPad ~1s after unlock (the APNs silent push woke the suspended iPad — the original 2h-gap scenario, fully closed + real-device validated).
- [x] **Sync-trust hardening SHIPPED** (2026-06-21, the silent-desync trap):
  - **localhost warning** (build 40, `d9e0ee36`): physical device + HTTP→`127.0.0.1`/localhost → loud amber warning + one-tap "Switch to Relay" in Settings.
  - **honest connection status** (build 41, `0b7f2403`): `MockMosaicService.refresh` `.http` catch no longer forces green `.ready` on HTTP failure — an unreachable backend now flips `.failed` ("Can't reach <host> — showing your local copy; changes are saved and will sync") on EVERY refresh, lighting up the ConnectionBanner / TopBar dot / Settings app-wide. Reads stay intact; reconnect loop self-heals. Built understand→implement→adversarial-review (3 lenses) workflows; review caught the "edits won't sync" overclaim (writes ride the relay independently) + banner truncation + dead `userInitiated` param — all fixed. New regression test; 25/25 green.
  - Deferred (acceptable, noted): amber "degraded" vs red "failed" visual split; surfacing write-path (`persistTaskToggle`) failures directly.
- [x] **Recurring-task auto-roll — iOS Relay-mode gap CLOSED** (build 42, `37635a7d`, 2026-06-21). Map finding: roll-on-completion was ALREADY built + working on desktop/web + iOS-HTTP (server `apply_post_save_bumps`); the gap was iOS **Relay** mode (engine path bypasses the server routes where the roll lives). `MockMosaicService.rollRecurringComplete` now rolls locally on completion via `onLocalPropertySet`, mirroring `rewrite_block_for_complete` (status→todo, dates stepped per-field, `last_completed`, `recurrence_done`); spent series keep `done`. Understand→implement→adversarial-review (3 lenses); review caught a BLOCKER (spent never persisted `done` → revert) + MAJOR (`[[...]]` wrapping so CRDT converges with the server) + anchor-fallback/suffix minors — all fixed. Convergence verified safe (sync-apply is pure Loro merge, no double-roll). 29/29 tests. iOS parser also synced with the biweekly/quarterly/every-other Rust additions.
  - Deferred: a fully-atomic multi-key engine op (the roll is 4-5 non-atomic ops; status-first order + tight window make the crash edge rare + recoverable).

## Blockers
- None.

## Open questions / next pick
- Next track is Taylor's pick. Strong candidates: **#73 desktop deploy** (his env), **3B block-merge-on-Backspace** (editor), **type-system/kanban depth**, or the web in-place recurring-roll toast (server already broadcasts `RecurringRolled`). iOS editor track, sync-durability arc, sync-trust hardening, and recurring auto-roll are all complete.
