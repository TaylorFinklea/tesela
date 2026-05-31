# Current State

## 2026-05-30 (PM) — Instant multi-device sync: Phases 0/A/B landed (engine + server + FFI)

**Active milestone: instant multi-device sync (Mac-hub WebSocket over Tailscale).** Approved + red-teamed spec: `phases/2026-05-30-instant-multidevice-spec.md`. Goal: Mac+phone edits appear <1s, conflict-free, relay bypassed (relay/RTC redesign stays deferred beyond this). Subagent-driven, commit per phase, two-stage review each.

- **Phase 0** (`453db61`): lifted `doc_version`/`export_doc_update`/`import_doc_update` onto the `SyncEngine` trait (were concrete-LoroEngine-only — the FFI holds `Arc<dyn SyncEngine>` and couldn't call them) + fixed a latent recursion trap in `apply_relay_updates` + added `trait_level_delta_methods_converge_cursor_free` test. Relay path untouched. `cargo test -p tesela-sync --lib` = 100 passed.
- **Phase B** (`21029b3`): iOS FFI `SyncEngineHandle` gained `produce_note_delta` / `apply_delta_frame` / `note_version` (cursor-free, TLR2-framed `Vec<u8>`↔Swift `Data`). Bindings regenerated (uniffi 0.31 library mode, mirrors c626d25) into `Generated/` + `CFFI/`. FFI tests pass. **Review verified** the catch-up VV concern is NOT a bug (Loro export(updates(&vv)) is correct; the "clamp" idea was a no-op) — the real requirement is bidirectional catch-up, recorded in Phase D.
- **Phase A** (`4fdaf72`): server is now a real-time hub. New `ws_delta_tx: broadcast::Sender<WsDelta>` (separate binary channel from text `ws_tx`), bidirectional `/ws` (forwards binary delta frames + reads inbound → apply → emit `WsEvent::NoteUpdated` for web + re-fan-out), per-conn-id echo-suppression, emit-on-apply across HTTP/WS-inbound/relay-tick origins. `sync_relay::tick` → `TickOutcome{applied, sent, applied_note_ids}`. **Review confirmed** the two load-bearing invariants sound (cursor rule upheld — pre-VV captured before mutation; loop-freedom sound; concurrency correct). Two Important findings both confined to the DORMANT config-bypassed relay path (snapshot-not-bytes re-export; spurious multi-note WsEvents) → deferred to Phase D. `cargo test -p tesela-server` = 26+2 passed, `--workspace` green.

### Phase C — LANDED + review-fixed (`eacc6f4`, `fb31e9c`)
iOS `LiveSyncSocket` now bidirectional + binary-aware: `.data` frames → `onBinaryDelta` → `relayTicker.applyInboundDelta` (engine owner mediates); local write → `recordAndPush` (records) → `relayTicker.produceDeltaFrame` (cursor-free export) → `liveSync.sendDelta(.data)`. Both shells (GrAppShell + AppShell) wired. **Review fix (`fb31e9c`):** both shells' `onAppliedChanges` routed through `mosaic.applyRemoteChange()` (was direct `refresh()` — bypassed the isEditingBlock/suppression guards; Phase C's sub-second delivery made the mid-edit clobber likely). iOS `xcodebuild` → **BUILD SUCCEEDED** (SourceKit cross-file errors are the known no-explicit-modules false positives).

