use super::*;

fn note_upsert(note_id: [u8; 16], slug: &str, title: &str, content: &str) -> OpPayload {
    OpPayload::NoteUpsert {
        note_id,
        display_alias: Some(slug.to_string()),
        title: title.to_string(),
        content: content.to_string(),
        created_at_millis: 0,
    }
}

#[test]
fn page_directory_binding_seed_is_deterministic_and_binding_specific() {
    let page = tesela_core::PageId::from_legacy_doc_id(&[7; 16]);
    let doc = hex_id(&[8; 16]);
    let first = page_directory_binding_seed_update(page, &doc).unwrap();
    let second = page_directory_binding_seed_update(page, &doc).unwrap();
    assert_eq!(
        first, second,
        "same binding must author the same seed update"
    );
    assert_ne!(
        first,
        page_directory_binding_seed_update(page, &hex_id(&[9; 16])).unwrap(),
        "a seed update must bind exactly one legacy document address"
    );
}

#[tokio::test]
async fn page_directory_backfill_persists_root_and_frontmatter_identity() {
    let note_id = blake3_note_id("identity");
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    engine
        .record_local(note_upsert(
            note_id,
            "identity",
            "Identity",
            "---\ntitle: Identity\naliases: [Identity Alias]\n---\n- body\n",
        ))
        .await
        .unwrap();

    let expected = tesela_core::PageId::from_legacy_doc_id(&note_id);
    let entries = engine.page_directory_list().await;
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].page_id, expected);
    assert_eq!(entries[0].loro_doc_id, hex_id(&note_id));
    assert!(!entries[0].deleted);
    assert_eq!(entries[0].aliases, ["Identity Alias"]);
    let full = engine.render_note_full(note_id).await.unwrap();
    assert!(full.contains(&format!("tesela_page_id: {expected}")));
    let docs = engine.inner.docs.read().await;
    let root = docs.get(&note_id).unwrap().get_map("root");
    let root_page_id = root
        .get("page_id")
        .and_then(|value| value.into_value().ok())
        .and_then(|value| value.into_string().ok())
        .map(|value| (*value).clone());
    let expected_string = expected.to_string();
    assert_eq!(root_page_id.as_deref(), Some(expected_string.as_str()));
}

#[tokio::test]
async fn block_upsert_initializes_page_identity_and_directory() {
    let note_id = blake3_note_id("block-created-page");
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: A_BID_BYTES,
            note_id,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "body".into(),
            after_block_id: None,
        })
        .await
        .unwrap();

    let expected = tesela_core::PageId::from_legacy_doc_id(&note_id);
    let full = engine.render_note_full(note_id).await.unwrap();
    assert!(
        full.contains(&format!("tesela_page_id: {expected}")),
        "a page created through a block operation must export its canonical identity"
    );
    let docs = engine.inner.docs.read().await;
    let root = docs.get(&note_id).unwrap().get_map("root");
    let root_page_id = root
        .get("page_id")
        .and_then(|value| value.into_value().ok())
        .and_then(|value| value.into_string().ok())
        .map(|value| (*value).clone());
    assert_eq!(root_page_id.as_deref(), Some(expected.to_string().as_str()));
    drop(docs);
    assert!(
        engine
            .page_directory_list()
            .await
            .iter()
            .any(|entry| entry.page_id == expected && entry.loro_doc_id == hex_id(&note_id)),
        "the page directory must bind the same canonical identity"
    );
}

