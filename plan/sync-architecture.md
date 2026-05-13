# Tesela sync architecture

Plan document. Written 2026-05-12. Scope: design only, no implementation.

## Decisions in effect

Locked at the start of this phase:

1. Targets: existing macOS desktop, future native iOS (not yet built).
2. SQLite is canonical. Markdown files are export-only.
3. New crate `tesela-sync`. Depends on `tesela-core`, never on `tesela-server` or any UI.
4. Substrate: append-only oplog with hybrid logical clock (HLC), row-level last-writer-wins, behind a `SyncEngine` trait so a richer engine can replace it later.
5. Every op carries: HLC timestamp, schema version, content hash, producing device ID, mutation payload.
6. Two transports, one wire protocol: LAN (mDNS + TLS-pinned, no internet required) and WAN (thin WebSocket relay, Cloudflare Worker first, replaceable in ~200 lines).
7. End-to-end encryption: out-of-band pairing (QR carrying shared secret plus pubkeys). All ops encrypted to a group symmetric key. Relay sees ciphertext, device IDs, sizes, timestamps.
8. iOS background wake via APNs silent push is opportunistic, not load-bearing.
9. Schema migrations carry the producer's sync-op schema version on every op. Newer-than-local ops park; older ops translate.
10. Real-time multi-user collaboration is out of scope for this iteration. Substrate must not preclude it.

Resolved this phase:

11. Sync grain: blocks are first-class rows with stable UUIDs. Concurrent block edits in the same note converge.
12. Note identity: internal UUID, slug as a `display_alias` secondary unique column. Existing URLs resolve through alias lookup.
13. Wire format: postcard.
14. Tables in scope for the oplog:
    - Canonical (replicated): `notes`, `blocks`, `attachments`.
    - Derived (rebuilt locally on apply by pure, schema-versioned parsers): `links`, `notes_fts`, `block_properties`, `tag_defs`, `property_defs`.
    - Local-only: `note_versions` (per-device edit history; cross-device history is served by the oplog itself).
    - On schema upgrade, all derived tables are dropped and rebuilt from canonical state.
15. API shape: async Rust internal traits, with FFI-design discipline applied to public types from day one (no borrowed types or lifetimes in trait methods, no generics in public methods, owned error types, concrete return types). The `tesela-sync-ffi` crate that bridges to UniFFI is deferred until iOS work begins, and is expected to be a mechanical wrap, not a refactor.
16. Schema mismatch policy: queue silently in `parked_ops`, surface prominently (banner in main UI, optional system notification), document the cap-and-replay-from-peer evolution path so the wire protocol accommodates it without a breaking change.

Implication noted: this design completes the database-first shift previously approved (2026-03-25). The current `Indexer` pipeline (file watcher driving SQLite from markdown) is replaced. Markdown export remains but flows the other direction.

## 1. Crate layout

`crates/tesela-sync/`

```
Cargo.toml
src/
  lib.rs              public re-exports, SYNC_SCHEMA_VERSION constant
  engine/
    mod.rs            SyncEngine trait
    sqlite_engine.rs  default impl backed by SQLite
    cursor.rs         LocalCursor, PeerCursor types
    applied.rs        AppliedChanges struct
  oplog/
    mod.rs            OpLog reader/writer
    op.rs             EncodedOp, OpPayload, content_hash
    parked.rs         parked_ops queue, schema-version gating
    retention.rs      GC driven by ack cursors
  hlc/
    mod.rs            Hlc newtype wrapping uhlc::HLC
    timestamp.rs      HlcTimestamp (serde-friendly)
  wire/
    mod.rs            postcard encode/decode
    envelope.rs       SyncEnvelope (encrypted blob plus metadata)
  transport/
    mod.rs            Transport trait, TransportSession trait
    loopback.rs       in-process Phase 1 transport
    lan.rs            (Phase 2 placeholder) mDNS plus TLS
    relay.rs          (Phase 3 placeholder) WebSocket relay client
  crypto/
    mod.rs            AEAD primitives, group-key derivation
    pairing.rs        QR contents, handshake (Phase 2)
    keys.rs           device keypair, group-key storage adapter
  migrate/
    mod.rs            OpTranslator trait, TranslatorRegistry
    v1_to_v2.rs       reserved (no translators yet at v1)
  rebuild/
    mod.rs            derived-table rebuild dispatcher
  device.rs           DeviceId (16 bytes), DeviceMetadata
  group.rs            GroupId, GroupMember
  error.rs            SyncError (owned, FFI-friendly), SyncResult
  schema.rs           sqlite DDL constants for sync tables
tests/
  convergence.rs      two-engine convergence harness
  property.rs         proptest partition-recovery cases
examples/
  two_node.rs         Phase 1 demo binary
```

