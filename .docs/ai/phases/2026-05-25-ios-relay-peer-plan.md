# iOS as a real sync peer — Path B execution plan

**Date locked:** 2026-05-25
**Decision:** Path B (full peer via expanded UniFFI) over Path A (HA-hosted
tesela-server) or Path C (offline-first with replay queue).
**Materialization model:** Full — iOS reconstructs the same Markdown-on-disk-
equivalent representation as the Mac. Trade-off accepted: ~1 extra week vs the
"lean" path, in exchange for byte-equivalence between iOS-opened and Mac-opened
notes.

Companion to [`2026-05-24-relay-protocol-design.md`](2026-05-24-relay-protocol-design.md).
That document specifies the wire protocol; this one specifies how iOS becomes
a first-class participant in it.

---

## Why Path B

Taylor's pain point: today iOS is an HTTP client to the Mac's tesela-server.
The WAN relay we shipped on 2026-05-24 helps Mac↔Mac sync but doesn't help
iOS at all — iPhone still needs Mac reachable. When the Mac is asleep,
travelling, or off, iOS is dead in the water.

Path A (tesela-server-on-HA as iOS's permanent backend) would deliver value
in 2–4 days but stamps a second source-of-truth mosaic onto the architecture
and puts the full note corpus on the HA host (encrypted at rest only via HA
snapshots). Path B is the long-term architecture: iPhone speaks the relay
protocol directly, no intermediate backend, true offline-first.

## Why UniFFI expansion, not native Swift

Crypto + postcard + the engine apply path are all already correct in Rust
and exercised by tests. CryptoKit could re-implement the primitives in
Swift, but postcard has no Swift port and the engine's CRDT semantics are
non-trivial. UniFFI compiles to an XCFramework / static library; Swift calls
into Rust the same way it calls system frameworks. No double-implementation,
no bug-for-bug compat work.

The existing `tesela-sync-ffi` crate is already laid out for expansion (see
the `_AnchorArc` placeholder + the "next engine-exposing pass" comment at
the bottom of `lib.rs`). iOS already links `libtesela_sync_ffi.a` directly
through `OTHER_LDFLAGS` in `app/Tesela-iOS/project.yml`. The build pipeline
exists; we just need to grow what flows through it.

---

## Milestone plan

### B.0 — design doc + task tracking (this doc, ~0.5d)

### B.1 — wire the engine + relay client through UniFFI (~3d)

Goal: iOS app can open a local SQLite-backed sync engine, register with the
relay, and poll it. No user-visible change yet — proves the pipeline.

- **B.1.1** Expose `SqliteEngine` over UniFFI: `open`, `device`,
  `apply_changes`, `produce_changes_since`. Async via
  `#[uniffi::export(async_runtime = "tokio")]` where the underlying call
  is async.
- **B.1.2** Expose `RelayClient` over UniFFI: `new`,
  `register_or_recover`, `verify_registration`, `put_envelope`, `poll`,
  `ack`. All async.
- **B.1.3** Cross-compile + regen bindings + verify iOS app builds on both
  simulator (aarch64-apple-ios-sim) and device (aarch64-apple-ios).
- **B.1.4** Add a temporary smoke probe to iOS — at launch, open an engine
  in the app's sandbox, register with the relay, poll once, log the result.
  Verify in the relay's logs that iOS shows up as a known member.

**Exit criterion:** iOS app launches → relay sees a new device id register
with the same group as the Mac.

### B.2 — iOS becomes a real producer (~2–3d)

Goal: editing a note on iPhone produces an op locally, pushes through the
relay, and the Mac applies it.

- **B.2.1** Hook iOS's existing block-edit handlers to produce sync ops
  through the engine. Use the same `NoteUpsert` op shape the Mac produces.
- **B.2.2** Background relay tick on iOS (Swift `Task` + `Timer`, or
  Combine timer wired to the engine). Cadence configurable from Settings.
- **B.2.3** Reachability gate — when offline, queue in the local engine
  (it already buffers); when reachable, drain.
- **B.2.4** End-to-end test: edit "hello world" on iPhone simulator;
  Mac mosaic shows the same edit within poll-interval seconds.

**Exit criterion:** iOS-originated edit appears on Mac via relay.

### B.3 — iPhone becomes a real consumer + UI (~3d)

Goal: ops from the Mac apply locally on iOS and materialize into the iOS
note view. Settings → Sync becomes editable.

- **B.3.1** Apply loop: iOS polls relay, applies envelopes via the engine,
  fires Combine/SwiftUI notifications to refresh affected views.
- **B.3.2** Full materialization — port enough of tesela-core's note
  rendering to UniFFI that iOS can produce the same Markdown-equivalent
  representation as the Mac (full materialization decision, locked
  2026-05-25).
- **B.3.3** Settings → Sync editable: relay URL field, poll interval,
  pairing-code generation (iOS-side, with `relay_url` populated).
- **B.3.4** Remove or deprecate the iOS-as-HTTP-client-of-Mac fallback in
  `MockMosaicService` once the relay path is reliable.

**Exit criterion:** Mac-originated edit appears live on iOS without iPhone
ever touching the Mac's HTTP API.

---

## Risks

1. **Materialization scope creep.** The Mac uses `tesela-core` for note
   rendering, which knows about block types, properties, tags, types
   registry, etc. Full materialization on iOS means exposing that surface
   too. Mitigation: keep the materialization layer pure (no `tokio`, no I/O)
   so the UniFFI wrap is mechanical; punt on advanced widgets (calendar,
   agenda) — they can keep reading from the engine's already-applied data
   on B.3 and improve over time.

2. **Engine concurrency on iOS.** SwiftUI views can't block on Rust calls.
   UniFFI's async support (`uniffi-rs ≥ 0.28`) handles this — async Rust
   fns become Swift `async` fns automatically. Single shared `SqliteEngine`
   per mosaic, owned by a Swift `actor` to serialize access.

3. **Pairing flow changes.** Today iOS generates pairing codes with
   `relay_url: None`. Once iOS is a peer, both directions must populate it.
   B.3.3 handles this; smoke test that joining the iPhone group from a fresh
   Mac via iPhone-issued QR code auto-configures the relay.

4. **App-store / signing.** No new entitlements needed — UniFFI is just a
   static library. Existing signing config in `project.yml` covers it.

5. **The user-experienced "first sync" after B.3.** The engine's CRDT will
   converge any two mosaics, but a phone that pairs into an existing 1000-note
   mosaic does a full materialization on first sync. Could be slow. Mitigation:
   surface a one-time progress UI ("syncing 437 of 1000…") in B.3.

---

## What we deliberately defer

- **Push notifications when the relay has pending envelopes.** The poll loop
  is plenty for v1; APNs integration is its own scoped work.
- **Background sync via iOS Background Tasks.** Foreground sync only in
  Path B; the relay's nonce dedupe + idempotent apply means background
  catch-up is safe to add later without protocol changes.
- **iPad-specific UX.** iOS-mobile only for Path B. iPad UX is `project_mobile_strategy.md` follow-up.

---

## How we'll know it's working

End-to-end smoke (mirror of `crates/tesela-relay/scripts/smoke.sh` but with
iPhone as one peer):

1. Mac mosaic + iPhone both configured against the same relay URL.
2. Edit a note on Mac → save → wait `poll_interval`.
3. Open the same note on iPhone → see the edit.
4. Edit on iPhone → save → wait.
5. Switch back to Mac → see the iPhone's edit.

We'll add this as a manual QA checklist at the end of B.3, plus an
automated test that runs the engine + relay client headlessly from the iOS
test target.