### Server→web hub path PROVEN live (Playwright, 2026-05-31) — TWO ways
1. **API-PUT (simulated remote edit):** opened a 2nd WS, PUT a marker via the API → `sawBinaryFrame:true` (Phase A `ws_delta_tx` binary Loro delta) + `gotNoteUpdated:true` (finding-#4 web-invalidation WsEvent).
2. **REAL editor keystroke (closes the prior self-QA gap):** clicked into today's first journal block, vim `A` + typed " EDIT9X7" through the actual CodeMirror engine, Esc → after the 500ms debounced save: `persistedEdit:true` (block id `019e7a50-4404…` preserved — clean in-place edit, no block churn), **`sawBinaryFrame:true`** + **`sawNoteUpdated:true`** on the genuine keystroke edit. So the full Graphite-editor → engine → WS-hub path works on a real edit.
- **⌘K palette:** opens (`gr-cmdk-input` focused), 47 commands over the real registry; typing "daily" fuzzy-filters to "Today's daily note" with per-char match highlighting + footer hints. **0 console errors** across the whole session. Test marker reverted + reload-confirmed disk clean (`- key` restored, no EDIT9X7 anywhere).
- So the Mac-hub→web direction of the milestone is solid end-to-end; only the phone leg is unverified (device test, Roshar offline at staging).

### DEVICE TEST (the user's step — Roshar was offline at staging time)
Server is live on the Phase-A binary: `*:7474`, `/health` 200, `/ws` upgrade → 101, relay bypassed. Device FFI lib current (post-Phase-B, 18:22). To run the test when Roshar is reconnected:
1. (Claude) build+install the `-graphite`-default app on Roshar — Roshar was `unavailable` via devicectl at staging; reconnect it (USB or on-tailnet+awake) and Claude reruns the build/install (the temp `useGraphiteShell→true` flip + xcodebuild id=00008130-000110592698001C + devicectl install, then revert).
2. On the phone: Settings → backend serverURL = `http://100.112.34.59:7474` (Mac's Tailscale IP), HTTP mode.
3. Test: edit a note on the Mac web (`/g`) → should appear on Roshar in <1s; edit on Roshar → appear on web in <1s; concurrent same-note edits converge, no flashing.

### NEXT — Phase D (drafting now, independent of the device test)
Bidirectional reconnect catch-up (each side exchanges per-note `doc_version` VV; the other exports missing ops via `export_doc_update(note, since_vv)`; a reconnecting device with offline edits must PUSH, not just pull) + the 2 deferred relay-path follow-ups (relay re-export uses snapshot not exact bytes; multi-note batch spurious WsEvents) + latency measurement. All in the spec §5 Phase D / §10.

### (superseded) NEXT — Phase C (needs the user for final verification)
iOS `LiveSyncSocket` (`app/Tesela-iOS/Sources/Sync/SyncState.swift`): dispatch on frame type (.string→existing JSON path; .data→decode Loro delta + apply via Phase B FFI `applyDeltaFrame` + refresh affected note, NOT full re-fetch); on local write produce via `produceNoteDelta` + `send(.data)`; point socket at Mac Tailscale `ws://100.112.34.59:7474/ws`; relay ticker stays idle/bypassed. **Acceptance = live device round-trip on Roshar** (sim shares Mac network, hides reachability — `feedback_ios_test_on_device`): Mac web↔Roshar edits <1s both ways, concurrent same-note edits converge no flashing. I build/install/launch + drive Mac side; the user confirms the phone screen. Then **Phase D** = bidirectional reconnect catch-up + the 2 deferred relay follow-ups + latency measurement.

### Server currently running (relay bypassed)
`TESELA_SERVER_BIND=0.0.0.0:7474 tesela-server --mosaic "<real logseq mosaic>"` — standalone local hub, `[sync.relay]` commented in mosaic config.toml (backup `config.toml.relay-bak`). NOTE: this running instance predates Phases A — needs a restart on the new binary before Phase C device testing so the `/ws` delta path is live.

---

## 2026-05-30 — Graphite on the iPhone; relay 413 fixed-in-code then BYPASSED

**Graphite build is installed on Roshar (iPhone 15 Pro).** Built device SDK (signed, fixed FFI), boots straight into Graphite (temp `useGraphiteShell→true` flip during build, reverted in-repo — shipping default stays AppShell). Sim + device both run the redesign.

**Relay 413 — root-caused, fixed in code, then deferred + bypassed.** Testing on the phone surfaced the real bug behind "edits revert on web + iOS": the Mac's outbound relay PUT 413'd (ai-business 1.3 MB note → ~5 MB Loro snapshot ≈ 7 MB wire > HA relay `max_body`), while inbound polling kept applying stale ops over fresh edits.
- **Fixed in code** (`08e941b`, `0c97b92`): relay binary `--max-body` default 1 MiB→16 MiB; client `MAX_RELAY_PLAINTEXT_BYTES` 2.5 MB→8 MiB under a new `RELAY_MAX_BODY_BYTES`=16 MiB invariant (+2 regression tests); first-broadcast ships a compact `ExportMode::Snapshot` not full deleted-history; HA add-on/compose/DOCS deploy defaults → 16 MiB (add-on 0.1.0→0.1.1).
- **HA-add-on gotcha:** the live relay reads `max_body` from the add-on **Configuration tab** (`/data/options.json` via `run.sh`), NOT any shell env — so the user's env-var restart never changed it, and config defaults don't retro-apply to an existing install.
- **DECISION (Taylor, 2026-05-30):** stop patching this relay; redesign it after Loro/RTC (likely need an RTC server/proxy anyway). **Relay BYPASSED for local testing** — `[sync.relay]` commented out in the Mac mosaic `config.toml` (backup `config.toml.relay-bak`); Mac is a standalone local server. Verified a PUT persists + survives the old poll window + hits disk. No cross-device sync while bypassed (fine for single-device Graphite testing). See decisions.md + [project_relay_413_blocks_sync].

---

## 2026-05-29 — Loro cutover FINISHED; redesign is next

**Loro is the sole sync engine.** Flag-day + ai-business dedup + DR drill all done, committed, green. Full report: `phases/2026-05-29-loro-cutover-report.md`.

### Commits this session
- `8ef366e` perf(sync): dedup — store frontmatter-only on root meta (lean snapshots).
- `471d619` refactor(sync)!: delete SqliteEngine/DualEngine/op-wire — Loro-only (~3.6k lines deleted).
- `c626d25` build(ios): regenerate UniFFI bindings for the Loro-only FFI.

### Build status
- `cargo build --workspace` + `cargo test --workspace` → GREEN (0 failures).
- `xcodebuild -scheme Tesela -sdk iphonesimulator` → **BUILD SUCCEEDED** (against the rebuilt `.a` + regenerated bindings).
- `cargo install` of `tesela-server` + `tesela` (flag-day binaries) — see this session's install.

### What the flag-day did
- Deleted `sqlite_engine.rs`, `dual_engine.rs`, `tests/convergence.rs`, `examples/two_node.rs`.
- `SyncEngine` trait = Loro-only (dropped `apply_changes`/`produce_changes_since`/`produce_local_authored_since`/`uses_loro_relay_payload`/`ProducedBatch`). Deleted the v1 op-wire (`encode/decode_op_batch`).
- Server: `main.rs` builds a bare `LoroEngine` unconditionally (no `TESELA_LORO_DUAL_WRITE`/`AUTHORITATIVE`; `TESELA_LORO_RESEED` kept for one-time canonical bootstrap). `sync_relay.rs` = Loro v2 only. Deleted dual-write divergence endpoints (kept `/loro/index`).
- **LAN P2P (peer_sync) data-plane RETIRED** — op-replay is incompatible with Loro + fully redundant with the relay spine; `produce`/`receive_envelope` → 501, daemon = no-op, pairing/discovery stay live. Follow-up: reimplement over the Loro relay-update protocol.
- FFI: `open_loro` is the sole constructor; ticks = Loro v2 only.

### Server launch (CHANGED — no flags)
`tesela-server --mosaic "/Users/tfinklea/Library/Application Support/tesela/logseq"` — Loro is now the default engine. Add `TESELA_LORO_RESEED=1` ONLY for a one-time canonical bootstrap from disk (one device). (No server is currently running.)

### DR drill (validated on an isolated copy — non-destructive)
Restore from `notes/*.md` + `TESELA_LORO_RESEED=1` rebuilds all 514 notes; `/health` 200, `/loro/index` = 514. **Dedup payoff: ai-business snapshot 5.13 MB → 2.58 MB** (now under the 5 MB relay limit). Canonical DR = the `.md` files are truth; `.tesela/loro/` is a derived cache.

## Blockers / open
- **Live data reset is USER-COORDINATED (needs the iPhone).** The dedup's size win lands only on fresh docs; the live mosaic still holds bloated snapshots, so ai-business won't sync until a coordinated reset: stop server → backup → `rm -rf <mosaic>/.tesela/loro/` → boot with `TESELA_LORO_RESEED=1` → **wipe + re-bootstrap the iPhone's local docs** (else fresh-identity docs duplicate against its old docs). Until then the server runs fine on existing docs via the backward-compat fallback (ai-business simply stays unsynced, as before). See the report's "Remaining" section.
- Backlog (unchanged): deferred review findings #7/#8 (slug-rename orphans), #10–18; #111 oplog-order (moot post-flag-day).

## ACTIVE MILESTONE — Graphite redesign (foundation DONE; shell next)
Approved spec: `phases/2026-05-29-graphite-redesign-spec.md`. Foundation plan: `phases/2026-05-29-graphite-foundation-plan.md`. Brand-new web (SvelteKit) + iOS (SwiftUI) frontends to the Graphite design system, reach daily-driver parity, then delete the old. Phasing: foundation → shell → daily-driver views → cutover → iterate. Web + iOS in parallel; shared tokens; REUSE vetted lib logic (CodeMirror editing engine) + Loro FFI/MosaicService. Design source: `.docs/ai/design/graphite/`.

### Foundation phase — LANDED (2026-05-29)
Executed subagent-driven (web + iOS in parallel). Commits: `7083956` (tokens.json + web primitives), `e316a6f` (iOS theme + SwiftUI primitives).
- **Shared tokens:** `.docs/ai/design/graphite/tokens.json` (canonical). Web: `web/src/lib/graphite/tokens.css` (mockup's exact `--*` vars scoped to `.gr-root`). iOS: `.graphite` Theme case in `Sources/DesignSystem/Theme.swift` (reuses existing `@Environment(\.theme)` infra).
- **Primitives (both platforms):** GrIcon (web=`@tabler/icons-svelte` name-map; iOS=Tabler→SF-Symbol map), GrButton (ghost/cta), GrChip, GrTypeDot, GrTypeTag, GrRow, GrWidget. Web in NEW `web/src/routes/g/` tree + `lib/graphite/` (old v4/v5 untouched). iOS in NEW `Sources/Graphite/` (Data/Sync/Generated/Views/Components untouched). Components reference tokens only (theme-swappable, no hardcoded hex bar the on-coral CTA ink).
- **Gates (re-verified by me):** web `svelte-check` clean for graphite (lone error = pre-existing v4 VoiceCaptureButton); iOS `xcodebuild -sdk iphonesimulator` → **BUILD SUCCEEDED**. (IDE SourceKit shows false-positive cross-file errors under this project's no-explicit-modules config — xcodebuild is authoritative.)
- **One deferred check:** visual parity of the `/g` primitives gallery vs the mockup screenshots — not done (user's Chrome held the MCP profile; build/type gates pass). View at `localhost:<webdev>/g`; iOS gallery via `GrGalleryView` `#Preview`. Confirm at shell-phase start.

### Shell phase — LANDED (2026-05-29)
Plan: `phases/2026-05-29-graphite-shell-plan.md`. Executed subagent-driven (web + iOS parallel). Commits: `88e4dfe` (web shell), `c897b98` (iOS shell). **All NEW Graphite presentation bound to EXISTING behavior — no behavior rebuilt.**
- **Web** (`web/src/lib/graphite/shell/`, composed at `/g`; primitives gallery moved to `/g/primitives`): GrTopBar (workspace tabs via `getWorkspace`/`switchTab` + ⌘K bar→`openStation` + connection dot via `getConnected`), GrRail (widget host: Quick-capture→`openColonMode`, Pinned=`getFavorites`, Today=`getRecents`, Tasks=placeholder, +Add-widget stub), GrPane (first-class, focus/side splits-ready, placeholder body), GrStatus (`getVimMode` + breadcrumb + clock), GrCommandPalette (mirrors `Station.svelte` over the real `buildV4Commands` + `scoreFuzzy`), GrLeaderOverlay (mirrors `ChordMenu` over `getLeaderTree`), GraphiteShell (composes + mirrors `v4/+layout` capture-phase keydown Space/⌘K/`:`).
- **iOS** (`Sources/Graphite/Shell/`): GrAppShell mirrors AppShell's native tab bar (4 tabs + `.search` glass circle) bound to the SAME `MockMosaicService`/`RelayTicker`/`CaptureComposer`/`StreamingVoiceRecorder` (full relay bring-up + scenePhase + voice wiring mirrored), forced `.graphite`. GrHeader, GrCaptureBar+GrCaptureSheet (reuse CaptureComposer + `MosaicService.capture`), GrTabPlaceholder. `#Preview` only — NOT the app entry until cutover (`TeselaApp` unchanged).
- **Gates (re-verified):** web `svelte-check` clean for graphite; iOS `xcodebuild` → BUILD SUCCEEDED. Old UI untouched on both.
- **Deferred check:** visual+interaction QA of `/g` (⌘K palette, Space leader) — user's Chrome held the MCP profile. Behavior is reused/mirrored (low risk); confirm at `localhost:5173/g`.
- **⚠ Cutover note:** the web palette/leader/commands reuse behavior modules under `lib/v4/` (`buildV4Commands`) + `lib/v5/` (`leader-tree`) + `lib/stores/`. At cutover, deleting the v4/v5 *UI routes/components* must NOT delete these reused *behavior* modules — separate them (move the reused logic out of the to-delete tree) first.

### Daily-driver views — LANDED + self-QA'd (2026-05-29)
Plan: `phases/2026-05-29-graphite-views-plan.md`. Commits: `6a8cbc3` (web views), `84a4dc0` (iOS views), `562b192` (iOS shell toggle). (A mid-run disk-full crash happened; recovered after the user freed space — both parts then completed + gated.) **Editing engines + data layer 100% reused; only presentation new/re-themed.**
- **Web** (`lib/graphite/`): Graphite CodeMirror theme + decoration CSS (`editor/`) re-skins the REUSED BlockOutliner/JournalView under `.gr-root`; `GrDaily` (wraps JournalView), `GrPage` (BlockOutliner + linked-refs + props, mirrors BufferShell fetch/save), `GrInbox` (chips + cards over `executeQuery`), `GrAgenda` (week grid over `getAgenda`); `GraphiteShell` routes the focused buffer by kind (daily/page/inbox/agenda).
- **iOS** (`Sources/Graphite/Views/`): `GrDailyView`/`GrPageView`/`GrLibraryView`/`GrAgendaView`/`GrInboxView` bind the shared `MockMosaicService` + reuse `BlockRow` untouched; render in GrAppShell's tabs (`.search` = native). `TeselaApp` gained a `-graphite` launch-arg / `tesela.useGraphiteShell` toggle (default = shipping AppShell).
- **Self-QA — web (Playwright, live backend on :7474, real mosaic):** `/g` renders the Graphite shell with REAL data — 9 live reused CodeMirror editors showing actual daily blocks, topbar/rail(4 widgets)/status="NORMAL"; **⌘K opens the palette with 39 real commands (Jump-to/Actions); Space opens the leader with the 6 real `getLeaderTree` chords; 0 console errors.** (Synthetic-keypress typing didn't engage CM through the harness — a Playwright/CM focus quirk, not an app bug; the editor is the byte-identical reused engine.)
- **Self-QA — iOS (simulator):** built for the booted `Tesela-Test` sim, installed, launched with `-graphite` → the Graphite **Today** journal renders (tasks/checkboxes, `#tags`, `[[wiki-link]]`, liquid-glass capture bar, Daily·Agenda·Inbox·Library tab bar + search circle) on seed data. (`simctl` can't tap-test the other tabs; they compiled + are wired — user taps to explore. Real data needs device pairing.)

### Adversarial review + fixes (2026-05-29, `0a6cc30`)
25-agent review (find → per-finding verify) of the whole redesign → 5 confirmed real (rest false positives, several mirroring existing v4 patterns). Verify stage notably caught that a proposed "rollback on save error" fix would itself cause data loss. Fixed: GrPage save failures now surfaced (toast + save-state, keep optimistic content, NO rollback); GrInbox.processAll counts+toasts batch failures (caught applyTriage's `false` stale-block return too); GrAppShell wired the missing **LiveSyncSocket** (instant Mac→app WS push, was ~2s poll) + corrected its overclaiming docstring. **Deferred to cutover:** GrAppShell `MosaicRegistry`/profile-switching (single-profile is fine for testing). Both gates green.

### Running services (I own these; for the user's testing)
- **Backend:** `tesela-server --mosaic "/Users/tfinklea/Library/Application Support/tesela/logseq"` on `127.0.0.1:7474` (flag-free; NO reseed — the live data reset stays deferred). Log: `/tmp/tesela-graphite-test.log`.
- **Web dev:** Vite serving the Graphite app at **http://localhost:5173/g** (proxies `/api`→7474). The user tests the daily flow here.

### NEXT — toward cutover
- **Web parity polish:** JournalView's own day headers render (not the `.gr-dayhdr` markup) — fine functionally; pixel-match later. Confirm editing/save round-trip by typing (reused engine; harness couldn't drive CM). Inbox/agenda are functional over the data layer; refine actions.
- **iOS:** wire GrAppShell's full real-data bring-up to parity with AppShell (onboarding/pairing/registry) so `-graphite` shows live data on a fresh device; tap-test all tabs.
- **Cutover** (when parity holds): delete old v4/v5 web + old iOS Views; make GrAppShell the sole entry; **preserve the reused `lib/v4`+`lib/v5` behavior modules** (move them out of the to-delete UI tree first).
