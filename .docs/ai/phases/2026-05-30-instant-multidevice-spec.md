# Instant Multi-Device Sync (Mac-hub over Tailscale) — Spec

**Status:** DRAFT (red-teamed — see §10) — awaiting Taylor's approval before execution.
**Author:** Claude (Opus), 2026-05-30.
**Milestone chosen by Taylor:** "Instant multi-device (your devices)" — your Mac + phone: your own
edits appear near-instantly and merge conflict-free, no 5 s lag. Topology: **Mac = hub, devices = WS
clients over the Tailscale tailnet.** (RTC targets that nest above this — true concurrent co-edit,
presence, Savanne-as-person — are explicitly OUT of scope here; this is their foundation.)

---

## 1. Why this, why now

The Loro CRDT core is engineering-complete and convergence is test-proven
(`loro_engine.rs:2707` two/three engines converge on concurrent same-note edits, no flashing). What
is NOT real-time is the **transport**:

- The WAN relay is a **poll-based store-and-forward FIFO** with no push (`tesela-relay/main.rs:26`
  documents this explicitly). Edit→peer floor = the poll interval (5 s default). [grounded]
- A live WS exists (`/ws`, `routes/ws.rs`) but carries **whole-`Note` JSON** and triggers a **full
  HTTP re-fetch**; it's **LAN-only** and never crosses to remote devices; it never carries Loro
  bytes. [grounded: `state.rs:58-96`, `SyncState.swift:123-140`]
- Two clients writing to one server both do whole-note REST PUT → **last-write-wins**; Loro's merge
  only happens *between separate engines exchanging deltas*. [grounded]

Your devices are always on the Tailscale tailnet (Mac + phone, incl. cellular). A direct Mac-hub WS
over `100.x` gives "works everywhere for your own devices" reach **without touching the relay**. The
relay stays bypassed (retained HA infra) — it's the offline / off-tailnet / collaborator fallback, a
LATER milestone. This milestone sidesteps the relay redesign entirely.

---

## 2. Architecture decision (made here; approve or redirect)

**Decision: phone-as-peer-engine, not phone-as-thin-client.** Both Mac and phone keep their own
`LoroEngine`; they exchange **Loro delta bytes** over a bidirectional WS with the Mac as the hub. The
phone does NOT revert to a dumb HTTP client.

**Why:**
- The cutover already made the phone run a real `LoroEngine` (`open_loro`). Demoting it to a thin
  HTTP client throws that away and breaks offline-first.
- True conflict-free Mac↔phone merge *requires* both sides to be engines exchanging deltas — that's
  the entire point of the Loro migration.
- It generalizes: the same WS-delta protocol is exactly what a future web-wasm engine and a future
  relay-push channel would carry. Build the protocol once.

**The transport substitution:** today the phone's `RelayTicker` drives `SyncCoordinator.tick_outbound`
/`tick_inbound` (FFI), which speak the relay's HTTP poll API. For this milestone, the phone instead
exchanges the SAME `LoroDocUpdate` payload over a **push WS to the Mac**. The relay HTTP path is left
intact in code (bypassed via config), so reverting is just config.

**Reuse boundary (load-bearing — mirrors the cutover + Graphite reuse discipline):**
- REUSE unchanged: `apply_relay_updates` (already on the `SyncEngine` trait, `engine/mod.rs:113`),
  `import_doc_update`, the convergence-test pattern (`loro_engine.rs:2620-2766`), the per-note Loro
  doc model.
- REUSE the wire type `wire::LoroDocUpdate` and `encode/decode_loro_relay_payload` (`TLR2` framing) —
  the WS carries the SAME bytes the relay does, so the engine merge code path is identical.
- REUSE the iOS `LiveSyncSocket` connection/reconnect/generation machinery (`SyncState.swift`); only
  its message handling + a new send path change.