Public API surface, re-exported from `lib.rs`:

- `SyncEngine`, the engine constructor `SqliteEngine::open(db_path, device_id)`.
- `Op`, `EncodedOp`, `OpKind`, `OpPayload`, `HlcTimestamp`.
- `Transport`, `TransportSession`, `TransportTarget`, plus `LoopbackTransport` and (later) `LanTransport`, `RelayClient`.
- `DeviceId`, `GroupId`, `GroupMember`.
- `SyncError`, `SyncResult<T>`.
- `SCHEMA_VERSION_DDL_CONST` (the SQL DDL major version for sync tables) and `SYNC_SCHEMA_VERSION` (the op-format version stamped onto every locally produced op). These are distinct: DDL evolves via the existing `MIGRATIONS` mechanism; op format evolves via `OpTranslator`.
- `PairingInvitation`, `PairingResponse` (types exposed even before the handshake implementation lands, so callers can plan UI flows).

Crate-private modules: `oplog::retention`, `engine::sqlite_engine`, `migrate::v1_to_v2`, key-storage adapters.

## 2. SyncEngine trait

```rust
#[async_trait::async_trait]
pub trait SyncEngine: Send + Sync {
    // Local-side mutation entry point. Tesela-core funnels every write here
    // when sync is enabled. The engine appends an oplog row, then applies
    // the change to canonical tables, then schedules derived rebuild.
    // The returned hash is the content_hash of the appended op (for tests
    // and for the caller's own idempotency tracking if needed).
    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash>;

    // Apply incoming changes from a peer. Returns the set of canonical
    // row identifiers (note IDs, block IDs, attachment IDs) that changed,
    // so the caller can rebuild derived tables and invalidate UI.
    async fn apply_changes(
        &self,
        peer: DeviceId,
        envelope: SyncEnvelope,
    ) -> SyncResult<AppliedChanges>;

    // Produce ops authored locally with HLC strictly greater than `since`,
    // up to `max_bytes` of postcard-encoded payload. Returns the produced
    // batch and a new cursor pointing past the last yielded op.
    async fn produce_changes_since(
        &self,
        peer: DeviceId,
        since: PeerCursor,
        max_bytes: usize,
    ) -> SyncResult<ProducedBatch>;

    // Current cursor for ops THIS device has produced. Used by transports
    // when initiating a session.
    async fn local_cursor(&self) -> SyncResult<LocalCursor>;

    // Cursor we have stored for ops we have received from a given peer.
    async fn peer_cursor(&self, peer: DeviceId) -> SyncResult<PeerCursor>;

    // Record that a peer has acknowledged ops up to cursor C. Drives oplog
    // retention.
    async fn ack_peer(&self, peer: DeviceId, ack: PeerCursor) -> SyncResult<()>;

    // Park an op the local schema cannot understand. Used internally by
    // apply_changes; exposed for tests and admin tooling.
    async fn park_op(
        &self,
        op: EncodedOp,
        reason: ParkReason,
    ) -> SyncResult<()>;

    // Replay parked ops after a schema upgrade. Returns the set that
    // applied and the set still parked (e.g. ops parked for reasons
    // other than schema version).
    async fn replay_parked(&self) -> SyncResult<ReplayReport>;

    // Snapshot the count and oldest-parked timestamp for the UI banner.
    async fn parked_summary(&self) -> SyncResult<ParkedSummary>;
}
```

Rationale, per method:

- `record_local`: a single mutation entry point matters because triggers cannot reliably distinguish "user-driven write" from "apply-time replay" without a side channel (see section 3). Funneling every write through `record_local` keeps that distinction explicit. Returning the `ContentHash` lets tests assert idempotency easily.
- `apply_changes`: takes a `SyncEnvelope` rather than a `Vec<EncodedOp>` so decryption and origin verification live inside the engine rather than spread across transports. Returns `AppliedChanges` so the rebuild dispatcher knows what to refresh; the trait does not call rebuilders directly to keep the engine free of `tesela-core` derived-table internals.
- `produce_changes_since`: paged via `max_bytes` because oplog tail can grow without bound between sync sessions; we never want to load it all. `max_bytes` reflects the wire-level constraint better than `max_count` since per-op size varies wildly (a one-block edit versus a 50-block paste).
- Distinct `LocalCursor` and `PeerCursor` types: a `LocalCursor` is "the HLC of my latest produced op." A `PeerCursor` is "the HLC of the latest op I have received from peer P." They look similar but the misuse is real (asking a peer "send me everything after my local cursor" instead of "after the peer cursor I have for you") and the type system should prevent it.
- `ack_peer`: oplog retention is "keep until all peers have ack'd, plus a safety lag." Without explicit acks the only retention policy is "forever," which is wrong.
- `park_op` and `replay_parked` are exposed (not crate-private) because a future "Replay parked ops" admin button in the UI is a real product surface, and because the convergence tests need to drive parking explicitly.
- `parked_summary` exists so the UI banner can render the prominent notice required by the schema-mismatch policy without scanning the whole parked table.