#[tokio::test]
async fn startup_does_not_hydrate_a_duplicate_for_materialized_directory_entry() {
    let tmp = tempfile::TempDir::new().unwrap();
    let snapshots = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    let note_id = [0xa8; 16];
    let slug = "arbitrary-document-id";
    assert_ne!(note_id, tesela_core::stable_uuid_from_slug(slug));
    let device = test_device();
    let engine = LoroEngine::with_dirs(
        device,
        Arc::new(Hlc::new(device)),
        snapshots.clone(),
        Some(notes.clone()),
    )
    .await
    .unwrap();
    engine
        .record_local(note_upsert(
            note_id,
            slug,
            "Arbitrary document identity",
            "---\ntitle: Arbitrary document identity\n---\n- body\n",
        ))
        .await
        .unwrap();
    assert!(notes.join(format!("{slug}.md")).exists());
    drop(engine);

    let reopened =
        LoroEngine::with_dirs(device, Arc::new(Hlc::new(device)), snapshots, Some(notes))
            .await
            .unwrap();
    assert_eq!(
        reopened.note_count().await,
        1,
        "a directory-backed PageId must prevent legacy-slug hydration from duplicating the page"
    );
    assert_eq!(reopened.page_directory_list().await.len(), 1);
}

#[tokio::test]
async fn opening_legacy_snapshot_backfills_and_persists_page_directory_identity() {
    let dir = tempfile::tempdir().unwrap();
    let note_id = blake3_note_id("legacy-snapshot");
    let doc = LoroDoc::new();
    let root = doc.get_map("root");
    root.insert("slug", "legacy-snapshot").unwrap();
    root.insert("title", "Legacy Snapshot").unwrap();
    root.insert("content", "---\ntitle: Legacy Snapshot\n---\n- body\n")
        .unwrap();
    doc.commit();
    std::fs::write(
        dir.path().join(format!("{}.bin", hex_id(&note_id))),
        doc.export(ExportMode::Snapshot).unwrap(),
    )
    .unwrap();

    let engine = LoroEngine::with_snapshot_dir(
        test_device(),
        Arc::new(Hlc::new(test_device())),
        dir.path().to_path_buf(),
    )
    .await
    .unwrap();
    let expected = tesela_core::PageId::from_legacy_doc_id(&note_id);
    assert_eq!(
        engine.page_directory_list().await,
        vec![crate::engine::PageDirectoryEntry {
            page_id: expected,
            loro_doc_id: hex_id(&note_id),
            slug: "legacy-snapshot".to_string(),
            title: "Legacy Snapshot".to_string(),
            aliases: Vec::new(),
            deleted: false,
            forward_to_loro_doc_id: None,
            conflict: false,
        }]
    );
    assert!(engine
        .render_note_full(note_id)
        .await
        .unwrap()
        .contains(&format!("tesela_page_id: {expected}")));

    drop(engine);
    let reopened = LoroEngine::with_snapshot_dir(
        test_device(),
        Arc::new(Hlc::new(test_device())),
        dir.path().to_path_buf(),
    )
    .await
    .unwrap();
    assert_eq!(reopened.page_directory_list().await.len(), 1);
}

#[tokio::test]
async fn startup_hydrates_disk_only_note_into_page_directory() {
    let tmp = tempfile::TempDir::new().unwrap();
    let snapshots = tmp.path().join("loro");
    let notes = tmp.path().join("notes");
    std::fs::create_dir_all(&notes).unwrap();
    std::fs::write(
        notes.join("disk-only.md"),
        "---\ntitle: Disk only\naliases: [Imported]\n---\n- body\n",
    )
    .unwrap();
    let device = test_device();
    let engine = LoroEngine::with_dirs(device, Arc::new(Hlc::new(device)), snapshots, Some(notes))
        .await
        .unwrap();

    let doc_id = blake3_note_id("disk-only");
    let expected = tesela_core::PageId::from_legacy_doc_id(&doc_id);
    let entry = engine
        .page_directory_list()
        .await
        .into_iter()
        .find(|entry| entry.loro_doc_id == hex_id(&doc_id))
        .unwrap();
    assert_eq!(entry.page_id, expected);
    assert_eq!(entry.aliases, ["Imported"]);
    assert!(engine
        .render_note_full(doc_id)
        .await
        .unwrap()
        .contains(&format!("tesela_page_id: {expected}")));
}

