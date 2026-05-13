//! Postcard wire format.

pub mod envelope;

pub use envelope::SyncEnvelope;

use crate::error::SyncResult;
use crate::oplog::op::EncodedOp;

/// Encode an op for the wire.
pub fn encode_op(op: &EncodedOp) -> SyncResult<Vec<u8>> {
    Ok(postcard::to_allocvec(op)?)
}

/// Decode an op from the wire.
pub fn decode_op(bytes: &[u8]) -> SyncResult<EncodedOp> {
    Ok(postcard::from_bytes(bytes)?)
}

/// Encode a batch of ops as a single postcard-serialized blob.
///
/// This is the shape that becomes `SyncEnvelope::ciphertext` in Phase 2
/// (after AEAD wraps it). Phase 1 carries it in cleartext for the loopback
/// transport.
pub fn encode_op_batch(ops: &[EncodedOp]) -> SyncResult<Vec<u8>> {
    Ok(postcard::to_allocvec(&ops.to_vec())?)
}

/// Decode a batch of ops produced by [`encode_op_batch`].
pub fn decode_op_batch(bytes: &[u8]) -> SyncResult<Vec<EncodedOp>> {
    Ok(postcard::from_bytes(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::DeviceId;
    use crate::hlc::HlcTimestamp;
    use crate::oplog::op::OpPayload;

    fn note_upsert(note_id_byte: u8, title: &str) -> OpPayload {
        OpPayload::NoteUpsert {
            note_id: [note_id_byte; 16],
            display_alias: Some("alias".to_string()),
            title: title.to_string(),
            created_at_millis: 0,
        }
    }

    #[test]
    fn op_roundtrip_note_upsert() {
        let dev = DeviceId::new_random();
        let hlc = HlcTimestamp { ntp64: 1, device: dev };
        let op = EncodedOp::new(hlc, 1, note_upsert(0xab, "Hello"), None).unwrap();
        let bytes = encode_op(&op).unwrap();
        let back = decode_op(&bytes).unwrap();
        assert_eq!(op, back);
        back.verify_hash().expect("hash verifies after roundtrip");
    }

    #[test]
    fn op_roundtrip_block_upsert() {
        let dev = DeviceId::new_random();
        let hlc = HlcTimestamp { ntp64: 5, device: dev };
        let payload = OpPayload::BlockUpsert {
            block_id: [0xcd; 16],
            note_id: [0xab; 16],
            parent_block_id: Some([0xef; 16]),
            order_key: "a3".to_string(),
            indent_level: 2,
            text: "First child block".to_string(),
        };
        let op = EncodedOp::new(hlc, 1, payload, None).unwrap();
        let bytes = encode_op(&op).unwrap();
        let back = decode_op(&bytes).unwrap();
        assert_eq!(op, back);
        back.verify_hash().unwrap();
    }

    #[test]
    fn op_batch_roundtrip() {
        let dev = DeviceId::new_random();
        let ops: Vec<EncodedOp> = (0..5)
            .map(|i| {
                let hlc = HlcTimestamp {
                    ntp64: i as u64,
                    device: dev,
                };
                EncodedOp::new(hlc, 1, note_upsert(i as u8, &format!("Note {i}")), None).unwrap()
            })
            .collect();
        let bytes = encode_op_batch(&ops).unwrap();
        let back = decode_op_batch(&bytes).unwrap();
        assert_eq!(ops, back);
    }

    #[test]
    fn distinct_payloads_have_distinct_hashes() {
        let dev = DeviceId::new_random();
        let hlc = HlcTimestamp { ntp64: 1, device: dev };
        let a = EncodedOp::new(hlc, 1, note_upsert(1, "A"), None).unwrap();
        let b = EncodedOp::new(hlc, 1, note_upsert(2, "B"), None).unwrap();
        assert_ne!(a.content_hash, b.content_hash);
    }
}
