//! SQLite-backed [`SyncEngine`] implementation.

use crate::device::DeviceId;
use crate::engine::applied::AppliedChanges;
use crate::engine::cursor::{LocalCursor, PeerCursor};
use crate::engine::{ParkedSummary, ProducedBatch, ReplayReport, SyncEngine};
use crate::error::{SyncError, SyncResult};
use crate::hlc::{Hlc, HlcTimestamp};
use crate::oplog::op::{compute_content_hash, ContentHash, EncodedOp, OpPayload};
use crate::oplog::parked::ParkReason;
use crate::schema;
use crate::wire::decode_op_batch;
use crate::wire::envelope::SyncEnvelope;
use async_trait::async_trait;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

/// SQLite-backed sync engine.
///
/// Holds a `SqlitePool`, an `Hlc` clock, and the local device id.
/// `clone` is cheap; the underlying state is `Arc`-wrapped.
#[derive(Clone)]
pub struct SqliteEngine {
    inner: Arc<Inner>,
}

struct Inner {
    pool: SqlitePool,
    hlc: Hlc,
    device: DeviceId,
    /// When set, `apply_changes` writes markdown files for incoming
    /// NoteUpsert / NoteDelete ops under `{mosaic_dir}/notes/`. When
    /// None, the engine is oplog-only (used in unit tests).
    mosaic_dir: Option<PathBuf>,
}

impl SqliteEngine {
    /// Open or create the engine state at the given SQLite URL (path or
    /// `sqlite::memory:`). Creates the device identity if absent. Runs
    /// the sync substrate DDL idempotently. No file materialization.
    pub async fn open(sqlite_url: &str, device: DeviceId) -> SyncResult<Self> {
        Self::open_with_mosaic(sqlite_url, None, device).await
    }

    /// Like [`open`] but additionally materializes markdown files under
    /// `mosaic_dir/notes/` when applying remote ops. Used by
    /// `tesela-server` so incoming sync ops produce on-disk files the
    /// existing `FsNoteStore` read path will see.
    pub async fn open_with_mosaic(
        sqlite_url: &str,
        mosaic_dir: Option<PathBuf>,
        device: DeviceId,
    ) -> SyncResult<Self> {
        let opts = SqliteConnectOptions::from_str(sqlite_url)
            .map_err(|e| SyncError::Storage(e.to_string()))?
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;
        schema::apply_ddl(&pool).await?;
        Self::ensure_device_self(&pool, device).await?;
        Ok(Self {
            inner: Arc::new(Inner {
                pool,
                hlc: Hlc::new(device),
                device,
                mosaic_dir,
            }),
        })
    }

    /// Local device id.
    pub fn device(&self) -> DeviceId {
        self.inner.device
    }

    /// Access the HLC (mostly for tests).
    pub fn hlc(&self) -> &Hlc {
        &self.inner.hlc
    }

    /// Total number of rows in the oplog. Useful for tests and diagnostics.
    pub async fn oplog_total(&self) -> SyncResult<i64> {
        let row = sqlx::query("SELECT COUNT(*) FROM oplog")
            .fetch_one(&self.inner.pool)
            .await?;
        Ok(row.get(0))
    }

    async fn ensure_device_self(pool: &SqlitePool, device: DeviceId) -> SyncResult<()> {
        let row = sqlx::query("SELECT device_id FROM device_self WHERE rowid = 1")
            .fetch_optional(pool)
            .await?;
        if row.is_none() {
            sqlx::query(
                "INSERT INTO device_self (rowid, device_id, ed25519_pubkey, ed25519_privkey, display_name)
                 VALUES (1, ?, ?, ?, ?)",
            )
            .bind(&device.0[..])
            .bind(Vec::<u8>::new()) // pubkey placeholder, Phase 2
            .bind(Vec::<u8>::new()) // privkey placeholder, Phase 2
            .bind("this-device")
            .execute(pool)
            .await?;
        }
        Ok(())
    }

