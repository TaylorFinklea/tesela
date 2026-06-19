# Sync durability — make the relay push/pull rock-solid (2026-06-18)

**Trigger:** Taylor added a block on iPhone; it did NOT reach the relay for ~2 hours (until he relaunched the app). iPad/desktop couldn't pull what was never pushed. Bar: "rock solid before everyday trust." Taylor chose **go big** (the full stack, phased).

## Root cause (audit `woaywen5i`)

**Foreground-only push.** A capture is durably stored on-device immediately (SQLite + materialized file — never lost). But the push to the relay rides the `RelayTicker` loop, which **only runs while the app is foregrounded**. On background, `scenePhase → .background` called `relayTicker.stop()`, cancelling the loop. If the immediate best-effort push in `recordAndPush` (RelayTicker.swift ~402-412) hadn't completed (coordinator still building / network blip), the op sat in the **in-memory outbound queue, stranded, until next launch**. No background task, no flush-before-suspend, no BGTask, no APNs. (The immediate push at line 408 already `await`s when the coordinator is ready — so flush-on-write is largely already there; the gap is background.)

Separately fixed same day (`fix(sync)…`, committed): the **desktop /g live-update** bug — the relay tick applied a remote edit but never re-broadcast the binary Loro delta to web clients (post-apply `export_doc_update` returned None); now `sync_relay::TickOutcome.applied_updates` carries the applied bytes and the daemon re-broadcasts them. Desktop-only; needs a desktop rebuild+swap to deploy.

## Plan (phased)

- [x] **P1 — flush-on-background (CHEAP, the direct fix).** `RelayTicker.flushOnBackground()`: stop the loop, then run a final `flushPendingOutbound()` inside a `UIApplication` background task (~30s) so a just-made capture reaches the relay before iOS suspends. Wired into both shells' `scenePhase → .background`. Shipped (TestFlight build — see git log). Covers the "captured, backgrounded too fast" case when the network is up.
- [ ] **P2a — BGProcessingTask periodic catch-up (MEDIUM).** Register a `BGProcessingTask` (Info.plist `BGTaskSchedulerPermittedIdentifiers` + `UIBackgroundModes: processing`; register handler + schedule in the App struct). On wake (iOS-scheduled, ~15m+ when conditions allow), run `flushPendingOutbound()` + an inbound `tickOnce()`, then reschedule. Drains the long-background tail (network was down at background time, or app backgrounded > the ~30s P1 window). iOS may defer under poor battery/state — best-effort, but guarantees periodic passes.
- [ ] **P2b — Background `URLSession` for the relay PUT (MEDIUM).** Configure a background `URLSession` (mirror the `TranscriptionStore` pattern) so the relay PUT survives app suspension — the system finishes the upload and wakes the app on completion. Makes the push resilient to sub-30s backgrounding + flaky networks. Route `RelayClient`/`coordinator.tickOutbound`'s HTTP through it.
- **P3 — APNs silent-push (BIG, the endgame).** The relay sends a `content-available` silent push when a new batch lands for a group → recipient devices wake + catch up → sub-second cross-device sync without reopening the source app. Phased:
  - [x] **P3a — iOS receiver** (build TBD; `AppDelegate` via `@UIApplicationDelegateAdaptor`): registers for remote notifications, captures the APNs device token (hex, `AppDelegate.deviceTokenHex`), and on a silent push runs `RelayTicker.runBackgroundCatchup()`. `UIBackgroundModes += remote-notification`. Built via a 4-way pi head-to-head (minimax won; all 4 build-verified). **Not functional end-to-end yet** — see deps below.
  - [ ] **P3b — relay device-token registry.** iOS POSTs `deviceTokenHex` (+ group id) to a new relay endpoint that stores tokens per group. Both relay impls: the HA add-on (Rust `tesela-relay`) AND the CF Worker (`cloudflare-relay/`). iOS side: a small client that POSTs the token once it's captured (wire `AppDelegate.deviceTokenHex` → a relay call).
  - [ ] **P3c — relay APNs send.** On `put_envelope` (a new batch deposited for a group), send a `content-available` APNs push to that group's other devices' tokens. Rust APNs client (HTTP/2 to `api.push.apple.com`, JWT signed with the APNs key) in `tesela-relay`; the CF Worker uses the same APNs HTTP/2 API.
  - **⚠ TAYLOR DEPENDENCIES (gate P3 end-to-end, none block the code):** (1) **Enable Push Notifications** for App ID `app.tesela.ios` in the Apple Developer portal + add the `aps-environment` entitlement to the iOS target (P3a registration fails at runtime until then — the code builds regardless). (2) Provide an **APNs auth key** (`.p8` + key id + team id) for the relay to sign pushes (P3c). (3) Decide HA-relay vs CF-Worker first (CF Worker is the canonical prod spine per the roadmap).

## Pull cadence (audit, for reference)

- iOS: 2s base poll while foreground, exponential backoff on errors, **stops entirely on background** (same scenePhase gate). P2/P3 address the background pull too.
- Desktop embed: relay tick every 5s while the app is alive (only when `TESELA_EMBED_RELAY_URL` set; default `TESELA_DISABLE_RELAY=1`). Runs continuously — fine.
- LAN/mDNS P2P data plane is RETIRED (returns 501); the relay is the spine. "Same network" doesn't change the relay path.

## Verify

- P1: capture a block → immediately background the app (don't relaunch the source) → on another device, the block appears within the pull cadence (no source relaunch needed). On-device: the outbound queue drains (RelayTicker status / `last_error` clean).
- P2/P3: longer-background + cross-device-while-source-suspended scenarios.