#[tokio::test]
async fn imported_legacy_frontmatter_is_restamped_without_a_restart() {
    let note_id = blake3_note_id("imported-legacy-frontmatter");
    let a_device = DeviceId::from_bytes([0xa1; 16]);
    let b_device = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::new(a_device, Arc::new(Hlc::new(a_device)));
    let b = LoroEngine::new(b_device, Arc::new(Hlc::new(b_device)));
    a.record_local(note_upsert(
        note_id,
        "imported-legacy-frontmatter",
        "Imported legacy frontmatter",
        "---\ntitle: Imported legacy frontmatter\n---\n- body\n",
    ))
    .await
    .unwrap();
    let snapshot = a.export_doc_update(note_id, None).await.unwrap();
    b.import_doc_update(note_id, &snapshot).await.unwrap();

    let before = b.doc_version(note_id).await.unwrap();
    {
        let docs = b.inner.docs.read().await;
        let root = docs.get(&note_id).unwrap().get_map("root");
        root.insert(
            "frontmatter",
            "title: Imported legacy frontmatter\n".to_string(),
        )
        .unwrap();
        docs.get(&note_id).unwrap().commit();
    }
    let stale_frontmatter_delta = b.export_doc_update(note_id, Some(&before)).await.unwrap();

    a.import_doc_update(note_id, &stale_frontmatter_delta)
        .await
        .unwrap();

    let expected = tesela_core::PageId::from_legacy_doc_id(&note_id);
    assert!(
        a.render_note_full(note_id)
            .await
            .unwrap()
            .contains(&format!("tesela_page_id: {expected}")),
        "a live legacy import must restore the root PageId mirror immediately"
    );
}

#[tokio::test]
async fn page_directory_same_page_first_create_merges_field_wise() {
    let note_id = blake3_note_id("convergent");
    let a_dev = DeviceId::from_bytes([0x11; 16]);
    let b_dev = DeviceId::from_bytes([0x22; 16]);
    let a = LoroEngine::new(a_dev, Arc::new(Hlc::new(a_dev)));
    let b = LoroEngine::new(b_dev, Arc::new(Hlc::new(b_dev)));
    let op = note_upsert(note_id, "convergent", "Convergent", "- body\n");
    a.record_local(op.clone()).await.unwrap();
    b.record_local(op).await.unwrap();

    let a_dir = a
        .export_doc_update(PAGE_DIRECTORY_DOC_ID, None)
        .await
        .unwrap();
    let b_dir = b
        .export_doc_update(PAGE_DIRECTORY_DOC_ID, None)
        .await
        .unwrap();
    a.import_doc_update(PAGE_DIRECTORY_DOC_ID, &b_dir)
        .await
        .unwrap();
    b.import_doc_update(PAGE_DIRECTORY_DOC_ID, &a_dir)
        .await
        .unwrap();

    assert_eq!(a.page_directory_list().await, b.page_directory_list().await);
    let records = a.page_directory_list().await;
    assert_eq!(records.len(), 1);
    assert!(!records[0].conflict);
}

#[tokio::test]
async fn page_directory_is_synced_persisted_and_excluded_from_notes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().join("loro");
    let engine = LoroEngine::with_snapshot_dir(
        test_device(),
        Arc::new(Hlc::new(test_device())),
        dir.clone(),
    )
    .await
    .unwrap();
    let note_id = blake3_note_id("restart");
    engine
        .record_local(note_upsert(note_id, "restart", "Restart", "- body\n"))
        .await
        .unwrap();
    assert!(dir
        .join(format!("{}.bin", hex_id(&PAGE_DIRECTORY_DOC_ID)))
        .exists());
    assert!(engine
        .tracked_note_ids()
        .await
        .contains(&PAGE_DIRECTORY_DOC_ID));
    assert_eq!(engine.note_count().await, 1);
    assert!(engine.render_note(PAGE_DIRECTORY_DOC_ID).await.is_none());
    assert!(engine
        .render_note_full(PAGE_DIRECTORY_DOC_ID)
        .await
        .is_none());
    assert!(!engine
        .index_entries()
        .await
        .iter()
        .any(|entry| entry.note_id == hex_id(&PAGE_DIRECTORY_DOC_ID)));

    drop(engine);
    let reopened =
        LoroEngine::with_snapshot_dir(test_device(), Arc::new(Hlc::new(test_device())), dir)
            .await
            .unwrap();
    assert_eq!(reopened.page_directory_list().await.len(), 1);
}