## 3. Oplog schema

SQLite DDL (lives in `tesela-sync/src/schema.rs`, applied by a new migration in `tesela-core/src/db/schema.rs::MIGRATIONS` so existing migration machinery handles it):

```sql
CREATE TABLE oplog (
    hlc_physical    INTEGER NOT NULL,
    hlc_logical     INTEGER NOT NULL,
    device_id       BLOB    NOT NULL,
    schema_version  INTEGER NOT NULL,
    payload         BLOB    NOT NULL,        -- postcard-encoded OpPayload
    content_hash    BLOB    NOT NULL,        -- blake3 of (hlc, schema_version, payload)
    txn_id          BLOB,                    -- optional grouping for atomic batches
    PRIMARY KEY (hlc_physical, hlc_logical, device_id)
) WITHOUT ROWID;

CREATE INDEX idx_oplog_device_hlc
    ON oplog(device_id, hlc_physical, hlc_logical);

CREATE INDEX idx_oplog_content_hash
    ON oplog(content_hash);

CREATE TABLE peer_cursors (
    peer_device_id          BLOB    PRIMARY KEY,
    last_seen_hlc_physical  INTEGER NOT NULL,
    last_seen_hlc_logical   INTEGER NOT NULL,
    last_ack_at_wall_clock  INTEGER NOT NULL   -- millis since epoch, UI only
);

CREATE TABLE parked_ops (
    op_hlc_physical INTEGER NOT NULL,
    op_hlc_logical  INTEGER NOT NULL,
    op_device_id    BLOB    NOT NULL,
    schema_version  INTEGER NOT NULL,
    payload         BLOB    NOT NULL,
    parked_at       INTEGER NOT NULL,
    park_reason     TEXT    NOT NULL,
    PRIMARY KEY (op_hlc_physical, op_hlc_logical, op_device_id)
);

CREATE TABLE device_self (
    rowid           INTEGER PRIMARY KEY CHECK (rowid = 1),
    device_id       BLOB    NOT NULL,
    ed25519_pubkey  BLOB    NOT NULL,
    ed25519_privkey BLOB    NOT NULL,     -- in test mode; in prod, a keychain ref
    display_name    TEXT    NOT NULL
);

CREATE TABLE group_members (
    group_id        BLOB    NOT NULL,
    device_id       BLOB    NOT NULL,
    ed25519_pubkey  BLOB    NOT NULL,
    display_name    TEXT,
    added_at        INTEGER NOT NULL,
    PRIMARY KEY (group_id, device_id)
);

CREATE TABLE group_keys (
    group_id        BLOB    PRIMARY KEY,
    group_sym_key   BLOB    NOT NULL          -- in test mode; in prod, keychain ref
);
```

Notes on indexes:

- `idx_oplog_device_hlc` supports `produce_changes_since`, which scans by `(device_id, hlc) WHERE hlc > since`.
- `idx_oplog_content_hash` supports idempotent apply: receiving an op we already have is detected by hash lookup before decoding.
- Oplog uses `WITHOUT ROWID` because the natural primary key is composite and a separate rowid would waste space.

### How tracked-table mutations produce oplog entries

**Recommendation: application-layer writes via a `Mutation` API. Not triggers.**

Reasoning:

- A trigger fires on every row write and would emit an oplog entry. To distinguish user-driven writes from apply-time replays, the trigger would need a session-scoped "currently applying" flag in a separate table that the engine sets before applying remote ops. This works but feels brittle: forget to set the flag, and apply-time double-emits.
- Triggers cannot easily access the producing device ID, the HLC the engine wants to stamp, or the content_hash. They would need to coordinate with the application layer via temp tables, which is the worst of both worlds.
- `INSERT OR REPLACE`, common in upsert flows, fires DELETE then INSERT triggers, doubling output.
- Application-layer writes give us one entry point (`SyncEngine::record_local`), one place to compute the HLC and hash, and explicit choice at each call site whether the mutation came from local user action (emit op) or remote apply (do not emit op).