    async fn append_op(&self, op: &EncodedOp) -> SyncResult<()> {
        let payload_bytes = postcard::to_allocvec(&op.payload)?;
        let result = sqlx::query(
            "INSERT OR IGNORE INTO oplog
                (hlc_ntp, device_id, schema_version, payload, content_hash, txn_id)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(op.hlc.ntp64_as_i64())
        .bind(&op.hlc.device.0[..])
        .bind(op.schema_version as i64)
        .bind(&payload_bytes[..])
        .bind(&op.content_hash.0[..])
        .bind(op.txn_id.as_ref().map(|t| &t[..]))
        .execute(&self.inner.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(SyncError::DuplicateOp {
                hash_hex: op.content_hash.to_hex(),
            });
        }
        Ok(())
    }

    async fn op_exists(&self, hash: &ContentHash) -> SyncResult<bool> {
        let row = sqlx::query("SELECT 1 FROM oplog WHERE content_hash = ?")
            .bind(&hash.0[..])
            .fetch_optional(&self.inner.pool)
            .await?;
        Ok(row.is_some())
    }

    fn touched_ids(payload: &OpPayload, applied: &mut AppliedChanges) {
        match payload {
            OpPayload::NoteUpsert { note_id, .. } | OpPayload::NoteDelete { note_id, .. } => {
                applied.note_ids.push(*note_id);
            }
            OpPayload::BlockUpsert {
                block_id, note_id, ..
            } => {
                applied.block_ids.push(*block_id);
                applied.note_ids.push(*note_id);
            }
            OpPayload::BlockMove { block_id, .. } | OpPayload::BlockDelete { block_id } => {
                applied.block_ids.push(*block_id);
            }
            OpPayload::AttachmentUpsert {
                attachment_id,
                note_id,
                ..
            } => {
                applied.attachment_ids.push(*attachment_id);
                applied.note_ids.push(*note_id);
            }
            OpPayload::AttachmentDelete { attachment_id } => {
                applied.attachment_ids.push(*attachment_id);
            }
        }
    }
}

#[async_trait]
impl SyncEngine for SqliteEngine {
    async fn record_local(&self, payload: OpPayload) -> SyncResult<ContentHash> {
        let hlc = self.inner.hlc.now();
        let op = EncodedOp::new(hlc, crate::SYNC_SCHEMA_VERSION, payload, None)?;
        let hash = op.content_hash;
        self.append_op(&op).await?;
        Ok(hash)
    }

    async fn apply_changes(
        &self,
        peer: DeviceId,
        envelope: SyncEnvelope,
    ) -> SyncResult<AppliedChanges> {
        let ops = decode_op_batch(&envelope.ciphertext)?;
        let mut applied = AppliedChanges::default();
        let mut max_seen: Option<HlcTimestamp> = None;

        for op in ops {
            op.verify_hash()?;

            // Advance our HLC against the remote timestamp.
            let _ = self.inner.hlc.observe(op.hlc)?;

            // Idempotency: skip ops we already have.
            if self.op_exists(&op.content_hash).await? {
                applied.deduped += 1;
                if max_seen.map(|m| op.hlc > m).unwrap_or(true) {
                    max_seen = Some(op.hlc);
                }
                continue;
            }

            // Schema version handling.
            if op.schema_version > crate::SYNC_SCHEMA_VERSION {
                self.park_op_internal(&op, ParkReason::NewerSchemaVersion)
                    .await?;
                applied.parked += 1;
                if max_seen.map(|m| op.hlc > m).unwrap_or(true) {
                    max_seen = Some(op.hlc);
                }
                continue;
            }
            // Older-than-local schema: Phase 1 has only v1, so unreachable.
            // Phase 2+ inserts translator chain here.

            self.append_op(&op).await?;
            self.materialize(&op.payload).await?;
            Self::touched_ids(&op.payload, &mut applied);
            applied.applied += 1;
            if max_seen.map(|m| op.hlc > m).unwrap_or(true) {
                max_seen = Some(op.hlc);
            }
        }

        if let Some(ts) = max_seen {
            self.update_peer_cursor(peer, ts).await?;
        }

        applied.note_ids.sort();
        applied.note_ids.dedup();
        applied.block_ids.sort();
        applied.block_ids.dedup();
        applied.attachment_ids.sort();
        applied.attachment_ids.dedup();

        Ok(applied)
    }

