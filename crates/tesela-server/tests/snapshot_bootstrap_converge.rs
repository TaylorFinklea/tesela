//! Snapshot-bootstrap convergence (Part D, task #146).
//!
//! Proves the server-side half of the multi-device convergence fix: when a
//! device imports the server's per-note Loro doc as a SHARED BASE before it
//! authors locally, concurrent edits to different blocks converge — each
//! block renders exactly once, both edits carry correct text, no duplicate
//! "twin" bullets (spec invariant 3).
//!
//! This is an ENGINE-LEVEL test in the server crate (not a full HTTP-layer
//! test): it obtains the note snapshot the exact way the `GET
//! /loro/notes/{id}/snapshot` handler (`get_loro_snapshot`) does — derive
//! the note id with `stable_uuid_from_slug` (blake3-truncate the slug),
//! then `export_doc_update(note_id, None)` — then drives the bootstrap →
//! edit → merge path the iOS `bootstrapNoteIfNeeded` + `recordNoteDiff`
//! flow exercises. A full server-spawning HTTP round-trip is impractical
//! with the existing harness (no resident-doc seam over HTTP), and the
//! spec permits the engine-level path that proves the same export → import
//! → edit → merge convergence.

use std::sync::Arc;

use tesela_sync::{DeviceId, Hlc, LoroEngine, OpPayload, SyncEngine};

const ALPHA_BID: [u8; 16] = [0x01; 16];
const BETA_BID: [u8; 16] = [0x02; 16];

fn engine(device_byte: u8) -> LoroEngine {
    let device = DeviceId::from_bytes([device_byte; 16]);
    let hlc = Arc::new(Hlc::new(device));
    LoroEngine::new(device, hlc)
}

/// Mirror of `tesela_server::routes::notes::stable_uuid_from_slug` (the
/// derivation the snapshot endpoint uses to address a note's doc): blake3 of
/// the slug, truncated to 16 bytes.
fn stable_uuid_from_slug(slug: &str) -> [u8; 16] {
    let hash = blake3::hash(slug.as_bytes());
    let mut out = [0u8; 16];
    out.copy_from_slice(&hash.as_bytes()[..16]);
    out
}

fn seed_body() -> String {
    "- alpha <!-- bid:01010101-0101-0101-0101-010101010101 -->\n\
     - beta <!-- bid:02020202-0202-0202-0202-020202020202 -->\n"
        .to_string()
}

fn bid_bullet_count(render: &str, bid_hex_dashed: &str) -> usize {
    render
        .lines()
        .filter(|l| l.contains(&format!("bid:{bid_hex_dashed}")))
        .count()
}

/// Server seeds a note; the device bootstraps from the server's snapshot
/// (obtained the way `get_loro_snapshot` builds it), edits beta, ships its
/// snapshot back; the server edits alpha. The merged server render must have
/// no twins and both edits' text — the shared base is what makes it converge.
#[tokio::test]
async fn snapshot_bootstrap_converges_without_twins() {
    let slug = "shared-base-note";
    // Address the doc exactly as the snapshot endpoint does.
    let note_id = stable_uuid_from_slug(slug);

    let server = engine(0x11);
    let device = engine(0x22);

    // Server seeds the note — the authoritative base the endpoint serves.
    server
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some(slug.into()),
            title: "Note".into(),
            content: seed_body(),
            created_at_millis: 1,
        })
        .await
        .expect("server seed NoteUpsert");

    // `get_loro_snapshot` path: full snapshot of the addressed doc.
    let snapshot = server
        .export_doc_update(note_id, None)
        .await
        .expect("server snapshot export (get_loro_snapshot path)");

    // Device bootstraps from that snapshot BEFORE authoring (the
    // `bootstrapNoteIfNeeded` -> `importNoteSnapshot` -> `import_doc_update`
    // path). Its tree now carries the SERVER's TreeIDs for alpha/beta.
    device
        .import_doc_update(note_id, &snapshot)
        .await
        .expect("device import shared base");

    // Idempotent import (invariant 4): re-importing the same base is a no-op.
    device
        .import_doc_update(note_id, &snapshot)
        .await
        .expect("device re-import shared base (idempotent)");

    // Device edit on the shared base: beta -> "beta from device". Because the
    // base is resident, this BlockUpsert resolves to the server's beta node
    // instead of minting a rival TreeID.
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: BETA_BID,
            note_id,
            parent_block_id: None,
            order_key: "a1".into(),
            indent_level: 0,
            text: "beta from device".into(),
        })
        .await
        .expect("device BlockUpsert beta");

    // Device ships its snapshot back; the server imports it (inbound delta).
    let device_snapshot = device
        .export_doc_update(note_id, None)
        .await
        .expect("device snapshot export");
    server
        .import_doc_update(note_id, &device_snapshot)
        .await
        .expect("server import device snapshot");

    // Concurrent server edit: alpha -> "alpha EDITED".
    server
        .record_local(OpPayload::BlockUpsert {
            block_id: ALPHA_BID,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "alpha EDITED".into(),
        })
        .await
        .expect("server BlockUpsert alpha");

    let merged = server
        .render_note(note_id)
        .await
        .expect("server render after converge");
    eprintln!("=== SNAPSHOT-BOOTSTRAP CONVERGED RENDER ===\n{merged}\n=== END ===");

    // Invariant 3: each block renders exactly once (no duplicate-bid twins).
    let alpha_bullets = bid_bullet_count(&merged, "01010101-0101-0101-0101-010101010101");
    let beta_bullets = bid_bullet_count(&merged, "02020202-0202-0202-0202-020202020202");
    assert_eq!(
        alpha_bullets, 1,
        "expected alpha bid exactly once (no twin), got {alpha_bullets}; render:\n{merged}"
    );
    assert_eq!(
        beta_bullets, 1,
        "expected beta bid exactly once (no twin), got {beta_bullets}; render:\n{merged}"
    );

    // Both edits survive with correct text.
    assert_eq!(
        merged.matches("alpha EDITED").count(),
        1,
        "expected 'alpha EDITED' exactly once; render:\n{merged}"
    );
    assert_eq!(
        merged.matches("beta from device").count(),
        1,
        "expected 'beta from device' exactly once; render:\n{merged}"
    );

    // No stale pre-edit ghost text survived the merge.
    let stale_alpha = merged
        .lines()
        .filter(|l| {
            let t = l.trim_start_matches([' ', '\t', '-']).trim();
            t.starts_with("alpha ") && !t.starts_with("alpha EDITED")
        })
        .count();
    assert_eq!(
        stale_alpha, 0,
        "stale pre-edit alpha text survived the merge; render:\n{merged}"
    );
}