The cost is one-time: every existing write path in `tesela-core` and `tesela-server` must funnel through a new `Mutation` enum in `tesela-core` rather than calling `fs::write` or `sqlite_index.upsert` directly. This is the boring half of Phase 1.

### Retention policy

- After every `ack_peer`, recompute `min_acked_hlc = min over all peer_cursors`.
- Drop oplog rows where `(hlc_physical, hlc_logical) < min_acked_hlc - safety_lag`. Default safety lag: 24 hours of wall-clock equivalent, configurable.
- Never drop ops a local consumer has not yet ack'd (sanity check, should not happen if `record_local` always inserts ahead of cursor advances).

## 4. HLC implementation

**Choice: `uhlc` crate (Eclipse Zenoh).** Wrap behind our own `Hlc` newtype.

Survey:

- `uhlc` 0.7.x. Battle-tested in Eclipse Zenoh (distributed pub-sub) under partition and clock skew. Built-in drift bound. `serde::Serialize` and `serde::Deserialize` on `Timestamp`. `Send + Sync`. Active maintenance, but small maintainer pool.
- `hlc` (timberio) 0.2.x. Lightweight, simpler. Less production exposure. No drift bound by default.
- Roll our own. ~100 to 150 lines of Rust. Trade-off: we own correctness bugs in a notoriously easy-to-get-wrong primitive. Not recommended at our stage.

Why wrap:

- `uhlc::Timestamp` packs `(physical: u64 millis, logical: u32, device: uhlc::ID)` where `uhlc::ID` is a 16-byte identifier. We adopt this device-ID width across the codebase (matches UUIDs).
- Drift bound: `uhlc` defaults to 100ms max skew tolerance. We raise to 5 seconds because the macOS-iPhone path can have noticeable skew on first boot.
- Wrapping in `Hlc` means a future swap to in-house code is mechanical.

Sketch:

```rust
pub struct Hlc(uhlc::HLC);

impl Hlc {
    pub fn new(device: DeviceId) -> Self { /* configure max_drift = 5s */ }
    pub fn now(&self) -> HlcTimestamp { /* uhlc tick */ }
    pub fn observe(&self, remote: HlcTimestamp) -> SyncResult<HlcTimestamp> {
        // Advance our physical component if remote is ahead.
        // Reject if remote exceeds local wall clock by more than max_drift.
    }
}
```

## 5. Wire format

**Choice: postcard.** (Locked.)

