# Sync architecture, phase completion report

Plan file: `plan/sync-architecture.md`. Written 2026-05-12.

## Locked at end of this phase

Sixteen decisions, the first ten entering the phase locked by you, the last six resolved during it.

1. Targets: existing macOS desktop, future native iOS.
2. SQLite is canonical; markdown files are export-only.
3. New crate `tesela-sync`, depending only on `tesela-core`.
4. Substrate: append-only oplog plus HLC plus row-level last-writer-wins, behind a `SyncEngine` trait that can be replaced later.
5. Every op carries HLC, schema version, content hash, device ID, mutation payload.
6. Two transports (LAN mDNS plus TLS-pinned, WAN thin WebSocket relay), one wire protocol.
7. End-to-end encryption with out-of-band pairing (QR with shared secret plus pubkeys). Relay sees ciphertext, device IDs, sizes, timestamps.
8. iOS APNs background wake is opportunistic, not load-bearing.
9. Schema migrations: ops carry producer schema version; newer-than-local parks, older-than-local translates.
10. Real-time multi-user collaboration is out of scope this iteration; substrate must not preclude it.
11. Sync grain: blocks are first-class rows with stable UUIDs.
12. Note identity: internal UUID, slug as `display_alias` secondary unique column.
13. Wire format: postcard.
14. Tables in scope: canonical = `notes`, `blocks`, `attachments`. Derived (rebuilt on apply by pure schema-versioned parsers) = `links`, `notes_fts`, `block_properties`, `tag_defs`, `property_defs`. Local-only = `note_versions`. Derived tables drop and rebuild from canonical on schema upgrade.
15. API shape: async Rust traits, FFI-discipline applied to public types from day one. The `tesela-sync-ffi` UniFFI shim is deferred until iOS work begins.
16. Schema-mismatch policy: queue silently in `parked_ops`, prominent in-UI banner plus optional system notification, wire protocol designed so the planned cap-and-replay-from-peer evolution lands without a breaking change.

## Still open

Items the plan recommends with reasoning but does not lock by decision. These are safe to defer to Phase 1 implementation or Phase 2 decision points.

1. Fractional-index library (the `fractional_index` crate vs hand-rolled comparable strings). Decide during Phase 1 once we have a concrete read of how serde and SQLite collation compose. Either is workable.
2. Whether `tesela-server`'s WebSocket broadcast becomes a thin shim over `AppliedChanges`. Plan recommends yes; concrete diff lives in Phase 1.
3. Whether the existing `Indexer` (file-watcher to SQLite) is deleted in Phase 1 or kept dormant. Plan recommends delete, since the database-first shift fully replaces its role.
4. Group-key rotation flow (op shape, encryption-for-each-remaining-member detail). Deferred. The `GroupKeyRotate` op shape is sketched, not finalized.
5. Phase 2 transport details: QUIC vs TCP plus TLS for `LanTransport`. Plan recommends TCP plus TLS for now; QUIC is a Phase 3 reconsideration if iOS QUIC stories mature.
6. APNs push proxy: still optional and out of Phase 1. Its existence is allowed by the design but the plan does not commit to building it.
7. iOS UniFFI shim's exact surface area. Plan locks "must be a mechanical wrap, not a refactor" via the FFI-discipline rules in section 15, but the shim crate is not designed yet.
8. Attachment ops (`AttachmentUpsert`, `AttachmentDelete`) and the content-addressed blob store they assume. Deferred to Phase 2.

## Risks and watch items

1. **The `Mutation` API refactor is the biggest non-sync code change in Phase 1.** Every existing write path in `tesela-core` and `tesela-server` must funnel through it. Existing tests across the workspace need to pass after refactor. Risk: missed write sites that bypass the funnel and silently never emit ops. Mitigation: a small `#[deny(...)]`-style lint or a runtime assertion in dev builds that catches direct SQLite writes outside `Mutation::apply`.
2. **Database-first shift is policy in memory but not in code yet.** Until the `Mutation` API lands, markdown files remain a load-bearing input. The plan's correctness assumes this completes in Phase 1. If Phase 1 ships in pieces, sync correctness depends on which piece shipped.
3. **HLC max-drift tuning.** Default `uhlc` is 100ms. Plan recommends 5 seconds for macOS-to-iPhone first-boot tolerance. If we set it too generously, a misconfigured clock-broken peer can poison the HLC space; if we set it too tightly, legitimate slow clocks get rejected. Watch in Phase 2 when real iOS clocks enter the test.
4. **Derived-table rebuild is a parser correctness surface.** Every schema-versioned parser must be pure and bound to a version. A bug here corrupts derived state for everyone on upgrade. Plan recommends unit testing each parser against pinned snapshots before any version bump.
5. **`uhlc` maintainer pool is small.** Mitigation: wrap behind our own `Hlc` newtype so a swap is mechanical, not a refactor.

## Recommended next phase

**Phase 1: Substrate.** Build the engine, oplog, HLC, postcard wire format, loopback transport, translator registry, and `Mutation` API funnel. No network, no crypto. Exit criteria in `plan/sync-architecture.md` section 10.

Estimated scope is 4 to 6 weeks of focused work, of which roughly half is the `Mutation` API refactor across `tesela-core` and `tesela-server`. The sync-crate-internal work (engine plus oplog plus HLC plus codec plus translator plus tests) is the smaller half.

After Phase 1 exit, recommended Phase 2 is `LanTransport` (mDNS plus TLS pinning), and Phase 3 is the WAN relay client plus reference implementation. Crypto sits in Phase 2 alongside the LAN transport since pairing and TLS pinning share key material.

## Files written this phase

- `plan/sync-architecture.md` (this phase's deliverable, 10 sections)
- `plan/sync-architecture-report.md` (this file)

No implementation code was written. No source files outside `plan/` were modified.
