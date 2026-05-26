# Sync Architecture Redesign — design + roadmap

*Drafted 2026-05-26 after a week of patching cross-device data-loss bugs that kept reappearing as new variants. Adopted after a back-and-forth between Claude Code (working in-repo) and Claude Desktop (independent second opinion) plus Taylor's preferences. This doc is the canonical reference for the next 5-6 phases of sync work.*

## Background — why we're here

Over 2026-05-23 → 2026-05-26 we shipped ~12 commits patching iOS↔Mac↔web sync bugs. Each patch held for the specific repro it targeted; new variants of the same class of bug kept appearing. The pattern:

- iOS edits offline → reconnect → edits never reach server → web hard-refresh wipes them
- Web edit lands on server → overwrites iOS's pending block changes
- Two devices edit different blocks of the same note → the later whole-file PUT wipes the other's blocks

The fixes shipped this week (engine-eager open, `record_local` materializes, local-first refresh on iOS, pairing-code cache, ticker race no-op, debounced reparse on web, in-flight new-block protection) are all correct *individually* but each one is downstream of a deeper architectural problem.

## The diagnosis — data-model bugs, not sync bugs

Tesela today has **two incompatible models of "what is a note":**

1. **HTTP PUT path:** a note is a markdown file. `FsNoteStore.write_note` writes the file. Last writer wins by mtime.
2. **Engine path:** a note is a sequence of CRDT ops. `engine.record_local` logs an op, `engine.materialize` writes the file as a projection of those ops.

When these collide, no patching reconciles them — each path believes the other is wrong about the unit of truth. Every recent bug is a variant of "the file-LWW path won when the op-CRDT path should have."

Compounding factors:
- **Whole-file `NoteUpsert` ops.** The engine has `BlockUpsert/Move/Delete` primitives, but the actual write paths emit `NoteUpsert(content: full_body)`. So even when ops flow through the engine, they don't get block-granular CRDT semantics — concurrent edits to different blocks still race at the whole-file level.
- **Web has no relay client.** Web is HTTP-only. The relay-as-buffer story doesn't apply to web.
- **Relay co-located with Mac.** When Mac is offline, the relay is offline. Multi-device sync requires Mac running.

## The decision — what we're committing to

### Architecture model: relay-as-spine, P2P-as-LAN-optimization