On-wire op:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EncodedOp {
    pub hlc: HlcTimestamp,            // (physical: u64, logical: u32, device: [u8; 16])
    pub schema_version: u32,
    pub content_hash: [u8; 32],        // blake3
    pub txn_id: Option<[u8; 16]>,
    pub payload: OpPayload,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OpPayload {
    NoteUpsert {
        note_id: [u8; 16],             // UUID
        display_alias: Option<String>,
        title: String,
        created_at_millis: i64,
    },
    NoteDelete {
        note_id: [u8; 16],
    },
    BlockUpsert {
        block_id: [u8; 16],
        note_id: [u8; 16],
        parent_block_id: Option<[u8; 16]>,
        order_key: String,             // fractional indexing
        indent_level: u16,
        text: String,
    },
    BlockMove {
        block_id: [u8; 16],
        new_parent: Option<[u8; 16]>,
        new_order_key: String,
    },
    BlockDelete {
        block_id: [u8; 16],
    },
    AttachmentUpsert {
        attachment_id: [u8; 16],
        note_id: [u8; 16],
        filename: String,
        mime_type: String,
        size_bytes: u64,
        content_blake3: [u8; 32],      // bytes flow out-of-band, content-addressed
    },
    AttachmentDelete {
        attachment_id: [u8; 16],
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SyncEnvelope {
    pub from_device: DeviceId,
    pub to_group: GroupId,
    pub nonce: [u8; 24],               // XChaCha20-Poly1305
    pub ciphertext: Vec<u8>,           // sealed: postcard(Vec<EncodedOp>)
}
```

Order-key (block ordering) uses fractional indexing as comparable strings. Library choice: `fractional_index` crate or hand-rolled comparable-base62 (decide at Phase 1 implementation time). Concurrent inserts between siblings produce a key strictly between them without renumbering, which is the property the merge needs.

Content hash inputs: blake3 over the postcard-serialized bytes of `(hlc, schema_version, payload)`, in that order. The hash deliberately does not cover `content_hash` itself or `txn_id` so that grouping ops into a transaction does not change their hashes.

## 6. Transport adapter trait

```rust
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    async fn open(&self, target: TransportTarget) -> SyncResult<TransportSession>;
    async fn tick(&self) -> SyncResult<TransportTickReport>;
    fn incoming(&self) -> Box<dyn Stream<Item = TransportSession> + Send + Unpin>;
}

pub enum TransportTarget {
    Peer(DeviceId),                                    // resolved via discovery
    Relay { group: GroupId, relay_url: String },
}

#[async_trait::async_trait]
pub trait TransportSession: Send + Sync {
    fn peer(&self) -> DeviceId;
    async fn send(&self, envelope: SyncEnvelope) -> SyncResult<()>;
    async fn recv(&self) -> SyncResult<Option<SyncEnvelope>>;
    async fn close(&self) -> SyncResult<()>;
}
```

Implementations, in build order:

1. **LoopbackTransport** (Phase 1). Two `Transport` handles wired to each other by a pair of `tokio::sync::mpsc` channels. No network, no discovery, no crypto. Used exclusively for in-process tests and the `two_node` example.
2. **LanTransport** (Phase 2). mDNS discovery via `mdns-sd` advertising `_tesela._tcp.local` with the device ID and port. Mutual TLS over TCP, using `rustls` with a custom certificate verifier that pins the peer's Ed25519 pubkey from `group_members`. QUIC deferred (works but adds debugging surface for marginal benefit at the bandwidth Tesela exchanges).
3. **RelayClient** (Phase 3). WebSocket client. Reconnects with backoff. Multiplexes one logical connection per group. The relay protocol is a thin envelope: `Subscribe { group_id, since_hlc }`, `Publish { envelope }`, `Push { envelope }`. Stateless on the relay side.

The engine driver (a small loop layered on top of `SyncEngine` and `Transport`) is responsible for: calling `tick()` on a timer, accepting `incoming()` sessions, deciding whether to talk LAN-direct or via relay (LAN preferred when both peers are reachable), and feeding sessions to `apply_changes`.

## 7. Crypto plan

### Primitives

- Long-term device identity: Ed25519. Crate: `ed25519-dalek` 2.x.
- Ephemeral pairing key agreement: X25519 ECDH. Crate: `x25519-dalek` 2.x.
- Group symmetric key: 32 bytes, csprng-generated at group genesis.
- AEAD: XChaCha20-Poly1305 (ietf 24-byte nonce variant). Crate: `chacha20poly1305`.
- KDF: HKDF-SHA256. Crate: `hkdf`.
- Hash: blake3 for content hashes, sha256 for HKDF.

### Key derivation per envelope

```
per_envelope_key = HKDF-SHA256(
    ikm  = group_sym_key,
    salt = envelope.nonce,
    info = "tesela-env-v1"
)

ciphertext, tag = XChaCha20-Poly1305(
    key       = per_envelope_key,
    nonce     = envelope.nonce,
    plaintext = postcard(Vec<EncodedOp>),
    aad       = envelope.from_device || envelope.to_group
)
```

Binding AAD to routing metadata means an attacker who flips `from_device` or `to_group` invalidates the tag. The relay cannot rewrite envelopes.

### Group key management

- Group sym key is created on the first device when it generates a group.
- Each paired member receives the key during pairing, sealed to its X25519 pubkey.
- Rotation (when a device is removed) is itself an op: `GroupKeyRotate { new_key_id, encrypted_for_each_remaining_member }`. Rotation is deferred to a later phase (not Phase 1).

### Pairing flow

Out-of-band channel: a QR code shown on an existing trusted device (the "introducer"). The new device scans.

QR contents, URL-encoded, base64url for binary fields:

```
tesela://pair
    ?g={group_id}
    &intro={introducer_device_id}
    &intro_pub={introducer_ed25519_pubkey}
    &n={pairing_nonce_32_bytes}
    &relay={optional_relay_url}
```

Handshake, over a transport session opened by the new device (LAN if reachable, relay otherwise, using `group_id` for routing):

1. New device B generates Ed25519 identity if absent, plus an X25519 ephemeral keypair.
2. B sends `PairRequest { device_id: B, pubkey_ed: B_ed_pub, pubkey_x: B_x_pub, signed_nonce: Ed25519_sign(B_ed_priv, pairing_nonce) }`.
3. Introducer A verifies `signed_nonce` matches the QR nonce and that the signature is by `B_ed_pub` (proof of possession).
4. A displays a 6-digit code derived from `blake3(B_ed_pub)[..3]` to its user. User confirms on A.
5. A computes `shared = X25519(A_x_priv_ephemeral, B_x_pub)`, derives a sealing key, wraps `group_sym_key` and the current `group_members` list, sends `PairAccept { sealed_blob }`.
6. B unwraps. Stores `group_sym_key` in keychain. Writes `group_members` rows.
7. A appends `MemberAdded { device_id: B, pubkey_ed: B_ed_pub, display_name }` to its oplog, which propagates to other members through normal sync.

Threats out of scope: physical capture of an unlocked introducer device. Mitigation is "show QR" is a privileged action gated by OS unlock.

### Secret storage

- Device ed25519 privkey: macOS keychain via `security-framework`. On iOS, keychain via UniFFI shim. In test mode (env flag), raw bytes in `device_self.ed25519_privkey`.
- Group sym key: same storage tiers as device privkey. The `group_keys.group_sym_key` column holds either a keychain reference (production) or raw bytes (test).
- The keychain adapter trait lives in `crypto/keys.rs` so headless test runs can pick the raw-bytes adapter and CI does not need keychain access.

## 8. Schema migration interface

Two distinct version concepts:

- **DDL schema version.** The SQL schema of the local SQLite database (the existing `schema_migrations` table). Evolves via the existing `MIGRATIONS` mechanism in `tesela-core/src/db/schema.rs`. The sync crate's DDL slots in as a new migration (call it `004_sync_substrate`).
- **Sync op schema version.** The shape of `OpPayload` and its variants. Stamped onto every locally produced op. Evolves via `OpTranslator`. Tracked by the constant `SYNC_SCHEMA_VERSION` in `tesela-sync/src/lib.rs`. Starts at 1.

### Translator trait

```rust
pub trait OpTranslator: Send + Sync {
    fn from_version(&self) -> u32;
    fn to_version(&self) -> u32;
    fn translate(&self, payload: OpPayload) -> SyncResult<OpPayload>;
}

pub struct TranslatorRegistry {
    translators: std::collections::BTreeMap<(u32, u32), Box<dyn OpTranslator>>,
}

impl TranslatorRegistry {
    pub fn register(&mut self, t: Box<dyn OpTranslator>);
    pub fn chain(&self, from: u32, to: u32) -> Option<Vec<&dyn OpTranslator>>;
    pub fn translate(
        &self,
        from: u32,
        to: u32,
        payload: OpPayload,
    ) -> SyncResult<OpPayload>;
}
```

### Where translations live

`crates/tesela-sync/src/migrate/v{N}_to_v{N+1}.rs`, one file per bump. Registered in a single `pub fn register_all(reg: &mut TranslatorRegistry)` so the engine sets up the registry at startup. Translators are pure functions over `OpPayload`. They never touch the database directly.

A schema-version bump is introduced when:

- A variant of `OpPayload` changes shape (field added or removed).
- A semantic shift requires reinterpretation (e.g. `order_key` changes encoding).
- A new variant is added (translators map old payloads to the new variant where appropriate; otherwise old payloads remain valid in newer code).

### Apply path

```
on receive(op):
    if op.schema_version == SYNC_SCHEMA_VERSION:
        apply(op.payload)
    else if op.schema_version < SYNC_SCHEMA_VERSION:
        chain = registry.chain(op.schema_version, SYNC_SCHEMA_VERSION)
        if chain.is_none():
            park(op, reason = "no translator chain")
        else:
            translated = chain.apply(op.payload)
            apply(translated)
    else:                                                  // newer than local
        park(op, reason = "newer schema version")

on local upgrade (SYNC_SCHEMA_VERSION bumped):
    replay_parked()
    rebuild_derived_tables()        // schema-versioned parsers
```

### Derived-table rebuild on upgrade

Derived tables (`links`, `notes_fts`, `block_properties`, `tag_defs`, `property_defs`) are rebuilt from canonical state on any schema upgrade that could alter parser output. Rebuild lives in `tesela-sync/src/rebuild/`. It dispatches to the existing parsers in `tesela-core` (`block::parse_blocks`, link extraction, property pane derivation). The parsers must be pure (no I/O beyond reading canonical inputs) and bound to a schema version; future parser changes increment the version and trigger a rebuild on next start.

### Parked-op UX

- Banner in the main UI of the web client and TUI: "N ops parked from devices on a newer version of Tesela. Update to apply." Banner is non-dismissable while count is non-zero, but does not block interaction.
- Optional system notification on macOS user-notifications and iOS in-app banner.
- A "Parked ops" panel in settings shows count per producing device and oldest parked timestamp.
- The `parked_summary` engine method drives all three.

### Wire-protocol forward-compatibility for cap + replay-from-peer

The protocol supports unbounded parked retention today. To later add a cap, two changes are required, and both are local-only:

- `parked_ops` table grows a `dropped_at` column or we move dropped entries to a separate `parked_ops_dropped` table.
- The engine driver, when it detects parked-ops count exceeds the cap, sets its `peer_cursor` for the dropping device back far enough to request re-send.

`produce_changes_since` already takes an arbitrary `PeerCursor`, including `PeerCursor::Earliest`, so requesting the whole tail from a peer is supported without a new wire message. No breaking change is needed.

## 9. Test strategy

### Unit tests, in-crate

- `hlc::tests::monotonic`: a stream of `tick()` returns strictly increasing timestamps even when the wall clock goes backward.
- `hlc::tests::merge_advances_physical`: receiving a future timestamp advances the local physical component appropriately.
- `hlc::tests::rejects_skew_over_max_drift`: a remote 10 seconds ahead is rejected with `SyncError::ClockSkew`.
- `oplog::tests::idempotent_apply`: applying the same `EncodedOp` twice is a no-op (content_hash dedup).
- `oplog::tests::retention_respects_min_ack`: oplog rows older than `min_acked_hlc - safety_lag` are dropped; rows newer or not yet ack'd by some peer are kept.
- `migrate::tests::translator_chain`: register `v1_to_v2` and `v2_to_v3`, ask for `chain(1, 3)`, get both, applied in order.
- `migrate::tests::missing_chain_parks_op`: an op at v5 with no translator beyond v3 parks with reason "no translator chain."
- `crypto::tests::aead_roundtrip`: encrypt then decrypt with correct AAD succeeds; flipped AAD bits cause decryption failure.
- `crypto::tests::envelope_routing_bound_to_aad`: an envelope re-routed by flipping `to_group` fails decryption.

### Convergence tests (two simulated engines, in-process)

A `TestRig` lives in `tests/convergence.rs`:

```rust
struct TestRig {
    a: SqliteEngine,
    b: SqliteEngine,
    transport_ab: LoopbackTransport,
    transport_ba: LoopbackTransport,
}

impl TestRig {
    fn new() -> Self;
    async fn record_a(&self, payload: OpPayload);
    async fn record_b(&self, payload: OpPayload);
    async fn sync_a_to_b(&self);
    async fn sync_b_to_a(&self);
    async fn sync_bidirectional(&self);
    async fn canonical_state(&self, engine: &SqliteEngine) -> CanonicalSnapshot;
}
```

Cases:

- `one_way_full_corpus`: A creates 100 notes, each with 5 blocks. `sync_a_to_b()`. B's snapshot equals A's.
- `bidirectional_disjoint`: A creates 50 notes, B creates 50 different notes. `sync_bidirectional()`. Both have 100.
- `concurrent_different_blocks`: A and B both edit different blocks of the same note. Merge cleanly; both blocks present.
- `concurrent_same_block`: A and B edit the same block. HLC ordering picks a winner. The loser's `EncodedOp` is still in the oplog (verify via `produce_changes_since(.., Earliest, ..)`).
- `delete_vs_create_race`: A creates block X, B deletes block X (somehow seeing it via prior sync), HLC ordering decides outcome. Verify it matches the policy.
- `partition_recovery`: A and B each run independently for 100 random ops. Connect. Both converge to the same canonical state (assert `canonical_state(a) == canonical_state(b)`).
- `apply_is_idempotent_across_session_restarts`: kill the engine mid-apply (simulate via injected error), restart, run sync again, verify state and oplog are consistent (no duplicate applies, no missing applies).

`proptest` covers the partition-recovery case with random op streams: assert final state is independent of which side sync_a_to_b vs sync_b_to_a runs first.

### Integration test, two processes

`crates/tesela-sync/examples/two_node.rs` spawns two engines in the same process, on the same machine, with two distinct mosaic directories, connected by `LoopbackTransport`. Phase 1 stops here; this is sufficient to validate the substrate.

In Phase 2, when `LanTransport` lands, a sibling example `two_node_lan.rs` spawns two engines in separate child processes on the same machine, talking over loopback TCP with the production mDNS+TLS path. This validates that the abstraction holds across process boundaries and TLS sessions, not just in-process channels.

### Manual tests during Phase 1

- Power-loss simulation: kill the process between `record_local` writing the oplog row and applying to canonical tables. Restart. The oplog row exists; recovery applies it; canonical tables converge.
- Schema-upgrade dry run: snapshot a database at `SYNC_SCHEMA_VERSION = 1`. Add a stub `v1_to_v2` translator that adds a trivial field. Bump local version to 2. Verify all ops apply, derived tables rebuild, no data lost.

## 10. Phase 1 build slice

User suggested: "laptop-to-laptop over LAN only, no crypto, no relay, just oplog + HLC + SyncEngine."

**Counter-proposal: tighter Phase 1. One process, two engines, loopback transport. No network at all.**

Reasons:

- The substrate-correctness question (does engine + oplog + HLC + envelope codec + translator registry actually converge?) does not need a network. Removing networking removes a huge incidental-failure source during the period where we want to isolate "is the engine itself correct?"
- `LoopbackTransport` is roughly 50 lines (a pair of `tokio::sync::mpsc` channels). It is faster to iterate on, runs hundreds of convergence cases per second, and never flakes on mDNS announce races.
- mDNS plus TLS pinning is a real engineering chunk in its own right. Doing it concurrently with engine bring-up conflates two unrelated failure surfaces.
- Phase 2 then narrowly adds `LanTransport`, with the substrate already proven.

### Concrete Phase 1 deliverable

1. `crates/tesela-sync` skeleton matching section 1's layout (modules stubbed, public types defined, async-trait signatures compiled).
2. A new migration `004_sync_substrate` in `tesela-core/src/db/schema.rs::MIGRATIONS` that creates `oplog`, `peer_cursors`, `parked_ops`, `device_self`, `group_members`, `group_keys`.
3. `Hlc` wrapper around `uhlc::HLC` with monotonicity and skew-rejection tests.
4. `OpPayload` with five variants: `NoteUpsert`, `NoteDelete`, `BlockUpsert`, `BlockMove`, `BlockDelete`. `AttachmentUpsert` and `AttachmentDelete` deferred to Phase 2 (attachments require the content-addressed blob store, which is its own design).
5. Postcard encoding round-trip tests for each `EncodedOp` variant.
6. `SqliteEngine` implementing `SyncEngine` against an in-process SQLite handle.
7. `LoopbackTransport` and `TestRig`.
8. All unit and convergence tests from section 9 that do not require networking or crypto.
9. A `Mutation` API in `tesela-core` that all writes funnel through. This is the largest non-sync-crate change in Phase 1.
    - Refactor `FsNoteStore::create` / `update` / `delete` and the `Indexer` write path to call `Mutation::apply` instead of writing files plus upserting SQLite directly.
    - Refactor `tesela-server`'s `routes/notes.rs` to call `Mutation::apply` instead of going through `FsNoteStore` directly.
    - `Mutation::apply` writes canonical SQLite rows, calls `SyncEngine::record_local` to append the oplog row, then schedules derived-table rebuild for the affected canonical rows. Markdown export (if enabled) runs after, fed by canonical state.
    - This is where the database-first shift becomes load-bearing in code, not just policy.
10. Derived-table rebuild dispatcher in `tesela-sync::rebuild` that calls into existing `tesela-core` parsers and updates `block_properties`, `links`, `notes_fts`, `tag_defs`, `property_defs` for affected note IDs.
11. An `examples/two_node.rs` binary that wires two engines with `LoopbackTransport`, runs a scripted scenario (~50 ops), and asserts convergence.

### Out of Phase 1 (Phase 2 plus)

- `LanTransport` (mDNS plus TLS pinning).
- Crypto path (AEAD, group key, pairing flow). Phase 1 envelopes carry plaintext payloads; the `ciphertext` field holds a postcard-encoded `Vec<EncodedOp>` directly so the wire format is forward-compatible.
- `RelayClient` and a WAN relay reference implementation (Cloudflare Worker plus ~200 line Rust self-host).
- iOS UniFFI shim crate.
- APNs push proxy.
- Attachment ops and the content-addressed blob store.

### Phase 1 exit criteria

- All convergence tests in section 9 pass.
- `examples/two_node` converges a 50-op scripted scenario in under 100ms.
- `tesela-core` writes funnel through `Mutation::apply`. The existing web client, TUI, MCP server, and CLI all pass their existing tests against the refactored core.
- Hand-tested power-loss recovery and schema-upgrade dry run, both green.

### Open during Phase 1, decide before Phase 2

- Fractional-index library choice (`fractional_index` crate vs hand-rolled comparable strings). Decide during Phase 1 implementation based on which composes better with serde and SQLite collation.
- Whether `tesela-server`'s WebSocket broadcast becomes a thin shim over `AppliedChanges` (almost certainly yes; current ad-hoc event types are subsumed by "the engine just told me row X changed").
- Whether the existing `Indexer` (file-watcher driving SQLite) is deleted in Phase 1 or kept dormant. Recommend delete: under the database-first model the indexer's role is fully replaced by `Mutation::apply` and rebuild. Keeping it dormant invites accidental wiring.