#[tokio::test]
async fn reopening_preserves_tombstone_forwarding_and_aliases() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().join("loro");
    let device = test_device();
    let engine = LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir.clone())
        .await
        .unwrap();
    let old_doc = blake3_note_id("old-slug");
    let new_doc = blake3_note_id("new-slug");
    engine
        .record_local(note_upsert(
            old_doc,
            "old-slug",
            "Old title",
            "---\ntitle: Old title\naliases: [Metadata Alias]\n---\n- body\n",
        ))
        .await
        .unwrap();
    let page_id = tesela_core::PageId::from_legacy_doc_id(&old_doc);
    engine
        .page_directory_upsert(crate::engine::PageDirectoryEntry {
            page_id,
            loro_doc_id: hex_id(&old_doc),
            slug: "old-slug".to_string(),
            title: "Old title".to_string(),
            aliases: vec!["Old Slug".to_string()],
            deleted: true,
            forward_to_loro_doc_id: Some(hex_id(&new_doc)),
            conflict: false,
        })
        .await
        .unwrap();
    drop(engine);

    let reopened = LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), dir)
        .await
        .unwrap();
    let entry = reopened
        .page_directory_list()
        .await
        .into_iter()
        .find(|entry| entry.loro_doc_id == hex_id(&old_doc))
        .unwrap();
    assert!(entry.deleted);
    assert_eq!(entry.forward_to_loro_doc_id, Some(hex_id(&new_doc)));
    assert_eq!(entry.aliases, ["Old Slug", "Metadata Alias"]);
}

#[tokio::test]
async fn page_directory_conflicts_fail_closed_instead_of_lww_resolution() {
    let page_id = tesela_core::PageId::from_legacy_doc_id(&[9; 16]);
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    for (doc, slug) in [([1; 16], "one"), ([2; 16], "two")] {
        engine
            .page_directory_upsert(crate::engine::PageDirectoryEntry {
                page_id,
                loro_doc_id: hex_id(&doc),
                slug: slug.to_string(),
                title: slug.to_string(),
                aliases: Vec::new(),
                deleted: false,
                forward_to_loro_doc_id: None,
                conflict: false,
            })
            .await
            .unwrap();
    }
    let entries = engine.page_directory_list().await;
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.conflict));
}

#[tokio::test]
async fn reopening_persisted_page_directory_conflict_stays_fail_closed() {
    let temp = tempfile::TempDir::new().unwrap();
    let snapshot_dir = temp.path().join("loro");
    let device = test_device();
    let engine =
        LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), snapshot_dir.clone())
            .await
            .unwrap();
    let doc = blake3_note_id("persisted-conflict");
    engine
        .record_local(note_upsert(
            doc,
            "persisted-conflict",
            "Persisted conflict",
            "---\ntitle: Persisted conflict\n---\n- body\n",
        ))
        .await
        .unwrap();
    engine
        .page_directory_upsert(crate::engine::PageDirectoryEntry {
            page_id: tesela_core::PageId::from_legacy_doc_id(&[0x77; 16]),
            loro_doc_id: hex_id(&doc),
            slug: "persisted-conflict".into(),
            title: "Conflicting binding".into(),
            aliases: Vec::new(),
            deleted: false,
            forward_to_loro_doc_id: None,
            conflict: false,
        })
        .await
        .unwrap();
    drop(engine);

    let reopened = LoroEngine::with_snapshot_dir(device, Arc::new(Hlc::new(device)), snapshot_dir)
        .await
        .expect("a persisted conflict is a valid fail-closed state");
    let entries = reopened
        .page_directory_list()
        .await
        .into_iter()
        .filter(|entry| entry.loro_doc_id == hex_id(&doc))
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.conflict));
}

