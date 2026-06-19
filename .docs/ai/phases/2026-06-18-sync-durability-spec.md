# Sync durability — make the relay push/pull rock-solid (2026-06-18)

**Trigger:** Taylor added a block on iPhone; it did NOT reach the relay for ~2 hours (until he relaunched the app). iPad/desktop couldn't pull what was never pushed. Bar: "rock solid before everyday trust." Taylor chose **go big** (the full stack, phased).

## Root cause (audit `woaywen5i`)

**Foreground-only push.** A capture is durably stored on-device immediately (SQLite + materialized file — never lost). But the push to the relay rides the `RelayTicker` loop, which **only runs while the app is foregrounded**. On background, `scenePhase → .background` called `relayTicker.stop()`, cancelling the loop. If the immediate best-effort push in `recordAndPush` (RelayTicker.swift ~402-412) hadn't completed (coordinator still building / network blip), the op sat in the **in-memory outbound queue, stranded, until next launch**. No background task, no flush-before-suspend, no BGTask, no APNs. (The immediate push at line 408 already `await`s when the coordinator is ready — so flush-on-write is largely already there; the gap is background.)

Separately fixed same day (`fix(sync)…`, committed): the **desktop /g live-update** bug — the relay tick applied a remote edit but never re-broadcast the binary Loro delta to web clients (post-apply `export_doc_update` returned None); now `sync_relay::TickOutcome.applied_updates` carries the applied bytes and the daemon re-broadcasts them. Desktop-only; needs a desktop rebuild+swap to deploy.

## Plan (phased)

- [x] **P1 — flush-on-background (CHEAP, the direct fix).** `RelayTicker.flushOnBackground()`: stop the loop, then run a final `flushPendingOutbound()` inside a `UIApplication` background task (~30s) so a just-made capture reaches the relay before iOS suspends. Wired into both shells' `scenePhase → .background`. Shipped (TestFlight build — see git log). Covers the "captured, backgrounded too fast" case when the network is up.
- [ ] **P2a — BGProcessingTask periodic catch-up (MEDIUM).** Register a `BGProcessingTask` (Info.plist `BGTaskSchedulerPermittedIdentifiers` + `UIBackgroundModes: processing`; register handler + schedule in the App struct). On wake (iOS-scheduled, ~15m+ when conditions allow), run `flushPendingOutbound()` + an inbound `tickOnce()`, then reschedule. Drains the long-background tail (network was down at background time, or app backgrounded > the ~30s P1 window). iOS may defer under poor battery/state — best-effort, but guarantees periodic passes.
- [ ] **P2b — Background `URLSession` for the relay PUT (MEDIUM).** Configure a background `URLSession` (mirror the `TranscriptionStore` pattern) so the relay PUT survives app suspension — the system finishes the upload and wakes the app on completion. Makes the push resilient to sub-30s backgrounding + flaky networks. Route `RelayClient`/`coordinator.tickOutbound`'s HTTP through it.
- [ ] **P3 — APNs silent-push (BIG, the endgame; needs Taylor + infra).** The relay sends a `content-available` silent push whenever it receives a new outbound batch for a group → recipient devices wake and `flushPendingOutbound()` (pull) → sub-second cross-device sync without anyone reopening the source app. Components: (1) iOS registers for remote notifications + a device-token → relay registry; (2) the relay (HA add-on + CF Worker) stores tokens per group and sends APNs on deposit; (3) an APNs auth key (Taylor — same ASC team key family or a dedicated APNs key) + the relay's APNs client. This is the "added it on my phone, it's instantly everywhere" guarantee.

## Pull cadence (audit, for reference)

- iOS: 2s base poll while foreground, exponential backoff on errors, **stops entirely on background** (same scenePhase gate). P2/P3 address the background pull too.
- Desktop embed: relay tick every 5s while the app is alive (only when `TESELA_EMBED_RELAY_URL` set; default `TESELA_DISABLE_RELAY=1`). Runs continuously — fine.
- LAN/mDNS P2P data plane is RETIRED (returns 501); the relay is the spine. "Same network" doesn't change the relay path.

## Verify

- P1: capture a block → immediately background the app (don't relaunch the source) → on another device, the block appears within the pull cadence (no source relaunch needed). On-device: the outbound queue drains (RelayTicker status / `last_error` clean).
- P2/P3: longer-background + cross-device-while-source-suspended scenarios.
