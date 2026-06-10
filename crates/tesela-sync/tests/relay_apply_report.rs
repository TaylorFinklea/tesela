//! A4: `apply_relay_updates` must surface per-note results instead of
//! silently swallowing failures behind an `.is_ok()` filter. Callers (the
//! server relay tick, the server WS inbound, the FFI tick) need to know
//! WHICH note failed (to hold the cursor / queue a snapshot catch-up) and
//! which import was left PENDING by Loro (causal gap → targeted catch-up).

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

fn engine(b: u8) -> LoroEngine {
    let d = DeviceId::from_bytes([b; 16]);
    LoroEngine::new(d, Arc::new(Hlc::new(d)))
}

#[tokio::test]
async fn apply_relay_updates_reports_per_note_failures() {
    let note_good = [0x6e; 16];
    let note_bad = [0x7f; 16];

    let author = engine(0x11);
    author
        .record_local(OpPayload::NoteUpsert {
            note_id: note_good,
            display_alias: Some("good".into()),
            title: "good".into(),
            content: "- hello good\n".into(),
            created_at_millis: 1,
        })
        .await
        .expect("seed");
    let good_snap = author
        .export_doc_update(note_good, None)
        .await
        .expect("snapshot");

    let consumer = engine(0x22);
    let report = consumer
        .apply_relay_updates(&[
            (note_bad, b"definitely not a loro update".to_vec()),
            (note_good, good_snap),
        ])
        .await;

    assert_eq!(report.applied, vec![note_good], "the good update applied");
    assert!(report.pending.is_empty(), "nothing pending: {report:?}");
    assert_eq!(report.failed.len(), 1, "the bad update surfaced: {report:?}");
    assert_eq!(report.failed[0].0, note_bad, "failure names the note");
    assert!(
        !report.failed[0].1.is_empty(),
        "failure carries the error message"
    );
    assert_eq!(report.applied_count(), 1);

    // The good note really did land despite the bad sibling.
    let rendered = consumer.render_note(note_good).await.unwrap_or_default();
    assert!(rendered.contains("hello good"), "good note applied: {rendered:?}");
}

#[tokio::test]
async fn apply_relay_updates_reports_pending_for_causal_gap() {
    let note = [0x8a; 16];

    let author = engine(0x11);
    author
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("gap".into()),
            title: "gap".into(),
            content: "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n".into(),
            created_at_millis: 1,
        })
        .await
        .expect("seed");
    let pre_vv = author.doc_version(note).await.expect("pre vv");
    author
        .record_local(OpPayload::BlockUpsert {
            block_id: [0x99; 16],
            note_id: note,
            parent_block_id: None,
            order_key: "z".into(),
            indent_level: 0,
            text: "tail edit".into(),
            after_block_id: None,
        })
        .await
        .expect("edit");
    let tail = author
        .export_doc_update(note, Some(&pre_vv))
        .await
        .expect("tail delta");

    // A device with NO base imports the tail: Loro buffers it as pending —
    // the report must say so instead of counting it as cleanly applied.
    let device = engine(0x22);
    let report = device.apply_relay_updates(&[(note, tail)]).await;

    assert_eq!(report.pending, vec![note], "causal gap surfaced as pending");
    assert!(report.failed.is_empty(), "a pending import is not a failure");
    assert!(report.applied.is_empty(), "not reported as cleanly applied");
    assert_eq!(
        report.applied_count(),
        1,
        "pending still counts as applied for observability parity"
    );
}