#[tokio::test]
async fn tombstoned_forward_source_rejects_local_and_remote_stale_edits() {
    let old_doc = blake3_note_id("forwarded-source");
    let new_doc = blake3_note_id("forwarded-target");
    let block_id = [0x91; 16];
    let page_id = tesela_core::PageId::from_legacy_doc_id(&old_doc);
    let a_device = DeviceId::from_bytes([0x92; 16]);
    let b_device = DeviceId::from_bytes([0x93; 16]);
    let a = LoroEngine::new(a_device, Arc::new(Hlc::new(a_device)));
    let b = LoroEngine::new(b_device, Arc::new(Hlc::new(b_device)));
    let source_content = "- source <!-- bid:91919191-9191-9191-9191-919191919191 -->\n";
    a.record_local(note_upsert(
        old_doc,
        "forwarded-source",
        "Forwarded source",
        source_content,
    ))
    .await
    .unwrap();
    let source_snapshot = a.export_doc_update(old_doc, None).await.unwrap();
    b.import_doc_update(old_doc, &source_snapshot)
        .await
        .unwrap();
    let source_cursor = b.doc_version(old_doc).await.expect("source cursor");
    a.record_local(note_upsert(
        new_doc,
        "forwarded-target",
        "Forwarded target",
        &tesela_core::storage::markdown::set_page_id_frontmatter(source_content, page_id),
    ))
    .await
    .unwrap();
    a.page_directory_upsert(crate::engine::PageDirectoryEntry {
        page_id,
        loro_doc_id: hex_id(&old_doc),
        slug: "forwarded-source".into(),
        title: "Forwarded source".into(),
        aliases: vec!["forwarded-source".into()],
        deleted: false,
        forward_to_loro_doc_id: Some(hex_id(&new_doc)),
        conflict: true,
    })
    .await
    .unwrap();
    a.record_local(OpPayload::NoteDelete {
        note_id: old_doc,
        display_alias: Some("forwarded-source".into()),
    })
    .await
    .unwrap();

    let local_error = a
        .record_local(OpPayload::BlockUpsert {
            note_id: old_doc,
            block_id,
            parent_block_id: None,
            order_key: "0".into(),
            indent_level: 0,
            text: "stale local edit".into(),
            after_block_id: None,
        })
        .await
        .expect_err("tombstoned source write must fail closed");
    assert!(local_error.to_string().contains("cannot edit"));
    assert!(a
        .splice_block_text(old_doc, block_id, 0, 0, "stale splice")
        .await
        .is_err());

    b.record_local(OpPayload::BlockUpsert {
        note_id: old_doc,
        block_id,
        parent_block_id: None,
        order_key: "0".into(),
        indent_level: 0,
        text: "stale remote edit".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    let stale_delta = b
        .export_doc_update(old_doc, Some(&source_cursor))
        .await
        .unwrap();
    a.import_doc_update(old_doc, &stale_delta).await.unwrap();
    let target = a.render_note_full(new_doc).await.expect("live target");
    assert!(target.contains("- source"));
    assert!(!target.contains("stale local edit"));
    assert!(!target.contains("stale remote edit"));
}

#[tokio::test]
async fn late_page_directory_keeps_resident_tombstoned_source_fail_closed() {
    let old_doc = blake3_note_id("directory-last-source");
    let new_doc = blake3_note_id("directory-last-target");
    let source_device = DeviceId::from_bytes([0xa2; 16]);
    let receiver_device = DeviceId::from_bytes([0xa3; 16]);
    let source = LoroEngine::new(source_device, Arc::new(Hlc::new(source_device)));
    let receiver = LoroEngine::new(receiver_device, Arc::new(Hlc::new(receiver_device)));
    let source_content = "- source <!-- bid:a1a1a1a1-a1a1-a1a1-a1a1-a1a1a1a1a1a1 -->\n";
    source
        .record_local(note_upsert(
            old_doc,
            "directory-last-source",
            "Directory last source",
            source_content,
        ))
        .await
        .unwrap();
    let page_id = tesela_core::PageId::from_legacy_doc_id(&old_doc);
    let target_content =
        tesela_core::storage::markdown::set_page_id_frontmatter(source_content, page_id);
    source
        .record_local(note_upsert(
            new_doc,
            "directory-last-target",
            "Directory last target",
            &target_content,
        ))
        .await
        .unwrap();
    source
        .page_directory_upsert(crate::engine::PageDirectoryEntry {
            page_id,
            loro_doc_id: hex_id(&old_doc),
            slug: "directory-last-source".into(),
            title: "Directory last source".into(),
            aliases: Vec::new(),
            deleted: true,
            forward_to_loro_doc_id: Some(hex_id(&new_doc)),
            conflict: false,
        })
        .await
        .unwrap();
    source
        .record_local(OpPayload::NoteDelete {
            note_id: old_doc,
            display_alias: Some("directory-last-source".into()),
        })
        .await
        .unwrap();
    // The target and source root stream can reach a receiver before the
    // independently synced directory record that explains the forwarding.
    let target_snapshot = source.export_doc_update(new_doc, None).await.unwrap();
    receiver
        .import_doc_update(new_doc, &target_snapshot)
        .await
        .unwrap();
    let tombstoned_source_snapshot = source.export_doc_update(old_doc, None).await.unwrap();
    receiver
        .import_doc_update(old_doc, &tombstoned_source_snapshot)
        .await
        .unwrap();
    assert!(
        !receiver
            .render_note_full(new_doc)
            .await
            .expect("target before forwarding directory")
            .contains("source edit before directory arrival"),
        "without forwarding provenance, source state remains on its original lineage"
    );

    let directory_snapshot = source
        .export_doc_update(PAGE_DIRECTORY_DOC_ID, None)
        .await
        .unwrap();
    receiver
        .import_doc_update(PAGE_DIRECTORY_DOC_ID, &directory_snapshot)
        .await
        .unwrap();

    assert!(
        !receiver
            .render_note_full(new_doc)
            .await
            .expect("target after forwarding directory")
            .contains("source edit before directory arrival"),
        "late forwarding provenance must not reauthor stale source state"
    );
}
#[tokio::test]
async fn page_directory_conflicts_when_one_document_claims_two_page_ids() {
    let doc = [3; 16];
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    for page_id in [
        tesela_core::PageId::from_legacy_doc_id(&[10; 16]),
        tesela_core::PageId::from_legacy_doc_id(&[11; 16]),
    ] {
        engine
            .page_directory_upsert(crate::engine::PageDirectoryEntry {
                page_id,
                loro_doc_id: hex_id(&doc),
                slug: "same-doc".to_string(),
                title: "Same document".to_string(),
                aliases: Vec::new(),
                deleted: false,
                forward_to_loro_doc_id: None,
                conflict: false,
            })
            .await
            .unwrap();
    }

    let entries = engine.page_directory_list().await;
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|entry| entry.conflict));
}

