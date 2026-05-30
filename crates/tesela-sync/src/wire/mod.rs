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

/// Encode a batch of per-doc Loro updates as the v2 relay plaintext:
/// `LORO_RELAY_MAGIC` ++ `deflate(postcard(Vec<LoroDocUpdate>))`. The
/// result is what becomes `SyncEnvelope::ciphertext` before AEAD sealing.
/// DEFLATE keeps large notes (Loro snapshots are text-heavy, ~3-4× ratio)
/// under the relay body limit — a 1.3 MB markdown note's ~5 MB snapshot
/// compresses to ~1.5 MB, fitting the 5 MB envelope.
pub fn encode_loro_relay_payload(updates: &[LoroDocUpdate]) -> SyncResult<Vec<u8>> {
    let body = postcard::to_allocvec(&updates.to_vec())?;
    let compressed = miniz_oxide::deflate::compress_to_vec(&body, 6);
    let mut out = Vec::with_capacity(LORO_RELAY_MAGIC.len() + compressed.len());
    out.extend_from_slice(&LORO_RELAY_MAGIC);
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// Decode a v2 relay plaintext produced by [`encode_loro_relay_payload`]
/// (DEFLATE-inflate then postcard). Returns `Ok(None)` when the bytes lack
/// the v2 magic (a legacy v1 payload or foreign data) so the caller can
/// skip it without erroring.
pub fn decode_loro_relay_payload(bytes: &[u8]) -> SyncResult<Option<Vec<LoroDocUpdate>>> {
    if bytes.len() < LORO_RELAY_MAGIC.len() || bytes[..LORO_RELAY_MAGIC.len()] != LORO_RELAY_MAGIC {
        return Ok(None);
    }
    let body = miniz_oxide::inflate::decompress_to_vec(&bytes[LORO_RELAY_MAGIC.len()..])
        .map_err(|e| crate::error::SyncError::Other(format!("loro relay inflate: {e:?}")))?;
    Ok(Some(postcard::from_bytes(&body)?))
}

/// The relay's `PUT /ops` body limit. Mirrors `tesela-relay`'s `--max-body`
/// default. A single Loro doc cannot be split across envelopes, so this MUST
/// exceed the wire size of the largest single note's snapshot. The biggest real
/// note here (`ai-business`, 1.3 MB markdown) is a ~5 MB Loro snapshot ≈ 7 MB on
/// the wire, so the cap is set generously at 16 MiB — the relay only ever stores
/// rate-limited ciphertext, so a large body cap is cheap. Producers MUST keep
/// each PUT's wire body at or under this; operators must run `tesela-relay`
/// with `--max-body`/`TESELA_RELAY_MAX_BODY` at least this large.
pub const RELAY_MAX_BODY_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Per-PUT *plaintext* budget for relay broadcast, kept comfortably under
/// [`RELAY_MAX_BODY_BYTES`]. The wire body is
/// `base64(aead_seal(deflate(postcard(batch))))` in a tiny JSON wrapper:
/// DEFLATE only ever shrinks the plaintext, base64 inflates it ×4/3, and the
/// AEAD nonce/tag + JSON keys add a few hundred bytes. So worst-case
/// (incompressible) wire ≈ plaintext × 4/3 + overhead. An 8 MiB plaintext budget
/// lands ≤ ~11.2 MiB on the wire even with zero compression — well under the
/// 16 MiB cap. The canonical-device bootstrap broadcasts every note's FULL
/// state at once; this chunking keeps each multi-note envelope under the limit
/// (413) while a single oversized note rides alone. See
/// [`pack_loro_relay_batches`].
///
/// NOTE: a single note whose snapshot exceeds the relay cap still can't ship
/// (no intra-doc chunking yet). Today the largest snapshot (~5 MB) fits the
/// 16 MiB cap with room to spare; a note that outgrew the cap would need
/// intra-doc chunking. Tracked as a follow-up.
pub const MAX_RELAY_PLAINTEXT_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Greedily pack per-note updates (`(note_id, update_bytes, captured_vv)`,
/// from `produce_relay_updates`) into batches whose summed `update_bytes`
/// stay under `max_bytes`, so each relay PUT fits the body limit. A single
/// update larger than the budget gets its own batch (best effort). Each
/// returned batch is `(payload, committed_cursors)`: send the payload, and
/// on a confirmed PUT commit that batch's cursors. Sending stops at the
/// first failed PUT, leaving later batches' cursors uncommitted so they
/// re-produce next tick.
#[allow(clippy::type_complexity)]
pub fn pack_loro_relay_batches(
    updates: Vec<([u8; 16], Vec<u8>, Vec<u8>)>,
    max_bytes: usize,
) -> Vec<(Vec<LoroDocUpdate>, Vec<([u8; 16], Vec<u8>)>)> {
    let mut batches = Vec::new();
    let mut payload: Vec<LoroDocUpdate> = Vec::new();
    let mut committed: Vec<([u8; 16], Vec<u8>)> = Vec::new();
    let mut bytes_acc = 0usize;
    for (doc, update_bytes, vv) in updates {
        let sz = update_bytes.len();
        if !payload.is_empty() && bytes_acc + sz > max_bytes {
            batches.push((std::mem::take(&mut payload), std::mem::take(&mut committed)));
            bytes_acc = 0;
        }
        payload.push(LoroDocUpdate { doc, update_bytes });
        committed.push((doc, vv));
        bytes_acc += sz;
    }
    if !payload.is_empty() {
        batches.push((payload, committed));
    }
    batches
}

/// Encode an op for the wire.
pub fn encode_op(op: &EncodedOp) -> SyncResult<Vec<u8>> {
    Ok(postcard::to_allocvec(op)?)
}

/// Decode an op from the wire.
pub fn decode_op(bytes: &[u8]) -> SyncResult<EncodedOp> {
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
        // A bare postcard(Vec<EncodedOp>) (the retired v1 wire) must decode
        // to None on the v2 path, so an authoritative peer skips a stray
        // legacy / foreign envelope instead of corrupting state.
        let dev = DeviceId::new_random();
        let ops: Vec<EncodedOp> = (0..3)
            .map(|i| {
                let hlc = HlcTimestamp { ntp64: i, device: dev };
                EncodedOp::new(hlc, 1, note_upsert(i as u8, "x"), None).unwrap()
            })
            .collect();
        let v1 = postcard::to_allocvec(&ops).unwrap();
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

    #[test]
    fn pack_batches_splits_under_budget_and_preserves_all() {
        // 5 updates of 100 bytes each, budget 250 → batches of [100,100],
        // [100,100], [100] (a batch may exceed budget only by its first
        // item). Every note + cursor is preserved exactly once.
        let updates: Vec<([u8; 16], Vec<u8>, Vec<u8>)> = (0u8..5)
            .map(|i| ([i; 16], vec![i; 100], vec![i; 8]))
            .collect();
        let batches = pack_loro_relay_batches(updates, 250);
        assert_eq!(batches.len(), 3, "5×100 @250 → 3 batches");
        let total: usize = batches.iter().map(|(p, _)| p.len()).sum();
        assert_eq!(total, 5, "all updates preserved");
        for (payload, committed) in &batches {
            assert_eq!(payload.len(), committed.len(), "payload/cursor aligned");
            let sz: usize = payload.iter().map(|u| u.update_bytes.len()).sum();
            // Each batch ≤ budget OR a single oversized item.
            assert!(sz <= 250 || payload.len() == 1, "batch within budget: {sz}");
        }
    }

    #[test]
    fn pack_batches_oversized_single_gets_own_batch() {
        let updates = vec![
            ([1u8; 16], vec![0u8; 10], vec![1u8; 4]),
            ([2u8; 16], vec![0u8; 5_000], vec![2u8; 4]), // bigger than budget
            ([3u8; 16], vec![0u8; 10], vec![3u8; 4]),
        ];
        let batches = pack_loro_relay_batches(updates, 1_000);
        assert_eq!(batches.len(), 3, "small, oversized-alone, small");
        assert_eq!(batches[1].0.len(), 1);
        assert_eq!(batches[1].0[0].doc, [2u8; 16]);
    }

    #[test]
    fn pack_batches_empty_input_is_empty() {
        assert!(pack_loro_relay_batches(Vec::new(), 1_000).is_empty());
    }

    #[test]
    fn plaintext_budget_stays_under_relay_body_limit() {
        // Worst case: incompressible plaintext, base64 ×4/3, plus headroom for
        // the AEAD nonce/tag + JSON wrapper. A full-budget batch must never
        // 413. Locks the two constants against drifting apart (the original
        // bug: a 2.5 MB budget aimed at a 5 MiB relay, deployed at 1 MiB).
        let worst_case_wire = MAX_RELAY_PLAINTEXT_BYTES * 4 / 3 + 4096;
        assert!(
            worst_case_wire <= RELAY_MAX_BODY_BYTES,
            "plaintext budget {MAX_RELAY_PLAINTEXT_BYTES} → ~{worst_case_wire} B wire \
             exceeds relay limit {RELAY_MAX_BODY_BYTES}"
        );
    }

    #[test]
    fn full_budget_batch_encodes_under_relay_limit() {
        // Build a single batch at the plaintext budget out of INCOMPRESSIBLE
        // bytes (an LCG stream DEFLATE can't shrink), run the real encoder, and
        // confirm the base64'd wire body (+ generous AEAD/JSON overhead) fits
        // the relay cap. Exercises the actual compression+framing path, not
        // just the arithmetic estimate.
        let mut lcg: u32 = 0x1234_5678;
        let bytes: Vec<u8> = (0..MAX_RELAY_PLAINTEXT_BYTES)
            .map(|_| {
                lcg = lcg.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (lcg >> 24) as u8
            })
            .collect();
        let updates = vec![LoroDocUpdate {
            doc: [7u8; 16],
            update_bytes: bytes,
        }];
        let encoded = encode_loro_relay_payload(&updates).unwrap();
        // Wire body = base64(aead_seal(encoded)) in a small JSON wrapper:
        // base64 = ×4/3; AEAD nonce+tag ≈ 40 B; JSON keys + device hex ≈ 256 B.
        let wire_estimate = encoded.len() * 4 / 3 + 40 + 256;
        assert!(
            wire_estimate <= RELAY_MAX_BODY_BYTES,
            "full-budget incompressible batch → ~{wire_estimate} B wire exceeds {RELAY_MAX_BODY_BYTES}"
        );
    }
}
