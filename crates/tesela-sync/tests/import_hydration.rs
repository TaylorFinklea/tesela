use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use std::time::Duration;

use tempfile::TempDir;
use tesela_core::import_logseq::{apply_plan_with_writer, build_plan, ApplyDecisions, PlanKind};
use tesela_core::stable_uuid_from_slug;
use tesela_sync::{hydrate_note, DeviceId, EngineImportNoteWriter, Hlc, LoroEngine, SyncEngine};

fn structural_projection(
    content: &str,
) -> (
    Option<String>,
    Vec<(String, String)>,
    Vec<(u16, String, Vec<(String, String)>)>,
) {
    let parsed = tesela_core::note_tree::parse_note(content);
    (
        parsed.frontmatter,
        parsed.page_properties,
        parsed
            .blocks
            .into_iter()
            .map(|block| (block.indent, block.text, block.properties))
            .collect(),
    )
}

fn write_graph(root: &std::path::Path, count: usize) {
    let pages = root.join("pages");
    fs::create_dir_all(&pages).unwrap();
    fs::write(
        pages.join("Feature.md"),
        "title:: Feature\n# Imported heading\n\nImported prose line one\nline two\n\n```query\n{:find [?b]}\nstatus:: done\n- literal bullet\n```\n",
    )
    .unwrap();
    for i in 1..count {
        fs::write(
            pages.join(format!("Page {i:03}.md")),
            format!("- imported page {i}\n"),
        )
        .unwrap();
    }
}

#[tokio::test]
async fn engine_import_is_durable_relay_visible_idempotent_and_scales() {
    let temp = TempDir::new().unwrap();
    let graph = temp.path().join("graph");
    let mosaic = temp.path().join("mosaic");
    let notes = mosaic.join("notes");
    let snapshots = mosaic.join(".tesela/loro");
    write_graph(&graph, 501);

    let device = DeviceId::from_bytes([0x51; 16]);
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshots.clone(),
        Some(notes.clone()),
    )
    .await
    .unwrap();
    let plan = build_plan(&graph, &mosaic).unwrap();
    assert_eq!(plan.items.len(), 501);
    let mut writer = EngineImportNoteWriter::new(&engine);

    let outcome = tokio::time::timeout(
        Duration::from_secs(20),
        apply_plan_with_writer(&plan, &ApplyDecisions::default(), &mosaic, &mut writer),
    )
    .await
    .expect("501-note import stays inside the request budget")
    .unwrap();
    assert_eq!(outcome.imported, 501);
    assert!(outcome.errors.is_empty(), "{:?}", outcome.errors);

    let tracked: HashSet<[u8; 16]> = engine.tracked_note_ids().await.into_iter().collect();
    assert_eq!(tracked.len(), 501);
    for item in &plan.items {
        let note_id = stable_uuid_from_slug(&item.target_id);
        assert!(tracked.contains(&note_id));
        assert!(snapshots.join(format!("{}.bin", hex::encode(note_id))).is_file());
        let materialized = fs::read_to_string(&item.target_path).unwrap();
        assert_eq!(
            structural_projection(item.rendered_full.as_deref().unwrap()),
            structural_projection(&materialized),
            "materialized structure diverged for {}",
            item.target_id
        );
    }

    let feature_id = stable_uuid_from_slug("feature");
    let materialized = fs::read_to_string(notes.join("feature.md")).unwrap();
    assert_eq!(
        engine.render_note_full(feature_id).await.unwrap(),
        materialized
    );
    assert!(materialized.contains("# Imported heading"));
    assert!(materialized.contains("Imported prose line one"));
    assert!(materialized.contains("line two"));
    assert!(materialized.contains("```query"));
    assert!(materialized.contains("status:: done"));
    assert!(materialized.contains("- literal bullet"));

    let updates = engine.produce_relay_updates().await;
    assert_eq!(updates.len(), 501);
    let committed: Vec<([u8; 16], Vec<u8>)> = updates
        .iter()
        .map(|(note_id, _, version)| (*note_id, version.clone()))
        .collect();
    engine.commit_broadcast_cursors(&committed).await;

    let second_plan = build_plan(&graph, &mosaic).unwrap();
    assert!(second_plan
        .items
        .iter()
        .all(|item| item.kind == PlanKind::Unchanged));
    let second = apply_plan_with_writer(
        &second_plan,
        &ApplyDecisions::default(),
        &mosaic,
        &mut writer,
    )
    .await
    .unwrap();
    assert_eq!(second.unchanged, 501);
    assert!(engine.produce_relay_updates().await.is_empty());

    drop(writer);
    drop(engine);
    let reopened =
        LoroEngine::with_dirs(device, Arc::new(Hlc::new(device)), snapshots, Some(notes))
            .await
            .unwrap();
    assert_eq!(reopened.tracked_note_ids().await.len(), 501);
    assert_eq!(reopened.index_entries().await.len(), 501);
}

