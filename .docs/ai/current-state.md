# Current State

## 2026-05-31 (PM) — First convergence fix REGRESSED the live path; delivery-layer redesign in progress

**The device test FAILED.** The engine-convergence fix (below) was correct at the engine level but the LIVE DELIVERY layer regressed: iOS froze, web edits didn't show on iOS, edits reverted. Engine tests proved convergence-given-a-base but never drove the live path — my miss.

**Evidence-based re-diagnosis (not theorized):**
- **Partial WS deltas can't bootstrap a base-less device** — `crates/tesela-sync/tests/partial_delta_needs_base.rs` (PASSES): `export_doc_update(note, Some(pre_vv))` into an empty doc → renders "" (pending). The first fix bootstrapped on the WRITE path, so a receive-only device never got the base.
- **iOS display = HTTP refresh, not the engine.** Inbound WS events only TRIGGER `applyRemoteChange()` → full-note `refresh()` + `refreshLoadedPages()`. So each inbound edit = a full-note re-fetch; a burst → refresh storm → freeze → refreshes never visibly land = "never updated".
- **`RUST_LOG=info` made it worse** — Loro logged ~10 lines per snapshot export, making server snapshot responses ~500ms (vs 12ms at `loro=warn`). FIXED: server restarted `RUST_LOG="info,loro=warn,loro_internal=warn"`.
- Server data ended intact (no dup bids) — revert was live LWW ping-pong + frozen UI, not disk corruption.

**Corrected plan: `phases/2026-05-31-multidevice-delivery-redesign-spec.md`. SIM self-QA gate held** (user approved "fix it properly now, sim-verified").
- [x] **T1 #151** server request logging (`TraceLayer`, `dad4cc2`).
- [x] **T2 #152** iOS bootstrap-on-OPEN via `mosaic.onNoteOpened` (`53afae6`).
- [x] **T4 #153** iOS refresh coalescing — 300ms debounce in `MockMosaicService.scheduleRemoteRefresh` (`29a5528`). **SIM-VERIFIED:** 5-edit burst → exactly 1 sim refresh (was a storm), responsive, web→sim works.
- [x] **T6 #155** ⭐ **THE DOMINANT BUG was WEB-side** (`68d64f3`): web used `invalidateQueries(["notes"])` — a PREFIX match → every mounted notes query refetched per save-echo (~14/edit storm), and the stale daily-list reseeded the editor body → "web edits clear on refresh". Fix: `web/src/lib/ws-refresh-coordinator.ts` (300ms coalesce + 1.5s own-echo suppression) + `api-client` records local saves + `+layout` routes through it. **PLAYWRIGHT-VERIFIED:** 5-edit burst → 1 coalesced refetch pass (~7 reqs, was ~35-70); a typed web edit PERSISTED across a full reload, NO revert. svelte-check clean, 146 unit tests pass.
- [ ] **T3 #150** iOS DELTAS-not-snapshots (real `sinceVv`) — DEFERRED: the iOS storm was already killed by T4 coalescing; deltas are now just a frame-size/perf nicety, not correctness.
- [ ] **T5 #154 / device round-trip (USER)** — the dominant fixes are verified in-hand (web persist/no-revert + sim no-storm). Remaining: full multi-device (web + iPhone + iPad) concurrent test on real devices.

