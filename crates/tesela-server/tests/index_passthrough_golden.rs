//! P1.12 — index passthrough golden test.
//!
//! Proves the migration invariant: a block whose properties live in the sync
//! engine's typed `props`/`prop_keys` containers indexes to the SAME
//! `block_properties` rows as the equivalent LEGACY in-text
//! `- text\n  key:: value` markdown. The index is downstream of the
//! MATERIALIZED markdown (`render_note_full` → `parse_blocks` →
//! `index_block_properties`), so this exercises materialize → parse → index
//! end-to-end with NO SQLite schema change and NO container → SQLite reader.
//!
//! Trust artifact: properties set through `OpPayload::BlockPropertySet` are
//! indistinguishable, at the index, from properties that were authored as
//! plain `key:: value` continuation lines. The container is the source of
//! truth; the materialized markdown is the index's only input. If the
//! materializer ever drifted from the legacy `key:: value` shape (wrong key
//! casing, wrong value formatting, dropped multi-value member, reordered
//! lines that shift a block's `{note_id}:{line}` id), this test fails.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use tesela_core::note::{Note, NoteId, NoteMetadata};
use tesela_core::traits::search_index::SearchIndex;
use tesela_core::SqliteIndex;
use tesela_sync::hlc::Hlc;
use tesela_sync::{DeviceId, LoroEngine, OpPayload, PropOp, PropScalar, SyncEngine};

/// The block's canonical id. The `[u8; 16]` filled with `0x07` is the on-wire
/// `block_id`; hex-encoded with dashes it is the same uuid the seed body
/// stamps as `<!-- bid:... -->`, so the engine keys its node to an id the
/// property op resolves.
const BID_BYTE: u8 = 0x07;
const BID_STR: &str = "07070707-0707-0707-0707-070707070707";

/// Both notes index under the SAME `NoteId` (into SEPARATE in-memory indexes)
/// so `parse_blocks`' `{note_id}:{line}` block ids line up for a direct
/// row-level comparison.
const NOTE_ID: &str = "passthrough-note";

fn make_note(body: &str) -> Note {
    Note {
        id: NoteId::new(NOTE_ID),
        title: "Passthrough".to_string(),
        content: body.to_string(),
        body: body.to_string(),
        metadata: NoteMetadata {
            title: None,
            tags: vec![],
            aliases: vec![],
            note_type: None,
            custom: Default::default(),
            created: None,
            modified: None,
        },
        path: PathBuf::from(format!("notes/{NOTE_ID}.md")),
        checksum: format!("checksum-{NOTE_ID}"),
        created_at: chrono::Utc::now(),
        modified_at: chrono::Utc::now(),
        attachments: vec![],
    }
}

/// Index `body` through the REAL index path and return the `block_properties`
/// rows for the `#Task` block as a `(block_id, property_name) → value` map.
/// `get_typed_blocks` reads the indexed rows back out of `block_properties`,
/// so the returned map IS the indexed-row content keyed by block id.
async fn indexed_rows(body: &str) -> BTreeMap<(String, String), String> {
    let index = SqliteIndex::open_in_memory().await.unwrap();
    index.reindex(&make_note(body)).await.unwrap();

    let blocks = index.get_typed_blocks("Task").await.unwrap();
    let mut rows = BTreeMap::new();
    for block in &blocks {
        for (key, value) in &block.properties {
            rows.insert((block.id.clone(), key.clone()), value.clone());
        }
    }
    rows
}

/// Materialize a note whose properties live in typed containers (set via
/// `BlockPropertySet`), then return the markdown body so it can be fed to the
/// real index path exactly like a legacy on-disk note.
async fn materialized_body_with_container_props() -> String {
    let dev = DeviceId::from_bytes([1u8; 16]);
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let note = [0x42u8; 16];
    let block = [BID_BYTE; 16];

    // Body-only note (no frontmatter) so `render_note_full` == body, and the
    // `#Task` tag is inline prose (NOT a `tags::` property line — keeping the
    // compared rows to the genuine properties).
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("passthrough".into()),
            title: "Passthrough".into(),
            content: format!("- finish report #Task <!-- bid:{BID_STR} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // A single scalar property through the typed container.
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();

    // A second scalar of a different value_type (number) — proves canonical
    // scalar → string formatting (no trailing-zero / quoting drift).
    engine
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: block,
            key: "priority".into(),
            value: PropOp::SetScalar(PropScalar::Int(3)),
        })
        .await
        .unwrap();

    engine
        .render_note_full(note)
        .await
        .expect("note materializes")
}