- NEW (red-team-corrected — see §10): (a) **trait additions** — `export_doc_update` /
  `import_doc_update` move onto the `SyncEngine` trait (today they're concrete-`LoroEngine`-only, so
  the `Arc<dyn SyncEngine>` FFI handle can't call them); (b) a **cursor-free** delta export for the
  live path so it never contends with the relay's `broadcast_cursor`; (c) a **separate binary
  broadcast channel** for Loro deltas (the existing `broadcast::Sender<WsEvent>` is text-JSON-only);
  (d) a `WsEvent` emit on the relay/WS *apply* path (today only the HTTP-edit path emits, so
  remote-originated edits never notify web); (e) the Swift FFI seam + bidirectional wiring.

**The cursor rule (load-bearing):** `produce_relay_updates` + `commit_broadcast_cursors` +
`broadcast_cursor` stay EXCLUSIVELY the relay path's (they encode "what has the relay shipped"). The
live WS path must NOT call `produce_relay_updates` — it would race the relay over the shared cursor
(relay commits first → the WS delta silently never ships; red-team finding #3). The live path instead
**forwards the exact delta bytes it just applied** (origin device → hub → other devices), which Loro
merges idempotently and which is inherently loop-free with origin-socket echo-suppression (finding
#4). The hub does NOT re-`produce` a delta after applying; it relays the bytes it received.

---

## 3. What exists vs. what's new (grounded inventory)

| Piece | State today | This milestone |
|---|---|---|
| `LoroEngine` merge core (`apply_relay_updates`, `import_doc_update`, convergence tests) | DONE, tested (`loro_engine.rs:399-483`, `:2620-2766`) | reuse unchanged |
| `export_doc_update`/`import_doc_update` on the **trait** | NOT on trait — concrete `LoroEngine` only (`engine/mod.rs:43-141` lacks them) | **MOVE onto `SyncEngine` trait** so the `Arc<dyn SyncEngine>` FFI handle can call them (finding #1) |
| Cursor-free live-delta export | does not exist; `produce_relay_updates` consumes `broadcast_cursor` (`loro_engine.rs:418-450`) | **ADD** `export_doc_update(note, since_vv)`-based live export that does NOT touch `broadcast_cursor` (finding #3) |
| `wire::LoroDocUpdate` + `TLR2` encode/decode | DONE (`wire/mod.rs:11-63`) | reuse unchanged |
| Server delta broadcast channel | only `broadcast::Sender<WsEvent>` → Text JSON (`state.rs:17`, `ws.rs:17-19`) | **ADD** a separate binary delta channel (don't overload the text one — finding #2) |
| Server `/ws` handler | forwards text frames only (`routes/ws.rs`) | forward binary delta frames on the new channel; ADD an inbound binary-frame handler (decode→apply→forward) |
| `WsEvent` emit on apply | ONLY the HTTP-edit path emits (`notes.rs:275`); relay/WS apply path emits NOTHING (`sync_relay.rs` returns counts only) | **ADD** a `WsEvent::NoteUpdated` emit after relay/WS-originated apply, so web invalidates for remote edits (finding #4 — the web-as-view fix) |
| iOS `LiveSyncSocket` | receive-only → triggers HTTP re-fetch; UTF-8-decodes ALL frames as JSON (`SyncState.swift:123-140`) | send + receive **binary** Loro deltas; dispatch on frame type; apply via FFI |
| iOS FFI delta surface | NOT exposed — only relay-coupled `tick_outbound/inbound` (`tesela-sync-ffi/src/lib.rs`) | ADD produce-last-change-delta + apply-delta-bytes methods (`Vec<u8>` marshaling) |
| Web client | view of Mac engine; WS text events invalidate queries (`ws-client.svelte.ts:178` already drops non-text) | unchanged code; now CORRECT because the relay/WS apply path emits `WsEvent` (finding #4). Web-own-engine = the *concurrent co-edit* milestone |

---

## 4. Protocol (the one new wire contract)

A new WS message direction + kind. Keep it minimal and framed so it can't be confused with the
existing JSON `WsEvent`s.

Two WS frame types on `/ws`: **text** = existing JSON `WsEvent` (unchanged; web's invalidation path),
**binary** = `TLR2`-framed `postcard(Vec<LoroDocUpdate>)` Loro delta (no AEAD — tailnet is the trust
boundary, mirroring the LAN-only `/ws` today). The server carries these on a **separate broadcast
channel** (`ws_delta_tx`), NOT by adding a variant to the text-only `Sender<WsEvent>` (finding #2: iOS
UTF-8-decodes every frame as JSON and would silently drop a binary frame mixed onto the same path).

- **Server → client (push):** when the engine applies a note change, the server forwards the **exact
  applied delta bytes** to all *other* connected sockets as a binary frame. It does NOT call
  `produce_relay_updates` (would race the relay cursor — finding #3) and does NOT re-`produce` a
  post-apply delta (would risk re-circulation — finding #4-echo). For an HTTP-originated edit, the
  delta is exported cursor-free from the just-applied note's pre-edit version vector (the new
  `export_doc_update(note, since_vv)` live path). For a WS-originated edit, the delta forwarded is the
  bytes that arrived (Loro merges idempotently).
- **Client → server (push):** the device sends its locally-produced delta as a binary frame. The
  server decodes → `apply_relay_updates` into its engine (idempotent/commutative) → **emits a
  `WsEvent::NoteUpdated`** on the text channel (finding #4: today the apply path emits nothing, so web
  would stay stale) → forwards the delta binary-frame to all sockets *except the origin*
  (echo-suppression by origin-socket id).
- **Cursor/catch-up:** on (re)connect, the client sends its per-note version-vector summary; the
  server replies with the deltas it's missing via `export_doc_update(note, since_vv)` (cursor-free).
  This is the WS path's catch-up; it does not touch the relay's seq cursor. (Detail deferred to Phase
  D; the steady-state push path is Phases A–C.)

**Authentication:** the WS is reachable on the tailnet only; the server already serves `/ws` without
auth on the LAN today. For this milestone we keep parity (tailnet = trust boundary). A WS auth
handshake (group key proof) is noted as a follow-up, NOT in scope.

---

## 5. Phasing (each phase independently testable; commit per phase)

**Phase 0 — Engine trait + cursor-free export (Rust, pure refactor, no behavior change).**
Move `export_doc_update` + `import_doc_update` from concrete `LoroEngine` onto the `SyncEngine` trait
(`engine/mod.rs`), with the `LoroEngine` impl as the override (finding #1). Add a cursor-free live
export — `export_doc_update(note, since_vv)` is already cursor-free; expose a small helper that
captures a note's pre-edit version vector so the HTTP path can export "the delta for the change that
just happened" WITHOUT consulting `broadcast_cursor` (finding #3). Leave `produce_relay_updates` /
`commit_broadcast_cursors` / `broadcast_cursor` exactly as-is (relay-only).
*Test:* `cargo test --workspace` stays green; the existing relay convergence tests still pass
(proves the relay path is untouched). New unit test: cursor-free export of a freshly-edited note
returns a delta that, applied to a second engine, converges — without advancing any broadcast cursor.

**Phase A — Server: bidirectional delta WS + emit-on-apply (Rust, no client changes).**
Add a `ws_delta_tx: broadcast::Sender<Vec<u8>>` (or a small framed struct carrying origin id) to
`AppState`, separate from `ws_tx` (finding #2). On HTTP edit (`notes.rs` apply path): after
`record_local`, export the cursor-free delta (Phase 0) and publish it on `ws_delta_tx`. Upgrade
`routes/ws.rs` to (a) forward binary delta frames to each socket (skipping the frame's origin), and
(b) handle inbound binary frames: decode `TLR2` → `apply_relay_updates` → **emit
`WsEvent::NoteUpdated` on `ws_tx`** (finding #4, the web-as-view fix) → forward the bytes to other
sockets. Echo-suppression: tag each socket with an id; never send a delta back to its origin.
*Test:* Rust integration test, two in-process WS clients on one server engine — client A sends a
delta frame → client B receives it AND a `NoteUpdated` text event fires → assert B's engine
converges and the text-event-driven web-invalidation path would trigger. Add the **3-node hub test**
(finding #4-echo): A and C send concurrent edits to the hub; assert deterministic, finite fan-out and
that neither A nor C receives its own origin frame.

**Phase B — iOS FFI: expose delta produce/apply (depends on Phase 0).**
Add two `#[uniffi::export(async_runtime = "tokio")]` methods on `SyncEngineHandle` (which holds
`Arc<dyn SyncEngine>` — now callable because Phase 0 put the methods on the trait): produce the
cursor-free delta for a note as `Vec<u8>` (`TLR2`-framed), and apply received delta `Vec<u8>`. Confirm
UniFFI `Vec<u8>`↔`Data` marshaling (it's supported; verify no signedness/copy gotcha). Rebuild the
`.a` for both iOS targets + regenerate bindings (mirror the cutover's `c626d25` procedure).
*Test:* `cargo test` on the FFI crate (host bindings) round-trips produce→apply between two handles
and asserts convergence.

**Phase C — iOS client: send + apply binary deltas over the WS (depends on B + A).**
Upgrade `LiveSyncSocket` (`SyncState.swift`): dispatch on frame type — `.string` → existing JSON
event path (re-fetch/notify), `.data` → decode as a Loro delta and apply via the Phase B FFI, then
refresh just the affected note's view (NOT a full HTTP re-fetch). On local write, produce a delta
(Phase B FFI) and `send(.data(...))`. Point the socket at the Mac's Tailscale URL
(`ws://100.x:7474/ws`). Keep the relay ticker present but idle (config-bypassed) so nothing regresses.
*Test (device, per [[feedback-ios-test-on-device]]):* edit on Mac web → appears on Roshar in <1 s;
edit on Roshar → appears on Mac web in <1 s (the finding-#4 direction); concurrent same-note edits on
both converge, no flashing. Sim is insufficient (shares the Mac's network) — verify on the paired
iPhone.

**Phase D — latency + correctness hardening.**
Reconnect catch-up (the cursor/version-vector exchange from §4), echo-suppression verification under
concurrent edits, and a "hub offline" graceful path (fall back to relay if/when re-enabled, or just
queue locally). Measure actual edit→peer latency and record it.

---

## 6. Acceptance (what "done" means for this milestone)

- Editing a note on the Mac web client makes the change appear on the paired iPhone in **< 1 second**
  (not the 5 s poll), and vice-versa, over Tailscale with the relay bypassed.
- Concurrent edits to the *same note* on Mac + phone **converge with no flashing / no lost edits**
  (the Loro guarantee, now delivered live).
- The relay remains untouched/bypassed; re-enabling it (config) does not break the WS path.
- All existing tests green; new convergence tests for the WS delta path green; iOS `xcodebuild`
  SUCCEEDED; device round-trip verified by Taylor.

---

## 7. Open design points to resolve during Phase A/D (not blocking approval)

- **Catch-up protocol shape:** version-vector summary exchange vs. "deltas since seq". VV is more
  Loro-native and handles the multi-note case cleanly; lean VV.
- **Server fan-out unit:** broadcast the exact applied delta, or re-export each recipient's missing
  delta per their VV? Former is simpler and idempotent (Loro dedupes); lean former, with VV catch-up
  only on (re)connect.
- **Web parity:** web stays a query-invalidation view this milestone. If "no lag" should also apply
  to web↔phone (not just Mac-as-renderer), that's the *concurrent co-edit* milestone (web gets a
  loro-wasm engine). Confirm web-as-view is acceptable for the first proof.
- **Multiple devices:** design for N clients from the start (broadcast already fans out); test with 2.

---

## 8. Risks / notes

- **Echo loops:** a delta applied from device A must not be re-sent to A. Server tracks the origin
  socket; engine apply is idempotent so a stray echo is harmless but wasteful. Test explicitly.
- **iOS background:** the existing `suspend()`/`nudge()` scenephase handling is reused; a backgrounded
  phone misses pushes and catches up via the reconnect VV exchange (Phase D).
- **FFI rebuild friction:** the no-explicit-modules iOS build shows false-positive SourceKit errors;
  `xcodebuild` is authoritative (per [[project-ios-sourcekit-false-positives]]).
- **Not in scope (explicitly):** presence/cursors, person-vs-device identity, edit attribution,
  off-tailnet reach, web-own-engine, relay push. Each is a later nested milestone.

---

## 9. Execution note

Substantial multi-session sync work → subagent-driven execution with per-phase spec+quality review,
matching how the cutover and Graphite phases ran. **Dependency order (red-team-corrected):** Phase 0
(trait + cursor-free export) is the foundation and must land first — both A and B depend on it. After
0: Phase A (server) and Phase B (FFI) can run in parallel; C depends on both A and B; D depends on C.
Commit per phase, don't push.

---

## 10. Red-team record (2026-05-30, 5-agent adversarial review of this spec)

Ran 5 independent skeptics against the spec's load-bearing assumptions before approval. **4 of 5 came
back "broken"** — all incorporated above. Verbatim verdicts retained for the implementers:

- **Finding #1 (broken) — FFI reuse.** `export_doc_update`/`import_doc_update` are NOT on the
  `SyncEngine` trait (only concrete `LoroEngine`); the FFI handle holds `Arc<dyn SyncEngine>` and
  can't call them → compile failure. **Fix:** Phase 0 moves them onto the trait. (`tesela-sync-ffi/
  src/lib.rs:250-253`, `engine/mod.rs:43-141`, `loro_engine.rs:367,399`.)
- **Finding #2 (broken) — binary WS frames.** The broadcast channel is `Sender<WsEvent>` →
  Text-JSON-only; iOS UTF-8-decodes every frame as JSON (`SyncState.swift:127`) and would silently
  drop a binary frame on the same channel. **Fix:** separate `ws_delta_tx` binary channel + frame-type
  dispatch on iOS. (`state.rs:17`, `ws.rs:17-19`, `ws-client.svelte.ts:178`.)
- **Finding #3 (broken) — cursor contention.** `produce_relay_updates` consumes the shared
  `broadcast_cursor`; if the live path also calls it, the relay commits first and the live delta
  silently never ships. **Fix:** live path uses cursor-free `export_doc_update(note, since_vv)` and
  forwards exact applied bytes; `broadcast_cursor` stays relay-only. (`loro_engine.rs:418-450`,
  `sync_relay.rs:237-263`.)
- **Finding #4 (broken) — web-as-view asymmetry (the big one).** The relay/WS *apply* path emits NO
  `WsEvent` (`sync_relay.rs` returns counts only; only the HTTP-edit path emits at `notes.rs:275`), so
  a phone→Mac edit materializes to disk but **web is never told to invalidate → stays stale**. The
  "vice-versa <1s" acceptance criterion fails exactly where Taylor watches (the Mac web client).
  **Fix:** emit `WsEvent::NoteUpdated` after every apply, including WS/relay-originated. Same finding's
  echo sub-point: forward exact applied bytes (don't re-`produce`) + origin-socket suppression for
  loop-freedom; added a 3-node hub convergence test to Phase A.
- **Finding #5 (holds-with-caveat) — scope vs goal.** Web-as-view DOES satisfy the goal *once finding
  #4 is fixed* — without the emit-on-apply fix it does not. Captured as the Phase A acceptance bar.

The convergence of #3 and #4 on the same fix ("forward exact applied bytes, don't re-produce, keep the
relay cursor isolated") is the spec's central correction and simplifies the design rather than
complicating it.
