//! Postcard wire format.

pub mod envelope;

pub use envelope::SyncEnvelope;

use crate::error::SyncResult;
use crate::oplog::op::EncodedOp;
use serde::{Deserialize, Serialize};

/// One doc's Loro update bytes for relay broadcast — the Loro-cutover
/// (protocol v2) relay payload unit. `doc` is the 16-byte note id;
/// `update_bytes` is `LoroDoc::export(ExportMode::updates(&since_vv))`.
/// The index doc is NOT broadcast — each peer rebuilds it locally from
/// the per-note docs it receives (the self-healing index), so this only
/// ever carries note docs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoroDocUpdate {
    /// 16-byte note id the update belongs to.
    pub doc: [u8; 16],
    /// Loro update bytes (a delta export, or full state for bootstrap).
    pub update_bytes: Vec<u8>,
}

/// Magic + version prefix for the Loro relay payload (protocol v2).
///
/// The legacy v1 payload is a bare `postcard(Vec<EncodedOp>)` with NO
/// prefix. This 4-byte magic is deliberately not a small postcard varint
/// length, so a v1 payload can never be mistaken for a v2 one (and vice
/// versa). A downlevel peer reading a v2 payload as v1 — or an
/// authoritative peer reading a v1 payload — detects the mismatch and
/// skips rather than corrupting state. The cutover is a flag-day (all
/// participants move to v2 at once); the magic is the defensive backstop
/// for the transition window.
pub const LORO_RELAY_MAGIC: [u8; 4] = *b"TLR2";

/// Encode a batch of per-doc Loro updates as the v2 relay plaintext
/// (`LORO_RELAY_MAGIC` ++ `postcard(Vec<LoroDocUpdate>)`). The result is
/// what becomes `SyncEnvelope::ciphertext` before AEAD sealing.
pub fn encode_loro_relay_payload(updates: &[LoroDocUpdate]) -> SyncResult<Vec<u8>> {
    let body = postcard::to_allocvec(&updates.to_vec())?;
    let mut out = Vec::with_capacity(LORO_RELAY_MAGIC.len() + body.len());
    out.extend_from_slice(&LORO_RELAY_MAGIC);
    out.extend_from_slice(&body);
    Ok(out)
}

/// Decode a v2 relay plaintext produced by [`encode_loro_relay_payload`].
/// Returns `Ok(None)` when the bytes lack the v2 magic (a legacy v1
/// payload or foreign data) so the caller can skip it without erroring.
pub fn decode_loro_relay_payload(bytes: &[u8]) -> SyncResult<Option<Vec<LoroDocUpdate>>> {
    if bytes.len() < LORO_RELAY_MAGIC.len() || bytes[..LORO_RELAY_MAGIC.len()] != LORO_RELAY_MAGIC {
        return Ok(None);
    }
    Ok(Some(postcard::from_bytes(&bytes[LORO_RELAY_MAGIC.len()..])?))
}

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
            content: String::new(),
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

    #[test]
    fn loro_relay_payload_round_trips() {
        let updates = vec![
            LoroDocUpdate { doc: [0x11; 16], update_bytes: b"alpha".to_vec() },
            LoroDocUpdate { doc: [0x22; 16], update_bytes: vec![] },
        ];
        let bytes = encode_loro_relay_payload(&updates).unwrap();
        assert_eq!(&bytes[..4], &LORO_RELAY_MAGIC, "magic prefix present");
        let back = decode_loro_relay_payload(&bytes).unwrap().expect("v2 payload");
        assert_eq!(back, updates);
    }

    #[test]
    fn legacy_v1_payload_is_not_misread_as_loro_v2() {
        // A bare postcard(Vec<EncodedOp>) (the v1 wire) must decode to
        // None on the v2 path, so an authoritative peer skips it instead
        // of corrupting state.
        let dev = DeviceId::new_random();
        let ops: Vec<EncodedOp> = (0..3)
            .map(|i| {
                let hlc = HlcTimestamp { ntp64: i, device: dev };
                EncodedOp::new(hlc, 1, note_upsert(i as u8, "x"), None).unwrap()
            })
            .collect();
        let v1 = encode_op_batch(&ops).unwrap();
        assert!(
            decode_loro_relay_payload(&v1).unwrap().is_none(),
            "v1 payload must not be mistaken for v2"
        );
    }

    #[test]
    fn empty_and_short_payloads_decode_to_none() {
        assert!(decode_loro_relay_payload(&[]).unwrap().is_none());
        assert!(decode_loro_relay_payload(b"TL").unwrap().is_none());
        assert!(decode_loro_relay_payload(b"XXXX").unwrap().is_none());
    }
}