#[tokio::test]
async fn page_directory_decodes_an_empty_alias_field_as_no_aliases() {
    let doc = [4; 16];
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    engine
        .page_directory_upsert(crate::engine::PageDirectoryEntry {
            page_id: tesela_core::PageId::from_legacy_doc_id(&doc),
            loro_doc_id: hex_id(&doc),
            slug: "no-alias".to_string(),
            title: "No alias".to_string(),
            aliases: Vec::new(),
            deleted: false,
            forward_to_loro_doc_id: None,
            conflict: false,
        })
        .await
        .unwrap();

    let entry = engine
        .page_directory_list()
        .await
        .into_iter()
        .next()
        .expect("directory binding");
    assert!(entry.aliases.is_empty());
}

#[tokio::test]
async fn ordinary_upsert_preserves_directory_tombstone_and_forwarding() {
    let note_id = blake3_note_id("tombstoned");
    let page_id = tesela_core::PageId::from_legacy_doc_id(&note_id);
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let forward_to = hex_id(&blake3_note_id("renamed"));

    engine
        .page_directory_upsert(crate::engine::PageDirectoryEntry {
            page_id,
            loro_doc_id: hex_id(&note_id),
            slug: "old-slug".into(),
            title: "Old title".into(),
            aliases: vec!["old-slug".into()],
            deleted: true,
            forward_to_loro_doc_id: Some(forward_to.clone()),
            conflict: false,
        })
        .await
        .unwrap();

    let error = engine
        .record_local(note_upsert(
            note_id,
            "late-upsert",
            "Late upsert",
            "---\ntitle: Late upsert\n---\n- body\n",
        ))
        .await
        .expect_err("stale NoteUpsert must fail closed");
    assert!(error.to_string().contains("cannot edit"));
    assert!(
        engine.render_note_full(note_id).await.is_none(),
        "a stale NoteUpsert must not recreate a tombstoned source document"
    );
    assert!(
        !engine
            .index_entries()
            .await
            .iter()
            .any(|entry| entry.note_id == hex_id(&note_id)),
        "a stale NoteUpsert must not re-index a tombstoned source document"
    );
    assert_eq!(
        engine.note_count().await,
        0,
        "a tombstoned source must stay excluded from the live note count"
    );

    let entry = engine
        .page_directory_list()
        .await
        .into_iter()
        .find(|entry| entry.loro_doc_id == hex_id(&note_id))
        .expect("directory binding");
    assert!(
        entry.deleted,
        "late NoteUpsert must not resurrect a tombstone"
    );
    assert_eq!(entry.aliases, ["old-slug"]);
    assert_eq!(
        entry.forward_to_loro_doc_id.as_deref(),
        Some(forward_to.as_str())
    );
}