    async fn produce_changes_since(
        &self,
        peer: DeviceId,
        since: PeerCursor,
        max_bytes: usize,
    ) -> SyncResult<ProducedBatch> {
        // Send ops we know about that did not originate from `peer`. That
        // way, peer learns about transitive history from other devices.
        let since_ntp = match since {
            PeerCursor::Earliest => i64::MIN,
            PeerCursor::At(ts) => ts.ntp64_as_i64(),
        };
        let rows = sqlx::query(
            "SELECT hlc_ntp, device_id, schema_version, payload, content_hash, txn_id
             FROM oplog
             WHERE device_id != ?
               AND hlc_ntp > ?
             ORDER BY hlc_ntp ASC, device_id ASC",
        )
        .bind(&peer.0[..])
        .bind(since_ntp)
        .fetch_all(&self.inner.pool)
        .await?;

        let mut ops = Vec::new();
        let mut new_cursor = since;
        let mut bytes_used = 0usize;
        for row in rows {
            let hlc_ntp: i64 = row.get(0);
            let dev_bytes: Vec<u8> = row.get(1);
            let schema_version: i64 = row.get(2);
            let payload_bytes: Vec<u8> = row.get(3);
            let hash_bytes: Vec<u8> = row.get(4);
            let txn_bytes: Option<Vec<u8>> = row.get(5);

            if dev_bytes.len() != 16 {
                return Err(SyncError::Storage(format!(
                    "device_id wrong length: {}",
                    dev_bytes.len()
                )));
            }
            let mut dev = [0u8; 16];
            dev.copy_from_slice(&dev_bytes);
            if hash_bytes.len() != 32 {
                return Err(SyncError::Storage(format!(
                    "content_hash wrong length: {}",
                    hash_bytes.len()
                )));
            }
            let mut hash = [0u8; 32];
            hash.copy_from_slice(&hash_bytes);
            let txn_id = match txn_bytes {
                Some(b) if b.len() == 16 => {
                    let mut t = [0u8; 16];
                    t.copy_from_slice(&b);
                    Some(t)
                }
                _ => None,
            };
            let payload: OpPayload = postcard::from_bytes(&payload_bytes)?;
            let hlc = HlcTimestamp::from_ntp64_i64(hlc_ntp, DeviceId(dev));
            let op = EncodedOp {
                hlc,
                schema_version: schema_version as u32,
                content_hash: ContentHash(hash),
                txn_id,
                payload,
            };

            let projected = bytes_used + payload_bytes.len() + 64;
            if !ops.is_empty() && projected > max_bytes {
                break;
            }
            bytes_used = projected;
            new_cursor = PeerCursor::At(hlc);
            ops.push(op);
        }

        Ok(ProducedBatch { ops, new_cursor })
    }

    async fn local_cursor(&self) -> SyncResult<LocalCursor> {
        let row = sqlx::query(
            "SELECT hlc_ntp FROM oplog
             WHERE device_id = ?
             ORDER BY hlc_ntp DESC
             LIMIT 1",
        )
        .bind(&self.inner.device.0[..])
        .fetch_optional(&self.inner.pool)
        .await?;
        match row {
            None => Ok(LocalCursor::Earliest),
            Some(r) => {
                let ntp: i64 = r.get(0);
                Ok(LocalCursor::At(HlcTimestamp::from_ntp64_i64(
                    ntp,
                    self.inner.device,
                )))
            }
        }
    }

