//! On-wire op types.

use crate::error::{SyncError, SyncResult};
use crate::hlc::HlcTimestamp;
use serde::{Deserialize, Serialize};

/// 32-byte blake3 hash uniquely identifying an op's payload-plus-metadata.
///
/// Used for idempotency: receiving the same op twice (e.g. from two
/// transports) is detected by hash lookup before any side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash(pub [u8; 32]);

impl ContentHash {
    /// Hex-encoded lowercase representation (64 chars).
    pub fn to_hex(&self) -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut out = String::with_capacity(64);
        for &b in &self.0 {
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
        out
    }
}

/// Discriminator for what kind of mutation an op carries. Used for indexing
/// and for human-readable logging; the canonical structure is the
/// [`OpPayload`] variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpKind {
    /// A note row was created or updated.
    NoteUpsert,
    /// A note row was deleted.
    NoteDelete,
    /// A block row was created or updated.
    BlockUpsert,
    /// A block row was moved (parent or order changed).
    BlockMove,
    /// A block row was deleted.
    BlockDelete,
    /// An attachment row was created or updated.
    AttachmentUpsert,
    /// An attachment row was deleted.
    AttachmentDelete,
}

/// The body of an op. One variant per canonical table mutation type.
///
/// Phase 1 implements the five Note and Block variants. Attachment variants
/// are reserved for Phase 2 once the content-addressed blob store lands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OpPayload {
    /// Create or update a note row.
    ///
    /// Phase 1.5 note: `content` carries the full markdown body for
    /// "blob" sync. When the block-level data model lands (Phase 2 of
    /// the data model), a v1-to-v2 translator will decompose
    /// `NoteUpsert.content` into a sequence of `BlockUpsert` ops at apply
    /// time, and `content` becomes the empty string on new ops.
    NoteUpsert {
        /// Note primary key (UUID).
        note_id: [u8; 16],
        /// Optional human-readable slug (e.g. "2026-05-12"). Non-unique
        /// across devices; conflicts surface as recoverable.
        display_alias: Option<String>,
        /// Note title.
        title: String,
        /// Full markdown content including frontmatter. Phase 1.5 only.
        content: String,
        /// Creation timestamp, millis since epoch.
        created_at_millis: i64,
    },
    /// Delete a note row.
    NoteDelete {
        /// Note primary key.
        note_id: [u8; 16],
    },
    /// Create or update a block row.
    BlockUpsert {
        /// Block primary key (UUID).
        block_id: [u8; 16],
        /// Note this block belongs to.
        note_id: [u8; 16],
        /// Optional parent block for hierarchy. None means top-level.
        parent_block_id: Option<[u8; 16]>,
        /// Fractional-index ordering key (comparable string).
        order_key: String,
        /// Indent level (0 = root).
        indent_level: u16,
        /// Block text.
        text: String,
    },
    /// Move a block to a new parent or order position.
    BlockMove {
        /// Block being moved.
        block_id: [u8; 16],
        /// New parent. None means top-level.
        new_parent: Option<[u8; 16]>,
        /// New fractional-index ordering key.
        new_order_key: String,
    },
    /// Delete a block row.
    BlockDelete {
        /// Block being deleted.
        block_id: [u8; 16],
    },
    /// Create or update an attachment metadata row.
    ///
    /// Reserved for Phase 2. Attachment bytes flow out-of-band via a
    /// content-addressed blob store; this op carries metadata only.
    AttachmentUpsert {
        /// Attachment primary key.
        attachment_id: [u8; 16],
        /// Note this attachment belongs to.
        note_id: [u8; 16],
        /// Original filename.
        filename: String,
        /// MIME type.
        mime_type: String,
        /// File size in bytes.
        size_bytes: u64,
        /// blake3 hash of the file contents (used as the blob-store key).
        content_blake3: [u8; 32],
    },
    /// Delete an attachment metadata row.
    AttachmentDelete {
        /// Attachment being deleted.
        attachment_id: [u8; 16],
    },
}

impl OpPayload {
    /// The discriminator for this payload.
    pub fn kind(&self) -> OpKind {
        match self {
            OpPayload::NoteUpsert { .. } => OpKind::NoteUpsert,
            OpPayload::NoteDelete { .. } => OpKind::NoteDelete,
            OpPayload::BlockUpsert { .. } => OpKind::BlockUpsert,
            OpPayload::BlockMove { .. } => OpKind::BlockMove,
            OpPayload::BlockDelete { .. } => OpKind::BlockDelete,
            OpPayload::AttachmentUpsert { .. } => OpKind::AttachmentUpsert,
            OpPayload::AttachmentDelete { .. } => OpKind::AttachmentDelete,
        }
    }
}

/// An op encoded for the wire or the oplog.
///
/// Carries timestamping, schema version, content hash for dedup, optional
/// transaction grouping, and the mutation payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedOp {
    /// HLC timestamp authored by the producer device.
    pub hlc: HlcTimestamp,
    /// Schema version under which this op was produced.
    pub schema_version: u32,
    /// Hash of (hlc, schema_version, payload). Used for idempotency.
    pub content_hash: ContentHash,
    /// Optional grouping for ops that should be applied atomically.
    pub txn_id: Option<[u8; 16]>,
    /// The mutation.
    pub payload: OpPayload,
}

impl EncodedOp {
    /// Build a new op stamped with the given metadata. Computes the
    /// content hash from the canonical encoding of (hlc, schema_version,
    /// payload).
    pub fn new(
        hlc: HlcTimestamp,
        schema_version: u32,
        payload: OpPayload,
        txn_id: Option<[u8; 16]>,
    ) -> SyncResult<Self> {
        let content_hash = compute_content_hash(&hlc, schema_version, &payload)?;
        Ok(EncodedOp {
            hlc,
            schema_version,
            content_hash,
            txn_id,
            payload,
        })
    }

    /// Recompute and verify the content hash against the current payload.
    /// Returns Err if the hash does not match (tamper detection on apply).
    pub fn verify_hash(&self) -> SyncResult<()> {
        let recomputed = compute_content_hash(&self.hlc, self.schema_version, &self.payload)?;
        if recomputed == self.content_hash {
            Ok(())
        } else {
            Err(SyncError::Protocol(format!(
                "content_hash mismatch: declared={} computed={}",
                self.content_hash.to_hex(),
                recomputed.to_hex(),
            )))
        }
    }
}

/// Compute the canonical content hash for an op.
///
/// Inputs are postcard-serialized `(hlc, schema_version, payload)`. Order
/// matters and must remain stable across implementations; this function is
/// the canonical reference.
pub fn compute_content_hash(
    hlc: &HlcTimestamp,
    schema_version: u32,
    payload: &OpPayload,
) -> SyncResult<ContentHash> {
    // We hash the postcard encoding of a tuple so the hash is stable across
    // any code path that builds an EncodedOp the same way.
    let bytes = postcard::to_allocvec(&(hlc, schema_version, payload))?;
    let hash = blake3::hash(&bytes);
    Ok(ContentHash(*hash.as_bytes()))
}