#[tokio::test]
async fn delayed_concurrent_upsert_cannot_clear_tombstone_or_forwarding() {
    let page_id = tesela_core::PageId::from_legacy_doc_id(&[0x71; 16]);
    let old_doc = hex_id(&[0x72; 16]);
    let new_doc = hex_id(&[0x73; 16]);
    let a_device = DeviceId::from_bytes([0x74; 16]);
    let b_device = DeviceId::from_bytes([0x75; 16]);
    let a = LoroEngine::new(a_device, Arc::new(Hlc::new(a_device)));
    let b = LoroEngine::new(b_device, Arc::new(Hlc::new(b_device)));

    // The source peer has a delayed, stale ordinary binding update. Advance
    // its operation clock so a scalar LWW `deleted: false` would otherwise
    // beat the independently authored tombstone.
    for revision in 0..3 {
        a.page_directory_upsert(crate::engine::PageDirectoryEntry {
            page_id,
            loro_doc_id: old_doc.clone(),
            slug: format!("old-{revision}"),
            title: "Old".into(),
            aliases: Vec::new(),
            deleted: false,
            forward_to_loro_doc_id: None,
            conflict: false,
        })
        .await
        .unwrap();
    }
    b.page_directory_upsert(crate::engine::PageDirectoryEntry {
        page_id,
        loro_doc_id: old_doc.clone(),
        slug: "old".into(),
        title: "Old".into(),
        aliases: vec!["old".into()],
        deleted: true,
        forward_to_loro_doc_id: Some(new_doc.clone()),
        conflict: false,
    })
    .await
    .unwrap();

    let a_update = a
        .export_doc_update(PAGE_DIRECTORY_DOC_ID, None)
        .await
        .unwrap();
    let b_update = b
        .export_doc_update(PAGE_DIRECTORY_DOC_ID, None)
        .await
        .unwrap();
    a.import_doc_update(PAGE_DIRECTORY_DOC_ID, &b_update)
        .await
        .unwrap();
    b.import_doc_update(PAGE_DIRECTORY_DOC_ID, &a_update)
        .await
        .unwrap();

    // A later repair pass that still sees its pre-delete cache must not author
    // mutable `false`/empty values over the immutable deletion provenance.
    for engine in [&a, &b] {
        engine
            .page_directory_upsert(crate::engine::PageDirectoryEntry {
                page_id,
                loro_doc_id: old_doc.clone(),
                slug: "stale-cache".into(),
                title: "Stale cache".into(),
                aliases: Vec::new(),
                deleted: false,
                forward_to_loro_doc_id: None,
                conflict: false,
            })
            .await
            .unwrap();
    }
    // Re-exchange the late repairs. Provenance must survive a full CRDT
    // convergence round, not merely the local read immediately afterward.
    let a_repair = a
        .export_doc_update(PAGE_DIRECTORY_DOC_ID, None)
        .await
        .unwrap();
    let b_repair = b
        .export_doc_update(PAGE_DIRECTORY_DOC_ID, None)
        .await
        .unwrap();
    a.import_doc_update(PAGE_DIRECTORY_DOC_ID, &b_repair)
        .await
        .unwrap();
    b.import_doc_update(PAGE_DIRECTORY_DOC_ID, &a_repair)
        .await
        .unwrap();

    for engine in [&a, &b] {
        let entry = engine
            .page_directory_list()
            .await
            .into_iter()
            .find(|entry| entry.loro_doc_id == old_doc)
            .expect("old binding");
        assert!(
            entry.deleted,
            "a stale upsert must not resurrect the source"
        );
        assert_eq!(
            entry.forward_to_loro_doc_id.as_deref(),
            Some(new_doc.as_str()),
            "a stale upsert must not clear the forwarding provenance"
        );
    }
}