**⚠ Roshar still runs the MORNING build** (engine fix only — NO T2/T4 coalescing). Before a valid multi-device device test, rebuild+reinstall Roshar (clean sandbox) with the T2/T4 build, and clean-install Sel (was paired-not-connected). **Web is already fixed + live** (Vite HMR'd `68d64f3`) — the user can test web editing now by reloading `localhost:5173/g`.

**Current safe state:** server fixed-binary live + quiet + request-logging (`/tmp/tesela-server-fix.log`); web fix live + verified; engine fix (dedup/heal/bootstrap/relay-gate) committed. Web editing should no longer revert.

---

## 2026-05-31 (AM) — [SUPERSEDED claim] Multi-device REVERT engine-fix (correct at engine level; live path regressed — see PM section)

**Symptom (user):** with iPhone (Roshar) + iPad (Sel) both open, web edits stopped persisting — refresh "cleared them away". Worked a while, then broke.

**Root cause (CONFIRMED by deterministic repro):** Loro tree node identity is the internal `TreeID` (peer+counter), NOT our `block_id`/bid. The Mac seeds note docs from disk; iOS `recordNoteDiff` re-authors blocks from its OWN markdown into a doc that **never imported the server's doc as a base** → `BlockUpsert` mints a NEW TreeID per bid under the iOS peer → same bid = two TreeIDs. iOS shipped a full snapshot every keystroke; server imported it and Loro **unioned** the twins. `note_tree_from_doc` rendered both (no dedup); the next web block-diff save updated only ONE twin (FxHashMap scan order = nondeterministic), leaving a stale ghost = "revert". Self-heal quirk: a title/frontmatter edit takes the NoteUpsert reseed path → "works for a while". The disabled-on-Mac relay didn't help because the DEVICES (cached pairing code, shared engine handle) were the injection vector.

**Fix (user chose FULL — heal + converge). Spec: `phases/2026-05-31-multidevice-converge-spec.md`.** Built subagent-driven, two-stage review each, repro test red→green.
- **E1 dedup-by-bid** (`5b05306`,`d1d7b49`): `dedup_twins_by_block_id` (deterministic **min-TreeID** — loro 1.12 exposes no per-text recency, so it's a LOSSY heal, NOT recency-aware) wired into `note_tree_from_doc` (render) + `tombstone_duplicate_twins` in `import_doc_update` (heals on-disk corruption). Repro reframed into T-heal (deterministic non-dup) + T-converge (shared-base correct text).
- **E2 relay gate + B WS cap** (`cc48174`,`09cbb63`): `RelayTicker.hubMode` (gates `tickOnce`/`recordAndPush` coordinator, `dropCoordinator()` on set, cache NOT cleared → reversible); set in BOTH shells under `.http`. `SyncState` `task.maximumMessageSize=64 MiB` (was silently dropping >1 MiB snapshot frames).
- **D shared-base bootstrap** (`b3b5eef`,`979b2ff`,`2f1b729`,`f381e14`): `GET /loro/notes/{id}/snapshot` (mirrors get_loro_index) → FFI `import_note_snapshot` (+regen bindings, +rebuilt device/sim `.a`) → iOS `bootstrapNoteIfNeeded(slug:)` imports the server doc **before first author** (gated on `noteVersion!=nil`, best-effort). Then `recordNoteDiff`'s BlockUpserts resolve to the EXISTING server nodes → true convergence. Reviewer verified the slug/path match makes this hold.
- **Skipped C** (deterministic TreeID-from-bid): loro 1.12 forbids caller-chosen TreeIDs (`pub(crate)`).

**Verified (code):** `cargo test -p tesela-sync` 110 green (incl. T-heal+T-converge), `-p tesela-server` 29 green (incl. `snapshot_bootstrap_converge`); `xcodebuild` (device) → BUILD SUCCEEDED.

**LIVE NOW:** server REBUILT + RESTARTED on the fixed binary — `target/debug/tesela-server --mosaic "<real logseq mosaic>"` (RUST_LOG=info, log `/tmp/tesela-server-fix.log`); `/loro/notes/2026-05-31/snapshot` → 200 (26 KB). Roshar **uninstalled→reinstalled fresh (clean sandbox) + launched** on the fixed Graphite build (flag flip reverted, tree clean).

**PENDING — USER device round-trip (their step; the clean-sandbox part matters):**
- Roshar ready. **Sel (iPad) is paired-but-NOT-connected** — must be connected, then Claude builds+installs there too (clean install). Phone backend serverURL must be `http://100.112.34.59:7474` (Mac Tailscale IP), HTTP mode.
- **CLEAN SANDBOX REQUIRED:** bootstrap SKIPS already-resident docs, so a device still holding PRE-FIX disjoint docs won't re-base them (only the lossy E1 server tombstone heals those). That's why Roshar was uninstalled first; do the same for Sel.
- Test: edit on Mac web `/g` → Roshar <1s; edit on Roshar → web <1s; concurrent edits on web+iPad+iPhone on the SAME note → converge, **no duplicated bullets, no revert**.

**Follow-ups:** #150 iOS snapshot→delta (real `sinceVv` now base is shared) + relay re-export-snapshot + latency; `flushPendingOutbound` not hub-gated (0 callers — guard if ever wired); slug not percent-encoded in iOS `endpoint()` (codebase-wide latent, slugs URL-safe by convention); `cargo install -p tesela-server` to refresh the PATH binary (currently running the debug build from `target/`).

---

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

## 2026-06-02 — REAL root cause found: iOS `preferLocalIfNewer` wholesale-body pick defeats block merge

Long live-debug session. Earlier theories (disjoint history, web refetch storm) were REAL but not the whole story. The dominant "new block reverts / iOS never updates" mechanism is **`MockMosaicService.preferLocalIfNewer` (app/Tesela-iOS/Sources/Data/MockMosaicService.swift:1555, called :801 + :938)**: on every HTTP refresh iOS compares its local sandbox `<id>.md` file mtime vs the server's `modified_at`; if local is strictly newer it **discards the server body wholesale and keeps/re-renders the LOCAL body**. That LWW-between-whole-documents defeats block-level CRDT merge:
- Web adds a new block → server has it. iOS's local daily is mtime-newer (it materialized today on bootstrap) → iOS keeps its local body which LACKS the web block → "iOS never updates" + can re-push the stale view.
- Existing-block TEXT edits survive because they arrive as block-level relay ops that merge into the same bid (don't depend on whole-body pick) → "edits show in 1s". This is the asymmetry the user reported.
Added `emit_deletes:false` (notes.rs:956 / diff.rs:81) means the SERVER never deletes the new block — confirmed: a server-side new block survives 24s+ with the phone connected-but-idle. So the loss is CLIENT-side reconciliation, not server deletion.

**Verified live this session:** server PID from worktree `.worktrees/sync-live-debug` (log `/tmp/tesela-server-sync-live-debug.log` — NOT the stale `/tmp/tesela-srv.log`/`tesela-server-fix.log`). Roshar reconnects + bootstraps `/loro/notes/<today>/snapshot` + `/ws 101` on relaunch but goes idle when backgrounded (date rolled to 2026-06-02). Could NOT force the full revert live (phone idle), but traced the mechanism in code + saw `emit_deletes:false` survive a body-omitting PUT during cleanup. Data safe (2026-05-31 = 15960 bytes intact).

**FIX DIRECTION (not yet built):** `preferLocalIfNewer` is the wrong primitive in a CRDT world — iOS should render the MERGED engine (Loro) doc, not pick between whole `.md` bodies by mtime. The legitimate case it protects (offline iOS edit not yet shipped → don't let a stale server refresh clobber it) must be preserved by MERGING (engine holds both sides), not by wholesale local-wins. Surgical option: when local is newer, flush pending engine ops + import server delta + render engine state, instead of returning the local file body. Deeper: switch the iOS display path from HTTP-body to engine-rendered (the delivery-redesign spec already flags HTTP-vs-engine split). Verify on a STABLE connected device (Roshar tends to idle; keep it foregrounded, watch the sub-second GET cadence in the live log before testing). #150 (iOS deltas-not-snapshots) is still pending and related.

## 2026-06-02 (later) — T7 deployed to all 3 devices; concurrent-edit CLOBBER is the last bug

T7 (engine-render + catch-up-on-open) built, reviewed (PASS), deployed to web + Roshar(iPhone) + Sel(iPad), all on the live server (worktree `.worktrees/sync-live-debug`, log `/tmp/tesela-server-sync-live-debug.log`, Mac IP 100.112.34.59 baked into device builds). **Live 3-device test results:**
- Tests 1-4 (sequential web/Roshar/Sel new blocks + web refresh) **WORK** — live propagation + no revert. T7 confirmed good.
- **Test 5 (CONCURRENT same-note edits) = REAL DATA LOSS, web-side.** User observed: web (stale) "came along and clobbered" Roshar+Sel's concurrent edits when web next saved. Server itself converges CORRECTLY (saw `web one`/`Roshar two`/`Sel three ...` all present) — the loss is a stale client OVERWRITING via whole-body write, not server corruption.

**Root mechanism (confirmed in code):** web saves via `PUT /notes/{id}` with FULL markdown `{content}` (api-client.ts:74, JournalView.flushSave:331). Server `update_note`→`record_sync_update` diffs that whole body and emits a `BlockUpsert` for EVERY block in it — including blocks web didn't edit but holds STALE (because peers edited them while web was out of sync). `emit_deletes:false` stops deletions but NOT stale-text re-assert. So a stale whole-document write stamps its view over concurrent edits. Same root disease as the iOS `preferLocalIfNewer` (T7) bug, on the WRITE side. No block-upsert server endpoint exists yet (only recur-bump/set-property/delete_block); `diff_note_trees`→BlockUpsert/Move/Delete machinery does.

**Fix chosen (user, ultracode):** BLOCK-GRANULAR WRITES — clients send only the block(s) actually edited, not the whole note body/snapshot. Kills the whole "stale whole-doc write clobbers concurrent edits" class. Web editor is per-block CodeMirror (knows the changed block); iOS recordNoteDiff already diffs but produceDeltaFrame ships full snapshot (#150). Investigating all 3 write surfaces + deterministic repro before spec.

## 2026-06-02 (eve) — Block-granular writes S0-S2 LANDED + LIVE-VERIFIED: concurrent clobber FIXED

Spec `phases/2026-06-02-block-granular-writes-spec.md`. The last data-loss bug (concurrent same-note edits: a stale client's whole-body write re-asserts blocks it didn't edit → clobbers a peer) is FIXED at the root via block-granular writes. Deterministic repro `crates/tesela-sync/tests/concurrent_whole_body_clobber.rs` (one test documents the bug on the legacy diff, one proves the granular fix).
- **S0 #158** (`86c60c9`): server `POST /notes/{id}/blocks` (upsert_blocks) — applies ONLY the submitted ops via record_local, bypassing the whole-body diff; reuses update_note's full WS fan-out (pre_vv→export_doc_update(Some)→ws_delta_tx + NoteUpdated + reindex + ensure_tag_pages). Reviewed PASS (note_id space = stable_uuid_from_slug, same doc as PUT; integration test proves both-survive). 30 server tests green.
- **S1 #159** (`523b25d` + debounce fix `a7c3924`): web routes in-place TEXT edits (handleBlockChange→upsert) + INDENT (handleIndent/bulkIndent→move) through POST /blocks, NOT buildFullContent→PUT. One-path-per-save invariant held (block-op edits never also fire a PUT; null-op/structural→PUT fallback). Restored 500ms debounce + AbortController cancel (a7c3924 also closed a type-then-Enter double-send window) + flush-on-blur/note-switch/teardown. 175 web tests green.
- **S2 #160** (`c247914`): web re-settle — own-echo note_updated inside the 1500ms window is now DEFERRED (deferredNoteIds) + flushed as targeted ['note',id] at window expiry, so the editing client re-fetches the converged file instead of staying split. Event-driven timer, terminates, won't reseed mid-typing.

**LIVE-VERIFIED (Claude-driven, web Playwright + API peer + live log):** concurrent web-edits-A / peer-edits-B → BOTH survive on server, BOTH directions; web converges to show peer's edit WITHOUT manual nudge (S2 works); 10 keystrokes → 1 coalesced POST (debounce works); web writes via POST /blocks not PUT; 0 console errors. The exact failure the user hit is gone.

**Server now: main-tree binary (has the block endpoint) on :7474, log /tmp/tesela-srv.log** (replaced the old sync-live-debug worktree build). Web has S1/S2 via HMR. iPad (Sel) on T7 build connected; iPhone (Roshar) taken by user.

REMAINING: S3 #161 (web structural insert/merge → block ops; mid-insert ordering caveat — engine appends at end, do NOT re-canonicalize via stale PUT) → fully kills the root class; S4 #162 (iOS produceDeltaFrame real sinceVv — perf, FFI regen). Follow-up: BlockOutliner.svelte:1952 onOut state_unsafe_mutation warning on block teardown (reactivity hygiene, non-blocking).

## 2026-06-02 (night) — Block-granular writes COMPLETE (S0-S4 + delete path + dropped-frame fix)

All 5 stages of `phases/2026-06-02-block-granular-writes-spec.md` landed + reviewed. The concurrent-edit clobber (the last data-loss bug) is killed at the root: NO web edit except frontmatter/title uses the whole-body PUT anymore.
- **S3 #161** (`44b53ee`): web structural edits (handleEnter/NewBlockAbove/PasteBlock→upsert; handleBackspaceMerge→survivor-upsert+absorbed-delete in ONE POST). Mid-note insert = end-append v1 (loss-free, position imperfect; NO re-canonicalizing PUT). 182 tests.
- **delete path** (`547da7d`): handleBackspace/handleDeleteBlock/deleteVisualBlocks → pure {kind:delete} ops, dropped the whole-body PUT + separate DELETE. Local-only deletes send nothing. 192 tests.
- **S4 #162** (`186b03c`): iOS produceDeltaFrame uses real sinceVv (lastPushedVV) → ships delta not full snapshot. Swift-only (FFI already had since_vv). Rust test: delta smaller + converges.
- **S4 review fix** (`16debef`): review caught a dropped-frame self-heal REGRESSION — in hub mode the WS is the SOLE author→hub path (relay tick gated off, HTTP-PUT removed), so optimistically advancing the VV permanently skipped a dropped frame's ops. Fix: sendDelta returns sent?; lastPushedVV advances (commitPushedDelta) ONLY on confirmed send → dropped frame re-ships next produce. Corrected the false "relay tick is a fallback" doc comment. Build SUCCEEDED.

Final review (S3+delete+S4): PASS on all correctness invariants (no double-write, split/merge atomicity, first-push-full-snapshot, builders match wire). cargo test -p tesela-sync -p tesela-server ALL GREEN; web 192 tests; xcodebuild SUCCEEDED.

**iOS NOT yet rebuilt/reinstalled on devices with S4** (Sel still on T7 build; Roshar with user). Web has S1/S2/S3/delete via HMR; server on main-tree binary (:7474, log /tmp/tesela-srv.log). To device-test S4: rebuild+reinstall Sel.

OPEN FOLLOW-UPS: #163 (BlockOutliner:1952 onOut teardown warning), #164 (template-insert + bulk status/tag-cycle still whole-body PUT — residual clobber on bulk multi-block ops), mid-insert engine ordering (order_key/anchor so peers render mid-inserts in place). #156 (Graphite has no backend Settings UI — device builds bake the Mac IP). NONE are data-loss for the common single-block edit path.