The original framing of "P2P preferred, relay as fallback" doesn't survive iOS's background-task constraints. Apple gives third-party apps ~30 seconds of background runtime, no persistent sockets, no privileged push. **Pure P2P daily-driver sync on iOS is empirically impossible** (Syncthing's refusal to support iOS is the evidence).

The reframe: the **zero-knowledge relay is the spine** of cross-device sync. It already preserves everything we actually care about (sovereignty — AEAD-sealed envelopes the relay can't decrypt; no vendor lock-in — open source, swappable; offline-first — every device has full local replica). Direct device-to-device sync over Tailscale is a **latency optimization** for "both devices on LAN, foreground", not the architectural backbone.

### Data model: single write path, block-granular ops, hand-rolled CRDT (for now)

- **Single write path** through the engine. Server-side `PUT /notes/<id>` stops calling `FsNoteStore.write_note` directly; it submits ops to the engine. The engine materializes the file as a side effect. Eliminates the FsNoteStore/engine race.
- **Block-granular ops.** iOS and web emit `BlockUpsert/Move/Delete` instead of `NoteUpsert(content: full_body)`. Concurrent edits to different blocks survive naturally.
- **Hand-rolled CRDT, not Loro.** Our engine's existing primitives (HLC clock, postcard envelope format, relay protocol, UniFFI surface, apply_changes path) represent months of investment. Migrating to Loro would be a multi-month rewrite. For Tesela's actual concurrency profile (single user, 2-4 devices, low concurrent-edit rate), the pathological CRDT cases Loro solves are unlikely to materialize.
  - **Design Loro-compatibly anyway.** Before shipping the block ops, read Loro's op format and the movable-tree-CRDT paper. Make sure our ops project onto Loro's cleanly. Cheap insurance that keeps the migration door open.
  - **Explicit triggers for Loro migration** (see "Loro Triggers" section below). Without these written down, "evaluate later" becomes "never evaluate."

### Background sync on iOS: APNs silent push

iOS's only Apple-sanctioned mechanism for server-driven background sync is APNs `content-available: 1` push. The relay sends a push when there are pending ops for a device; iOS wakes for ~30s, pulls from relay, applies. This is what Signal/Tinder/every-iOS-app-with-sync uses. Without it, iOS only syncs when foregrounded — which is what's making sync feel laggy today.

### Disaster recovery: iPhone as full op-history replica

Today the iPhone has the engine's SQLite oplog (full op history) plus materialized files (`Documents/sync-ios-mosaic/`). This is **almost** enough to rehydrate a fresh Mac, but we haven't proven it end-to-end. The fire scenario (HA server + laptop both lost same day, only phone survives) requires this path to work.

**Practice the restore.** Take an iPhone backup, restore to a fresh Mac, prove it works. Re-test six months later. DR that hasn't been tested isn't DR.

### Two-year vision: per-tenant isolation already aligned

Taylor's honest answer to "who edits these notes in two years": realistically just him and household, possibly some friends, possibly a $5/mo hosted Tesela for people who don't want to self-host. The zero-knowledge relay's per-group HKDF isolation already supports this — each user/household is a distinct group, the relay sees opaque blobs, scaling to thousands of tenants is operational not architectural. Concurrency within any single mosaic stays low even in the hosted-service future. Hand-rolled block ops remain sufficient.

## The 7-step plan

In order. Each step is a phase. Each step's commit history should be reviewable independently.

### Phase 1: Single write path on Mac

Server-side `PUT /notes/<id>` stops calling `FsNoteStore.write_note` directly. Instead it submits a `NoteUpsert` op to the engine, which materializes the file. The HTTP handler becomes a thin op-submission wrapper.

**Eliminates:** the FsNoteStore/engine race that's behind bugs #2 and #3.
**Risk:** indexer + file watchers see the file written by engine instead of HTTP handler — should be transparent but worth confirming.

### Phase 2: Block-granular ops on writers (Loro-compatible format)

**Pre-step (~half day):** read Loro's op format spec and the movable-tree-CRDT paper. Design our block ops to project cleanly onto Loro's model.

Then: iOS `pushPage` and web `saveBlocks` stop emitting `NoteUpsert(content: full_body)`. They emit `BlockUpsert/Move/Delete` ops directly. The engine applies them in place; materialization regenerates the file from the new block state.

**Eliminates:** the "two devices edit different blocks, one whole-file PUT wins" class entirely.
**Risk:** the engine's `apply_block_*` functions today have not been heavily exercised — likely needs hardening.

### Phase 3: Verify the iPhone-only DR path (and practice it)

Document the recovery procedure: from a fresh Mac with no Tesela data, restore from an iPhone backup, prove the full mosaic is intact. Includes:
- Export `Documents/sync-ios-mosaic/sync.db` + `notes/` from iPhone
- Import into a fresh `tesela-server` install
- Verify: every note has the right body, materialized files match SQLite oplog, indexer sees everything, web client renders the same content
- Test the *iPhone-only-surviving-replica* case (no Mac to compare against)
- Document the steps so future Taylor can do this without me

Schedule a 6-month re-test on the roadmap so DR doesn't decay.

### Phase 4: APNs silent push for iOS background sync

Apple Push Notification certificate for the Tesela bundle ID (annual expiry — budget for rotation). Relay gains per-device pending-ops state (`device X has unacked ops since seq N`). When a new envelope is deposited for a group, the relay sends `content-available: 1` to every device in that group except the sender.

iOS gains background-fetch capability + APNs handler that wakes the app, runs a single tick, applies, terminates cleanly.

**Real setup work** (sandbox vs production APNs environment quirks; push-cert management; relay state schema changes; payload design — don't ship the op in the push, send "wake up, pull from relay"). Budget at least a week.

**Eliminates:** the "I opened iOS and waited 30 seconds for sync to converge" feeling. iOS becomes responsive to Mac/web edits in <1s while backgrounded.

### Phase 5: Deploy the Cloudflare Worker relay

The CF Worker port from 2026-05-25 (`cloudflare-relay/`) is already written but not deployed or wired. Deploy to Taylor's CF account. Mac-hosted relay becomes secondary; CF becomes the primary relay URL distributed via pairing codes.

**Eliminates:** the "Mac is offline, no sync" failure mode entirely. CF Worker survives Mac being down. Web (when it eventually gets a relay client per Phase 6) can sync via CF too.

### Phase 6 (deferred — design space only)

**Tailscale-direct sync as LAN/foreground optimization.** Devices on the same Tailnet skip the relay for sub-second sync. Build only if needed; the relay already feels fast enough with APNs.

**Web as relay-direct client when Mac is down.** Middle-path option from Claude Desktop's "third way." Web doesn't become a full local-replica (no WASM CRDT, no OPFS storage) but gains a thin relay-client so "Mac is down, I want to read my notes from a different laptop" works. Medium complexity. Don't build unless we hit the use case.

### Phase 7 (only if triggered): Loro migration

Triggers (any one of these fires → Loro evaluation becomes a project):

1. **Same block edited concurrently** by two devices and we can't deterministically merge.
2. **Move-and-edit-same-block** produces visible data loss.
3. **Two devices both delete a parent** while one edits a child.
4. **Any of the above happens twice in a month**, OR once with unrecoverable data loss.

Until triggered, hand-rolled stays. The Loro-compatible op format design from Phase 2 keeps the migration door open.

## What's deliberately NOT in scope

- **Multi-user-within-single-mosaic editing.** Not in any of Taylor's described futures.
- **Conflict-resolution UX (diff/merge prompt on conflict).** Was on the table in earlier discussions; CRDT-merge is the right default for daily-driver. Claude Desktop's read on Logseq's hated diff UX is correct.
- **CloudKit-based sync.** Apple Notes / Bear / iA Writer use this; it solves iOS background sync trivially. Wrong for Tesela's multi-platform goals (no web, no Linux/Windows).
- **Switching to Loro now.** Multi-month migration; defer until triggered.
- **Web as full local replica with WASM CRDT.** Significant new engineering; iPhone covers the daily-driver case when Mac is down.

## Loro Triggers — when to evaluate

Writing these down explicitly so "Loro later" doesn't become "Loro never":

| Trigger | Severity | Action |
|---|---|---|
| Same block edited concurrently, merge produces wrong content | High | Evaluate Loro |
| Move-and-edit-same-block loses data | High | Evaluate Loro |
| Delete-parent-while-editing-child loses data | High | Evaluate Loro |
| Any of the above happens twice in 30 days | Medium | Schedule Loro evaluation |
| Any of the above with unrecoverable data loss | Critical | Loro becomes immediate phase |
| Tesela ever supports multi-user-within-single-mosaic editing | Critical | Loro becomes immediate phase |

## What we should preserve from this discussion

The full reasoning chain (Claude Code's revised plan + Claude Desktop's concessions + the per-tenant-isolation argument that "multi-tenant doesn't push Loro earlier") is captured in memory notes saved alongside this doc:

- `project_sync_redesign_plan.md` — references this doc, summarizes the 7-step plan
- `project_loro_triggers.md` — the explicit trigger list above

## Open questions for the next session

- Should iOS local writes ALSO go through `record_local` block-granular ops, or stay as `NoteUpsert(full_body)` for now? (Today iOS emits `NoteUpsert` for daily/page pushes. Aligning with Phase 2 means iOS write path becomes block-emitting too.)
- Does the indexer (`tesela-core::indexer`) need any changes when files start being materialized differently? Probably no — it reads files; doesn't care who wrote them.
- Phase 4 APNs: APNs certificate needs to be issued under Taylor's Apple Developer account. Operational task, not code.