#[tokio::test]
async fn every_special_document_is_excluded_from_note_operations() {
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    assert_eq!(SPECIAL_DOC_IDS, [VIEWS_DOC_ID, PAGE_DIRECTORY_DOC_ID]);

    for (index, note_id) in SPECIAL_DOC_IDS.into_iter().enumerate() {
        assert!(is_special_doc(&note_id));
        engine
            .apply_payload(&note_upsert(
                note_id,
                &format!("reserved-{index}"),
                "Must not materialize",
                "- must not materialize\n",
            ))
            .await
            .unwrap();
        assert!(engine.render_note(note_id).await.is_none());
        assert!(engine.render_note_full(note_id).await.is_none());
    }

    assert_eq!(engine.note_count().await, 0);
    assert!(engine.index_entries().await.is_empty());
    assert!(engine.page_directory_list().await.is_empty());
    assert!(!is_special_doc(&blake3_note_id("ordinary-note")));
}

#[test]
fn forwarded_merge_propagates_only_uncontested_block_deletions() {
    let deleted_bid = "11111111-1111-1111-1111-111111111111";
    let retained_bid = "22222222-2222-2222-2222-222222222222";
    let prior = tesela_core::note_tree::parse_note(&format!(
        "- delete me <!-- bid:{deleted_bid} -->\n- keep me <!-- bid:{retained_bid} -->\n"
    ));
    let source =
        tesela_core::note_tree::parse_note(&format!("- keep me <!-- bid:{retained_bid} -->\n"));

    let mut unchanged_target = prior.clone();
    merge_forwarded_source_changes(Some(&prior), &source, &mut unchanged_target);
    assert_eq!(unchanged_target.blocks, source.blocks);

    let mut changed_target = tesela_core::note_tree::parse_note(&format!(
        "- changed independently <!-- bid:{deleted_bid} -->\n- keep me <!-- bid:{retained_bid} -->\n"
    ));
    merge_forwarded_source_changes(Some(&prior), &source, &mut changed_target);
    assert!(
        changed_target
            .blocks
            .iter()
            .any(|block| block.text == "changed independently"),
        "a target-side edit must win over a stale forwarded deletion"
    );
}