    async fn peer_cursor(&self, peer: DeviceId) -> SyncResult<PeerCursor> {
        let row = sqlx::query(
            "SELECT last_seen_hlc_ntp FROM peer_cursors WHERE peer_device_id = ?",
        )
        .bind(&peer.0[..])
        .fetch_optional(&self.inner.pool)
        .await?;
        match row {
            None => Ok(PeerCursor::Earliest),
            Some(r) => {
                let ntp: i64 = r.get(0);
                Ok(PeerCursor::At(HlcTimestamp::from_ntp64_i64(ntp, peer)))
            }
        }
    }

    async fn ack_peer(&self, peer: DeviceId, ack: PeerCursor) -> SyncResult<()> {
        let ntp = match ack {
            PeerCursor::Earliest => return Ok(()),
            PeerCursor::At(ts) => ts.ntp64_as_i64(),
        };
        let wall = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO peer_cursors
                (peer_device_id, last_seen_hlc_ntp, last_ack_at_wall_clock)
             VALUES (?, ?, ?)
             ON CONFLICT(peer_device_id) DO UPDATE SET
                last_seen_hlc_ntp = MAX(peer_cursors.last_seen_hlc_ntp, excluded.last_seen_hlc_ntp),
                last_ack_at_wall_clock = excluded.last_ack_at_wall_clock",
        )
        .bind(&peer.0[..])
        .bind(ntp)
        .bind(wall)
        .execute(&self.inner.pool)
        .await?;
        Ok(())
    }

    async fn park_op(&self, op: EncodedOp, reason: ParkReason) -> SyncResult<()> {
        self.park_op_internal(&op, reason).await
    }

    async fn replay_parked(&self) -> SyncResult<ReplayReport> {
        let still: i64 = sqlx::query("SELECT COUNT(*) FROM parked_ops")
            .fetch_one(&self.inner.pool)
            .await?
            .get(0);
        Ok(ReplayReport {
            applied: 0,
            still_parked: still as u32,
        })
    }

    async fn parked_summary(&self) -> SyncResult<ParkedSummary> {
        let row = sqlx::query("SELECT COUNT(*), MIN(parked_at) FROM parked_ops")
            .fetch_one(&self.inner.pool)
            .await?;
        let count: i64 = row.get(0);
        let oldest: Option<i64> = row.try_get(1).ok();
        Ok(ParkedSummary {
            count: count as u32,
            oldest_parked_at_millis: oldest,
        })
    }
}