/// The golden assertion: container-sourced properties materialize to markdown
/// that indexes to the SAME `block_properties` rows as the legacy in-text form.
#[tokio::test(flavor = "current_thread")]
async fn container_props_index_to_same_rows_as_legacy_intext_markdown() {
    let materialized = materialized_body_with_container_props().await;

    // Guard (fails "for the right reason" if the engine ever stops
    // materializing container props as `key:: value` continuation lines).
    assert!(
        materialized.contains("status:: doing") && materialized.contains("priority:: 3"),
        "container props must materialize as `key:: value` lines; got:\n{materialized}"
    );

    // The hand-authored legacy equivalent: same prose + same `key:: value`
    // continuation lines. The bid comment is stripped by `parse_blocks`, so
    // the block lands at the same `{note_id}:0` id in both forms.
    let legacy = format!(
        "- finish report #Task <!-- bid:{BID_STR} -->\n  status:: doing\n  priority:: 3\n"
    );

    let from_containers = indexed_rows(&materialized).await;
    let from_legacy = indexed_rows(&legacy).await;

    assert!(
        !from_legacy.is_empty(),
        "legacy markdown must index at least one block property row"
    );
    assert_eq!(
        from_containers, from_legacy,
        "container-sourced props must index to the SAME block_properties rows \
         (block_id / property_name / value) as the legacy in-text markdown;\n\
         materialized body was:\n{materialized}"
    );
}

/// Multi-value passthrough: a `LoroList` property (union-merge container)
/// materializes comma-joined per the `tags::` convention and indexes to the
/// SAME row as the legacy `key:: a, b` in-text form.
#[tokio::test(flavor = "current_thread")]
async fn multi_value_container_prop_indexes_to_same_row_as_legacy() {
    let dev = DeviceId::from_bytes([2u8; 16]);
    let engine = LoroEngine::new(dev, Arc::new(Hlc::new(dev)));
    let note = [0x43u8; 16];
    let block = [BID_BYTE; 16];

    engine
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("passthrough-multi".into()),
            title: "PassthroughMulti".into(),
            content: format!("- triage #Task <!-- bid:{BID_STR} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    for member in ["alpha", "beta"] {
        engine
            .record_local(OpPayload::BlockPropertySet {
                note_id: note,
                block_id: block,
                key: "labels".into(),
                value: PropOp::AddToList(PropScalar::Text(member.into())),
            })
            .await
            .unwrap();
    }

    let materialized = engine.render_note_full(note).await.expect("materializes");
    assert!(
        materialized.contains("labels::"),
        "multi-value prop must materialize a `labels::` line; got:\n{materialized}"
    );

    // The legacy equivalent uses the same comma-joined value the materializer
    // emits, so both index to one identical `labels` row.
    let labels_line = materialized
        .lines()
        .find(|l| l.trim_start().starts_with("labels::"))
        .expect("labels line");
    let legacy = format!("- triage #Task <!-- bid:{BID_STR} -->\n{labels_line}\n");

    let from_containers = indexed_rows(&materialized).await;
    let from_legacy = indexed_rows(&legacy).await;

    assert_eq!(
        from_containers, from_legacy,
        "multi-value container prop must index to the SAME row as the legacy \
         comma-joined in-text form; materialized body was:\n{materialized}"
    );
    assert!(
        from_containers
            .iter()
            .any(|((_, key), value)| key == "labels" && value.contains("alpha") && value.contains("beta")),
        "both list members must survive into the indexed row; got: {from_containers:?}"
    );
}