#[tokio::test]
async fn restart_repairs_a_current_schema_index_missing_durable_notes() {
    let temp = TempDir::new().unwrap();
    let notes = temp.path().join("notes");
    let snapshots = temp.path().join(".tesela/loro");
    let device = DeviceId::from_bytes([0x52; 16]);
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshots.clone(),
        Some(notes.clone()),
    )
    .await
    .unwrap();

    hydrate_note(
        &engine,
        stable_uuid_from_slug("alpha"),
        "alpha",
        "- alpha\n",
    )
    .await
    .unwrap();
    let stale_index = fs::read(snapshots.join("_index.bin")).unwrap();
    hydrate_note(&engine, stable_uuid_from_slug("beta"), "beta", "- beta\n")
        .await
        .unwrap();
    drop(engine);

    fs::write(snapshots.join("_index.bin"), stale_index).unwrap();
    let reopened =
        LoroEngine::with_dirs(device, Arc::new(Hlc::new(device)), snapshots, Some(notes))
            .await
            .unwrap();
    let slugs: HashSet<String> = reopened
        .index_entries()
        .await
        .into_iter()
        .map(|entry| entry.slug)
        .collect();
    assert_eq!(
        slugs,
        HashSet::from(["alpha".to_string(), "beta".to_string()])
    );
    assert!(reopened
        .render_note(stable_uuid_from_slug("beta"))
        .await
        .unwrap()
        .contains("beta"));
}

#[tokio::test]
async fn restart_repairs_current_schema_index_with_stale_same_note_metadata() {
    let temp = TempDir::new().unwrap();
    let notes = temp.path().join("notes");
    let snapshots = temp.path().join(".tesela/loro");
    let device = DeviceId::from_bytes([0x54; 16]);
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshots.clone(),
        Some(notes.clone()),
    )
    .await
    .unwrap();
    let note_id = stable_uuid_from_slug("alpha");
    hydrate_note(
        &engine,
        note_id,
        "alpha",
        "---\ntitle: Old title\n---\n- #old [[Before]] <!-- bid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa -->\n",
    )
    .await
    .unwrap();
    let stale_index = fs::read(snapshots.join("_index.bin")).unwrap();

    hydrate_note(
        &engine,
        note_id,
        "alpha",
        "---\ntitle: New title\n---\n- #new [[After]] <!-- bid:aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa -->\n",
    )
    .await
    .unwrap();
    drop(engine);
    fs::write(snapshots.join("_index.bin"), stale_index).unwrap();

    let reopened =
        LoroEngine::with_dirs(device, Arc::new(Hlc::new(device)), snapshots, Some(notes))
            .await
            .unwrap();
    let entries = reopened.index_entries().await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "New title");
    assert_eq!(entries[0].tags, vec!["new"]);
    assert_eq!(entries[0].links, vec!["After"]);
}

#[tokio::test]
async fn restart_prunes_current_schema_index_when_all_note_snapshots_are_missing() {
    let temp = TempDir::new().unwrap();
    let notes = temp.path().join("notes");
    let snapshots = temp.path().join(".tesela/loro");
    let device = DeviceId::from_bytes([0x55; 16]);
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshots.clone(),
        Some(notes.clone()),
    )
    .await
    .unwrap();
    let note_id = stable_uuid_from_slug("ghost");
    hydrate_note(&engine, note_id, "ghost", "- ghost\n")
        .await
        .unwrap();
    drop(engine);
    fs::remove_file(snapshots.join(format!("{}.bin", hex::encode(note_id)))).unwrap();

    let reopened =
        LoroEngine::with_dirs(device, Arc::new(Hlc::new(device)), snapshots, Some(notes))
            .await
            .unwrap();
    assert!(reopened.index_entries().await.is_empty());
}

#[tokio::test]
async fn batch_does_not_publish_markdown_when_snapshot_persistence_fails() {
    let temp = TempDir::new().unwrap();
    let graph = temp.path().join("graph");
    let mosaic = temp.path().join("mosaic");
    let notes = mosaic.join("notes");
    let snapshots = mosaic.join(".tesela/loro");
    write_graph(&graph, 1);
    let device = DeviceId::from_bytes([0x53; 16]);
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshots.clone(),
        Some(notes.clone()),
    )
    .await
    .unwrap();
    fs::remove_dir_all(&snapshots).unwrap();
    fs::write(&snapshots, b"not a directory").unwrap();

    let plan = build_plan(&graph, &mosaic).unwrap();
    let mut writer = EngineImportNoteWriter::new(&engine);
    let outcome = apply_plan_with_writer(&plan, &ApplyDecisions::default(), &mosaic, &mut writer)
        .await
        .unwrap();

    assert_eq!(outcome.imported, 0);
    assert_eq!(outcome.errors.len(), 1);
    assert!(outcome.errors[0].contains("snapshot write"));
    assert!(!notes.join("feature.md").exists());
}