impl SqliteEngine {
    /// Materialize an applied op into the on-disk markdown file (Phase 1.5
    /// blob model). No-op if `mosaic_dir` is None. The file watcher in
    /// `tesela-core::indexer::Indexer` will pick up the change and update
    /// the derived tables (`notes`, `notes_fts`, `links`,
    /// `block_properties`, `tag_defs`, `property_defs`) on its own.
    async fn materialize(&self, payload: &OpPayload) -> SyncResult<()> {
        let Some(mosaic) = self.inner.mosaic_dir.as_ref() else {
            return Ok(());
        };
        match payload {
            OpPayload::NoteUpsert {
                display_alias,
                content,
                ..
            } => {
                let Some(slug) = display_alias.as_deref() else {
                    // Without a slug we don't have a stable filename. Skip
                    // file materialization but the oplog row was already
                    // written, so future ops with the slug will work.
                    return Ok(());
                };
                let notes_dir = mosaic.join("notes");
                if let Err(e) = tokio::fs::create_dir_all(&notes_dir).await {
                    return Err(SyncError::Storage(format!(
                        "create_dir_all {}: {e}",
                        notes_dir.display()
                    )));
                }
                let path = notes_dir.join(format!("{slug}.md"));
                if let Err(e) = tokio::fs::write(&path, content).await {
                    return Err(SyncError::Storage(format!(
                        "write {}: {e}",
                        path.display()
                    )));
                }
                tracing::debug!(slug, "tesela-sync: materialized NoteUpsert");
            }
            OpPayload::NoteDelete { display_alias, .. } => {
                let Some(slug) = display_alias.as_deref() else {
                    tracing::debug!(
                        "tesela-sync: NoteDelete without slug; file delete skipped"
                    );
                    return Ok(());
                };
                let path = mosaic.join("notes").join(format!("{slug}.md"));
                match tokio::fs::remove_file(&path).await {
                    Ok(()) => {
                        tracing::debug!(slug, "tesela-sync: materialized NoteDelete");
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        tracing::debug!(slug, "tesela-sync: NoteDelete on already-gone file");
                    }
                    Err(e) => {
                        return Err(SyncError::Storage(format!(
                            "remove_file {}: {e}",
                            path.display()
                        )));
                    }
                }
            }
            OpPayload::BlockUpsert {
                block_id,
                note_id,
                parent_block_id,
                order_key,
                indent_level,
                text,
            } => {
                self.apply_block_upsert(
                    mosaic,
                    *note_id,
                    *block_id,
                    *parent_block_id,
                    order_key,
                    *indent_level,
                    text,
                )
                .await?;
            }
            OpPayload::BlockMove {
                block_id,
                new_parent,
                new_order_key,
            } => {
                self.apply_block_move(mosaic, *block_id, *new_parent, new_order_key)
                    .await?;
            }
            OpPayload::BlockDelete { block_id } => {
                self.apply_block_delete(mosaic, *block_id).await?;
            }
            OpPayload::AttachmentUpsert { .. } | OpPayload::AttachmentDelete { .. } => {
                // Phase 2: content-addressed blob store.
            }
        }
        Ok(())
    }

    /// Locate the slug for a note by walking the oplog for the most
    /// recent NoteUpsert with this note_id. None means no NoteUpsert
    /// is in the oplog yet (the block op landed before its note's
    /// create was replayed); the caller logs and skips.
    async fn find_slug_for_note(&self, note_id: [u8; 16]) -> SyncResult<Option<String>> {
        let rows = sqlx::query(
            "SELECT payload FROM oplog ORDER BY hlc_ntp DESC",
        )
        .fetch_all(&self.inner.pool)
        .await?;
        for row in rows {
            let bytes: Vec<u8> = row.get(0);
            let Ok(payload) = postcard::from_bytes::<OpPayload>(&bytes) else {
                continue;
            };
            if let OpPayload::NoteUpsert {
                note_id: nid,
                display_alias,
                ..
            } = payload
            {
                if nid == note_id {
                    return Ok(display_alias);
                }
            }
        }
        Ok(None)
    }

    /// Locate the note_id that owns a given block. First checks the
    /// oplog for any BlockUpsert with this block_id; if none, parses the
    /// stamped content carried by each NoteUpsert and returns the note
    /// whose content contains the block id. Returns None only when no
    /// trace of the block exists in the oplog yet (e.g. a BlockMove
    /// arrived before the establishing NoteUpsert or BlockUpsert).
    async fn find_note_for_block(&self, block_id: [u8; 16]) -> SyncResult<Option<[u8; 16]>> {
        let rows = sqlx::query(
            "SELECT payload FROM oplog ORDER BY hlc_ntp DESC",
        )
        .fetch_all(&self.inner.pool)
        .await?;
        let target_uuid = uuid::Uuid::from_bytes(block_id);
        let mut note_upserts: Vec<([u8; 16], String)> = Vec::new();
        for row in &rows {
            let bytes: Vec<u8> = row.get(0);
            let Ok(payload) = postcard::from_bytes::<OpPayload>(&bytes) else {
                continue;
            };
            match payload {
                OpPayload::BlockUpsert {
                    block_id: bid,
                    note_id,
                    ..
                } => {
                    if bid == block_id {
                        return Ok(Some(note_id));
                    }
                }
                OpPayload::NoteUpsert {
                    note_id, content, ..
                } => {
                    note_upserts.push((note_id, content));
                }
                _ => {}
            }
        }
        // Fallback: scan NoteUpsert content payloads.
        for (note_id, content) in note_upserts {
            let tree = tesela_core::note_tree::parse_note(&content);
            if tree.blocks.iter().any(|b| b.id == target_uuid) {
                return Ok(Some(note_id));
            }
        }
        Ok(None)
    }

    async fn apply_block_upsert(
        &self,
        mosaic: &std::path::Path,
        note_id: [u8; 16],
        block_id: [u8; 16],
        parent: Option<[u8; 16]>,
        _order_key: &str,
        indent_level: u16,
        text: &str,
    ) -> SyncResult<()> {
        let Some(slug) = self.find_slug_for_note(note_id).await? else {
            tracing::debug!(
                "BlockUpsert for unknown note_id; deferring until NoteUpsert arrives"
            );
            return Ok(());
        };
        let path = mosaic.join("notes").join(format!("{slug}.md"));
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
            Err(e) => {
                return Err(SyncError::Storage(format!(
                    "read {}: {e}",
                    path.display()
                )));
            }
        };
        let mut tree = tesela_core::note_tree::parse_note(&content);
        let block_uuid = uuid::Uuid::from_bytes(block_id);
        let parent_uuid = parent.map(uuid::Uuid::from_bytes);

        if let Some(existing) = tree.blocks.iter_mut().find(|b| b.id == block_uuid) {
            existing.text = text.to_string();
            existing.indent = indent_level;
            existing.parent = parent_uuid;
        } else {
            tree.blocks.push(tesela_core::note_tree::FlatBlock {
                id: block_uuid,
                parent: parent_uuid,
                indent: indent_level,
                text: text.to_string(),
            });
        }

        let serialized = tesela_core::note_tree::serialize_note(&tree);
        if let Err(e) = tokio::fs::write(&path, serialized).await {
            return Err(SyncError::Storage(format!(
                "write {}: {e}",
                path.display()
            )));
        }
        tracing::debug!(slug, "tesela-sync: materialized BlockUpsert");
        Ok(())
    }

    async fn apply_block_move(
        &self,
        mosaic: &std::path::Path,
        block_id: [u8; 16],
        new_parent: Option<[u8; 16]>,
        _new_order_key: &str,
    ) -> SyncResult<()> {
        let Some(note_id) = self.find_note_for_block(block_id).await? else {
            tracing::debug!("BlockMove for unknown block_id; deferring");
            return Ok(());
        };
        let Some(slug) = self.find_slug_for_note(note_id).await? else {
            tracing::debug!("BlockMove: note_id has no NoteUpsert; deferring");
            return Ok(());
        };
        let path = mosaic.join("notes").join(format!("{slug}.md"));
        let Ok(content) = tokio::fs::read_to_string(&path).await else {
            tracing::debug!(slug, "BlockMove: target file missing; deferring");
            return Ok(());
        };
        let mut tree = tesela_core::note_tree::parse_note(&content);
        let block_uuid = uuid::Uuid::from_bytes(block_id);
        let parent_uuid = new_parent.map(uuid::Uuid::from_bytes);
        if let Some(existing) = tree.blocks.iter_mut().find(|b| b.id == block_uuid) {
            existing.parent = parent_uuid;
            // Recompute indent from parent: if parent exists in the tree,
            // child indent is parent.indent + 1; else top-level (0).
            let new_indent = match parent_uuid {
                None => 0u16,
                Some(pid) => tree
                    .blocks
                    .iter()
                    .find(|b| b.id == pid)
                    .map(|p| p.indent + 1)
                    .unwrap_or(0),
            };
            if let Some(existing) = tree.blocks.iter_mut().find(|b| b.id == block_uuid) {
                existing.indent = new_indent;
            }
            let serialized = tesela_core::note_tree::serialize_note(&tree);
            if let Err(e) = tokio::fs::write(&path, serialized).await {
                return Err(SyncError::Storage(format!(
                    "write {}: {e}",
                    path.display()
                )));
            }
            tracing::debug!(slug, "tesela-sync: materialized BlockMove");
        } else {
            tracing::debug!(slug, "BlockMove: block not found in current file");
        }
        Ok(())
    }

    async fn apply_block_delete(
        &self,
        mosaic: &std::path::Path,
        block_id: [u8; 16],
    ) -> SyncResult<()> {
        let Some(note_id) = self.find_note_for_block(block_id).await? else {
            tracing::debug!("BlockDelete for unknown block_id; deferring");
            return Ok(());
        };
        let Some(slug) = self.find_slug_for_note(note_id).await? else {
            tracing::debug!("BlockDelete: note_id has no NoteUpsert; deferring");
            return Ok(());
        };
        let path = mosaic.join("notes").join(format!("{slug}.md"));
        let Ok(content) = tokio::fs::read_to_string(&path).await else {
            tracing::debug!(slug, "BlockDelete: target file missing");
            return Ok(());
        };
        let mut tree = tesela_core::note_tree::parse_note(&content);
        let block_uuid = uuid::Uuid::from_bytes(block_id);
        let before = tree.blocks.len();
        tree.blocks.retain(|b| b.id != block_uuid);
        if tree.blocks.len() == before {
            return Ok(());
        }
        // Also drop any children that pointed at the deleted block.
        // Simpler: re-parent them to None (preserves their content,
        // loses the hierarchy). Tradeoff: matches the on-disk
        // representation we can express; an alternative would be to
        // recursively delete them, but that loses user data.
        for child in tree.blocks.iter_mut() {
            if child.parent == Some(block_uuid) {
                child.parent = None;
                child.indent = 0;
            }
        }
        let serialized = tesela_core::note_tree::serialize_note(&tree);
        if let Err(e) = tokio::fs::write(&path, serialized).await {
            return Err(SyncError::Storage(format!(
                "write {}: {e}",
                path.display()
            )));
        }
        tracing::debug!(slug, "tesela-sync: materialized BlockDelete");
        Ok(())
    }

    async fn park_op_internal(&self, op: &EncodedOp, reason: ParkReason) -> SyncResult<()> {
        let payload_bytes = postcard::to_allocvec(&op.payload)?;
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT OR IGNORE INTO parked_ops
                (op_hlc_ntp, op_device_id, schema_version, payload, parked_at, park_reason)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(op.hlc.ntp64_as_i64())
        .bind(&op.hlc.device.0[..])
        .bind(op.schema_version as i64)
        .bind(&payload_bytes[..])
        .bind(now)
        .bind(reason.as_db_string())
        .execute(&self.inner.pool)
        .await?;
        Ok(())
    }

    async fn update_peer_cursor(&self, peer: DeviceId, ts: HlcTimestamp) -> SyncResult<()> {
        let wall = chrono::Utc::now().timestamp_millis();
        sqlx::query(
            "INSERT INTO peer_cursors
                (peer_device_id, last_seen_hlc_ntp, last_ack_at_wall_clock)
             VALUES (?, ?, ?)
             ON CONFLICT(peer_device_id) DO UPDATE SET
                last_seen_hlc_ntp = MAX(peer_cursors.last_seen_hlc_ntp, excluded.last_seen_hlc_ntp),
                last_ack_at_wall_clock = excluded.last_ack_at_wall_clock",
        )
        .bind(&peer.0[..])
        .bind(ts.ntp64_as_i64())
        .bind(wall)
        .execute(&self.inner.pool)
        .await?;
        Ok(())
    }
}

#[doc(hidden)]
pub fn _compute_content_hash_for_test(
    hlc: &HlcTimestamp,
    schema_version: u32,
    payload: &OpPayload,
) -> SyncResult<ContentHash> {
    compute_content_hash(hlc, schema_version, payload)
}
