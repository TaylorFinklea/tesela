use super::*;
use crate::engine::TableColumnConfig;

// ─── Views registry (saved-views spec, 2026-06-10) ───────────────────

// ─── tesela-ya4.4 — table column config persistence ───────────────────

#[tokio::test]
async fn views_upsert_persists_and_clears_table_config() {
    let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let mut table_view = user_view("v-table", "Tasks", "tag:task", 10);
    table_view.display_mode = "table".to_string();
    table_view.display_table_config = Some(TableColumnConfig {
        hidden: vec!["notes".to_string()],
        order: vec!["priority".to_string(), "status".to_string()],
        sort_by: Some("priority".to_string()),
        sort_dir: Some("desc".to_string()),
    });
    e.views_upsert(table_view.clone()).await.unwrap();

    let views = e.views_list().await;
    assert_eq!(views.len(), 1);
    assert_eq!(
        views[0].display_table_config,
        table_view.display_table_config,
        "hide/reorder/sort config round-trips through the CRDT store"
    );

    // Clearing the config (None) deletes the underlying field rather than
    // persisting a stale JSON blob.
    let mut cleared = table_view.clone();
    cleared.display_table_config = None;
    e.views_upsert(cleared).await.unwrap();
    let views = e.views_list().await;
    assert_eq!(views[0].display_table_config, None, "config clears back to None");
}

#[tokio::test]
async fn views_upsert_table_config_survives_unrelated_field_update() {
    // Field-level LWW: updating `name` must not clobber a previously-saved
    // `display_table_config` (mirrors the group-by/show-done coverage in
    // `views_upsert_list_round_trip_sorted_by_order` above).
    let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let mut table_view = user_view("v-table", "Tasks", "tag:task", 10);
    table_view.display_mode = "table".to_string();
    table_view.display_table_config = Some(TableColumnConfig {
        hidden: vec![],
        order: vec!["status".to_string()],
        sort_by: Some("status".to_string()),
        sort_dir: Some("asc".to_string()),
    });
    e.views_upsert(table_view.clone()).await.unwrap();

    let mut renamed = table_view.clone();
    renamed.name = "My tasks".to_string();
    e.views_upsert(renamed).await.unwrap();

    let views = e.views_list().await;
    assert_eq!(views[0].name, "My tasks");
    assert_eq!(views[0].display_table_config, table_view.display_table_config);
}

#[tokio::test]
async fn views_upsert_list_round_trip_sorted_by_order() {
    let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let mut kanban = user_view("v-kanban", "Board", "tag:project", 20);
    kanban.display_mode = "kanban".to_string();
    kanban.display_group_by = Some("status".to_string());
    kanban.display_show_done = Some(true);
    e.views_upsert(kanban.clone()).await.unwrap();
    e.views_upsert(user_view("v-week", "This week", "has:scheduled", 10))
        .await
        .unwrap();

    let views = e.views_list().await;
    assert_eq!(views.len(), 2);
    assert_eq!(
        views.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        vec!["v-week", "v-kanban"],
        "sorted by (order, id)"
    );
    assert_eq!(views[1], kanban, "all fields round-trip");

    // Update one field; the others persist (field-level write).
    let mut renamed = kanban.clone();
    renamed.name = "Project board".to_string();
    e.views_upsert(renamed.clone()).await.unwrap();
    let views = e.views_list().await;
    assert_eq!(views.len(), 2, "upsert of existing id is an update");
    assert_eq!(views[1], renamed);
}

#[tokio::test]
async fn views_delete_guards_builtin_and_removes_user_view() {
    let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    e.ensure_builtin_views().await.unwrap();
    e.views_upsert(user_view("v-user", "Mine", "tag:x", 10))
        .await
        .unwrap();

    // Builtin: not deletable — enforced at the API.
    let err = e.views_delete(INBOX_VIEW_ID).await;
    assert!(err.is_err(), "builtin delete must error: {err:?}");
    assert!(
        e.views_list().await.iter().any(|v| v.id == INBOX_VIEW_ID),
        "inbox survives the delete attempt"
    );

    // User view: deletable; second delete reports false.
    assert!(e.views_delete("v-user").await.unwrap());
    assert!(!e.views_delete("v-user").await.unwrap());
    assert!(
        !e.views_list().await.iter().any(|v| v.id == "v-user"),
        "user view removed"
    );

    // Unknown id: Ok(false), no error.
    assert!(!e.views_delete("nope").await.unwrap());
}

#[tokio::test]
async fn views_upsert_cannot_unflag_builtin() {
    // The delete guard would be bypassable by first upserting
    // builtin=false — `builtin` is sticky to close that hole.
    let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    e.ensure_builtin_views().await.unwrap();
    let mut edited = e.views_list().await[0].clone();
    assert_eq!(edited.id, INBOX_VIEW_ID);
    edited.builtin = false;
    edited.dsl = "status:todo".to_string();
    e.views_upsert(edited).await.unwrap();

    let inbox = e.views_list().await[0].clone();
    assert!(inbox.builtin, "builtin flag is sticky across upserts");
    assert_eq!(inbox.dsl, "status:todo", "the edit itself landed");
    assert!(e.views_delete(INBOX_VIEW_ID).await.is_err());
}

#[tokio::test]
async fn ensure_builtin_views_is_idempotent_and_preserves_user_edits() {
    let e = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    e.ensure_builtin_views().await.unwrap();
    e.ensure_builtin_views().await.unwrap();
    let views = e.views_list().await;
    assert_eq!(views.len(), 1, "double seed yields ONE inbox");
    assert_eq!(views[0].id, INBOX_VIEW_ID);
    assert_eq!(views[0].dsl, INBOX_DEFAULT_DSL);
    assert!(views[0].builtin);

    // The builtin is editable; a later reseed must NOT clobber the edit.
    let mut edited = views[0].clone();
    edited.dsl = "status:todo -has:deadline".to_string();
    e.views_upsert(edited.clone()).await.unwrap();
    e.ensure_builtin_views().await.unwrap();
    assert_eq!(
        e.views_list().await[0].dsl,
        edited.dsl,
        "reseed preserves the user's dsl edit"
    );
}

#[tokio::test]
async fn concurrent_seed_converges_to_one_inbox() {
    // Two devices both seed BEFORE ever syncing — the deterministic
    // seed means both author the SAME ops, and the group converges to
    // ONE Inbox with the default fields (no container race at all).
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    a.ensure_builtin_views().await.unwrap();
    b.ensure_builtin_views().await.unwrap();

    ship_relay(&a, &b).await;
    ship_relay(&b, &a).await;
    ship_relay(&a, &b).await;

    let va = a.views_list().await;
    let vb = b.views_list().await;
    assert_eq!(va, vb, "engines converge");
    assert_eq!(va.len(), 1, "exactly ONE inbox group-wide");
    assert_eq!(va[0].id, INBOX_VIEW_ID);
    assert_eq!(va[0].dsl, INBOX_DEFAULT_DSL);
    assert!(va[0].builtin);
}

#[tokio::test]
async fn fresh_device_that_syncs_before_seeding_noops_and_preserves_edit() {
    // The bring-up ordering contract (main.rs / RelayTicker.viewsList):
    // a relay-configured fresh device bootstraps BEFORE seeding, so the
    // seed sees the group's registry — including a user-edited builtin
    // — and no-ops instead of authoring anything.
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_c = DeviceId::from_bytes([0xc3; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
    a.ensure_builtin_views().await.unwrap();
    let mut edited = a.views_list().await[0].clone();
    edited.dsl = "status:todo -has:deadline".to_string();
    a.views_upsert(edited.clone()).await.unwrap();

    // C receives the group state FIRST (bootstrap-before-seed)…
    ship_relay(&a, &c).await;
    // …so its seed no-ops and A's edit survives on both.
    c.ensure_builtin_views().await.unwrap();
    ship_relay(&c, &a).await;
    let va = a.views_list().await;
    let vc = c.views_list().await;
    assert_eq!(va, vc, "engines converge");
    assert_eq!(va.len(), 1, "exactly ONE inbox");
    assert_eq!(va[0].dsl, edited.dsl, "A's edit survives the join");
}

#[tokio::test]
async fn offline_first_seed_then_sync_preserves_remote_builtin_edit() {
    // The INVERTED order: C seeds while truly offline-never-synced,
    // then joins. Every device authors the seed as the SAME
    // deterministic ops (fixed seed peer, no timestamps), so there is
    // no same-key container race to lose — A's edit must survive for
    // BOTH peer orderings, not just the one where A's container wins
    // the map-key LWW coin flip.
    for (bytes_a, bytes_c) in [([0xa1u8; 16], [0xc3u8; 16]), ([0xc3u8; 16], [0xa1u8; 16])] {
        let dev_a = DeviceId::from_bytes(bytes_a);
        let dev_c = DeviceId::from_bytes(bytes_c);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
        a.ensure_builtin_views().await.unwrap();
        let mut edited = a.views_list().await[0].clone();
        edited.dsl = "status:todo -has:deadline".to_string();
        a.views_upsert(edited.clone()).await.unwrap();

        // C seeds with no shared history at all, then syncs.
        c.ensure_builtin_views().await.unwrap();
        ship_relay(&a, &c).await;
        ship_relay(&c, &a).await;
        ship_relay(&a, &c).await;

        let va = a.views_list().await;
        let vc = c.views_list().await;
        assert_eq!(va, vc, "engines converge (A={bytes_a:02x?})");
        assert_eq!(va.len(), 1, "exactly ONE inbox (A={bytes_a:02x?})");
        assert_eq!(
            va[0].dsl, edited.dsl,
            "A's edit survives an offline-first seed (A={bytes_a:02x?})"
        );
        assert!(va[0].builtin);
    }
}

#[tokio::test]
async fn builtin_seed_ops_are_identical_across_devices() {
    // Determinism pin: two devices seeding independently author
    // byte-identical seed updates (reserved peer, no timestamps), so a
    // one-way ship leaves the receiver unchanged — its version vector
    // already covers the seed ops.
    assert_eq!(
        builtin_views_seed_update().unwrap(),
        builtin_views_seed_update().unwrap(),
        "seed update bytes are deterministic"
    );
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    a.ensure_builtin_views().await.unwrap();
    b.ensure_builtin_views().await.unwrap();
    let before = b.views_list().await;
    ship_relay(&a, &b).await;
    assert_eq!(b.views_list().await, before, "A's seed is already known");
    assert_eq!(before.len(), 1);
}

#[tokio::test]
async fn builtin_upsert_on_unseeded_device_routes_through_seed_container() {
    // iOS hub-mode shape: a never-synced device EDITS the builtin
    // directly (views_upsert, no prior seed — the UI edits the
    // fallback Inbox). The upsert must land its fields in THE
    // deterministic seed container so a later join field-merges with
    // the group instead of racing whole containers — for BOTH peer
    // orderings, not just the one where C's container would win.
    for (bytes_a, bytes_c) in [([0xa1u8; 16], [0xc3u8; 16]), ([0xc3u8; 16], [0xa1u8; 16])] {
        let dev_a = DeviceId::from_bytes(bytes_a);
        let dev_c = DeviceId::from_bytes(bytes_c);
        let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
        let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
        a.ensure_builtin_views().await.unwrap();
        let mut a_edit = a.views_list().await[0].clone();
        a_edit.dsl = "status:todo".to_string();
        a.views_upsert(a_edit).await.unwrap();

        // C renames the builtin with no seed and no shared history.
        let mut c_record = user_view(INBOX_VIEW_ID, "Triage", INBOX_DEFAULT_DSL, 0);
        c_record.builtin = true;
        c.views_upsert(c_record).await.unwrap();

        ship_relay(&a, &c).await;
        ship_relay(&c, &a).await;
        ship_relay(&a, &c).await;

        let va = a.views_list().await;
        assert_eq!(va, c.views_list().await, "engines converge");
        assert_eq!(va.len(), 1, "exactly ONE inbox");
        // Field-level merge, not wholesale container loss: C's rename
        // survives; dsl (written concurrently by BOTH upserts) resolves
        // to one deterministic LWW winner — never a third value.
        assert_eq!(
            va[0].name, "Triage",
            "C's rename survives (A={bytes_a:02x?})"
        );
        assert!(
            va[0].dsl == "status:todo" || va[0].dsl == INBOX_DEFAULT_DSL,
            "dsl is one LWW winner: {}",
            va[0].dsl
        );
        assert!(va[0].builtin);
    }
}

#[tokio::test]
async fn concurrent_upsert_of_different_views_both_survive() {
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    // Shared base: A seeds, B receives it.
    a.ensure_builtin_views().await.unwrap();
    ship_relay(&a, &b).await;

    // Concurrent: each device creates a DIFFERENT view.
    a.views_upsert(user_view("v-from-a", "A's", "tag:a", 10))
        .await
        .unwrap();
    b.views_upsert(user_view("v-from-b", "B's", "tag:b", 20))
        .await
        .unwrap();
    ship_relay(&a, &b).await;
    ship_relay(&b, &a).await;
    ship_relay(&a, &b).await;

    let va = a.views_list().await;
    assert_eq!(va, b.views_list().await, "engines converge");
    assert_eq!(
        va.iter().map(|v| v.id.as_str()).collect::<Vec<_>>(),
        vec![INBOX_VIEW_ID, "v-from-a", "v-from-b"],
        "both concurrent creations survive (+ the seeded inbox)"
    );
}

#[tokio::test]
async fn concurrent_edit_of_same_view_dsl_is_lww_and_other_fields_survive() {
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    a.views_upsert(user_view("v-shared", "Shared", "tag:base", 10))
        .await
        .unwrap();
    ship_relay(&a, &b).await;

    // Concurrent: A edits the dsl, B edits the name (different fields
    // of the SAME view — field-level LWW keeps both).
    let mut on_a = a.views_list().await[0].clone();
    on_a.dsl = "tag:edited-by-a".to_string();
    a.views_upsert(on_a).await.unwrap();
    let mut on_b = b.views_list().await[0].clone();
    on_b.name = "Renamed by B".to_string();
    b.views_upsert(on_b).await.unwrap();
    ship_relay(&a, &b).await;
    ship_relay(&b, &a).await;
    ship_relay(&a, &b).await;

    let va = a.views_list().await;
    assert_eq!(va, b.views_list().await, "engines converge");
    // B's upsert re-wrote dsl with its stale base value — same-field
    // LWW resolves deterministically to ONE of the two; the rename
    // (the field only B touched with a NEW value) must survive.
    assert_eq!(va[0].name, "Renamed by B");
    assert!(
        va[0].dsl == "tag:edited-by-a" || va[0].dsl == "tag:base",
        "dsl is one LWW winner, not a mash: {}",
        va[0].dsl
    );
}

#[tokio::test]
async fn views_delete_vs_concurrent_edit_converges_deterministically() {
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    a.views_upsert(user_view("v-doomed", "Doomed", "tag:x", 10))
        .await
        .unwrap();
    ship_relay(&a, &b).await;

    // Concurrent: A deletes the view, B edits its dsl.
    assert!(a.views_delete("v-doomed").await.unwrap());
    let mut on_b = b.views_list().await[0].clone();
    on_b.dsl = "tag:edited".to_string();
    b.views_upsert(on_b).await.unwrap();
    ship_relay(&a, &b).await;
    ship_relay(&b, &a).await;
    ship_relay(&a, &b).await;

    let va = a.views_list().await;
    let vb = b.views_list().await;
    assert_eq!(va, vb, "delete vs edit converges to the same state");
    // The map-key delete outranks edits INSIDE the (removed)
    // container: deterministic delete-wins on both replicas.
    assert!(va.is_empty(), "deleted view stays deleted: {va:?}");
}

#[tokio::test]
async fn views_doc_rides_relay_update_path_and_deposit_streams() {
    // Spec item 5: A creates a view → B receives it via the relay
    // update path → B edits the dsl → A converges. Plus: the views doc
    // id is in `tracked_note_ids` (what `deposit_snapshots` iterates).
    let tmp_a = tempfile::tempdir().unwrap();
    let tmp_b = tempfile::tempdir().unwrap();
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let dev_b = DeviceId::from_bytes([0xb2; 16]);
    let a = LoroEngine::with_dirs(
        dev_a,
        Arc::new(Hlc::new(dev_a)),
        tmp_a.path().join("loro"),
        Some(tmp_a.path().join("notes")),
    )
    .await
    .unwrap();
    let b = LoroEngine::with_dirs(
        dev_b,
        Arc::new(Hlc::new(dev_b)),
        tmp_b.path().join("loro"),
        Some(tmp_b.path().join("notes")),
    )
    .await
    .unwrap();

    a.ensure_builtin_views().await.unwrap();
    a.views_upsert(user_view("v-travel", "Travel", "tag:trip", 10))
        .await
        .unwrap();
    assert!(
        SyncEngine::tracked_note_ids(&a)
            .await
            .contains(&VIEWS_DOC_ID),
        "views doc is in the deposit walk (tracked_note_ids)"
    );

    assert!(ship_relay(&a, &b).await >= 1, "B received the views doc");
    assert_eq!(a.views_list().await, b.views_list().await, "bootstrapped");

    // B edits the dsl; A converges through the same path.
    let mut travel = b
        .views_list()
        .await
        .into_iter()
        .find(|v| v.id == "v-travel")
        .unwrap();
    travel.dsl = "tag:trip status:todo".to_string();
    b.views_upsert(travel.clone()).await.unwrap();
    ship_relay(&b, &a).await;
    let on_a = a
        .views_list()
        .await
        .into_iter()
        .find(|v| v.id == "v-travel")
        .unwrap();
    assert_eq!(on_a, travel, "A converged on B's edit");

    // One bounded transitive re-broadcast (A re-emits the delta it just
    // imported — idempotent on B), then steady state: nothing to send.
    ship_relay(&a, &b).await;
    assert_eq!(ship_relay(&b, &a).await, 0);
    assert_eq!(ship_relay(&a, &b).await, 0);
}

#[tokio::test]
async fn views_doc_survives_snapshot_deposit_bootstrap_round() {
    // The relay compaction path: `deposit_snapshots` exports
    // `export_doc_update(id, None)` per tracked doc; a fresh device's
    // `bootstrap_from_snapshots` imports each via `import_doc_update`.
    // Mirror that engine-level seam for the views doc.
    let dev_a = DeviceId::from_bytes([0xa1; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    a.ensure_builtin_views().await.unwrap();
    a.views_upsert(user_view("v-x", "X", "tag:x", 10))
        .await
        .unwrap();

    let snapshot = a
        .export_doc_update(VIEWS_DOC_ID, None)
        .await
        .expect("views doc exports a full snapshot for deposit");

    // Fresh device bootstraps from the deposited snapshot.
    let dev_c = DeviceId::from_bytes([0xc3; 16]);
    let c = LoroEngine::new(dev_c, Arc::new(Hlc::new(dev_c)));
    c.import_doc_update(VIEWS_DOC_ID, &snapshot).await.unwrap();
    assert_eq!(c.views_list().await, a.views_list().await, "bootstrap");

    // The targeted catch-up path (`import_authoritative_snapshot`) is
    // idempotent on the same bytes.
    c.import_authoritative_snapshot(VIEWS_DOC_ID, &snapshot)
        .await
        .unwrap();
    assert_eq!(c.views_list().await, a.views_list().await, "idempotent");
}

#[tokio::test]
async fn views_doc_is_excluded_from_note_machinery() {
    let tmp = tempfile::tempdir().unwrap();
    let dev = test_device();
    let e = LoroEngine::with_dirs(
        dev,
        Arc::new(Hlc::new(dev)),
        tmp.path().join("loro"),
        Some(tmp.path().join("notes")),
    )
    .await
    .unwrap();
    e.ensure_builtin_views().await.unwrap();

    // A real note for contrast.
    let note = blake3_note_id("real-note");
    e.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("real-note".into()),
        title: "Real".into(),
        content: "- hi <!-- bid:70707070-7070-7070-7070-707070707070 -->\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();

    // Not indexed.
    let views_hex = hex_id(&VIEWS_DOC_ID);
    assert!(
        !e.index_entries()
            .await
            .iter()
            .any(|x| x.note_id == views_hex),
        "no phantom index entry for the views doc"
    );
    // Not renderable / not a note for walkers.
    assert!(LoroEngine::render_note(&e, VIEWS_DOC_ID).await.is_none());
    assert!(LoroEngine::render_note_full(&e, VIEWS_DOC_ID)
        .await
        .is_none());
    // Not materialized: notes/ holds exactly the real note.
    let mut files = Vec::new();
    let mut rd = tokio::fs::read_dir(tmp.path().join("notes")).await.unwrap();
    while let Some(entry) = rd.next_entry().await.unwrap() {
        files.push(entry.file_name().to_string_lossy().to_string());
    }
    assert_eq!(files, vec!["real-note.md"], "views doc never hits notes/");
    // But its snapshot IS persisted like any doc's.
    assert!(
        tmp.path()
            .join("loro")
            .join(format!("{views_hex}.bin"))
            .exists(),
        "views doc snapshot persisted"
    );

    // Note-shaped ops addressed at the views doc are refused no-ops.
    let before = e.views_list().await;
    e.apply_payload(&OpPayload::NoteUpsert {
        note_id: VIEWS_DOC_ID,
        display_alias: Some("evil".into()),
        title: "Evil".into(),
        content: "- nope\n".into(),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    e.apply_payload(&OpPayload::NoteDelete {
        note_id: VIEWS_DOC_ID,
        display_alias: None,
    })
    .await
    .unwrap();
    assert_eq!(
        e.views_list().await,
        before,
        "NoteUpsert/NoteDelete at the views doc are no-ops"
    );
    assert!(
        !e.index_entries()
            .await
            .iter()
            .any(|x| x.note_id == views_hex),
        "still not indexed after the refused ops"
    );
}

#[tokio::test]
async fn views_doc_survives_reseed_from_disk() {
    // `reseed_from_disk` replays NoteUpserts from `.md` files — it must
    // leave the views registry untouched (it only ever UPSERTS notes).
    let tmp = tempfile::tempdir().unwrap();
    let dev = test_device();
    let e = LoroEngine::with_dirs(
        dev,
        Arc::new(Hlc::new(dev)),
        tmp.path().join("loro"),
        Some(tmp.path().join("notes")),
    )
    .await
    .unwrap();
    e.ensure_builtin_views().await.unwrap();
    e.views_upsert(user_view("v-keep", "Keep", "tag:keep", 10))
        .await
        .unwrap();
    let before = e.views_list().await;

    tokio::fs::write(tmp.path().join("notes").join("seeded.md"), "- from disk\n")
        .await
        .unwrap();
    let count = e.reseed_from_disk(&tmp.path().join("notes")).await.unwrap();
    assert_eq!(count, 1, "reseed processed the md file");
    assert_eq!(e.views_list().await, before, "views registry untouched");
}

#[tokio::test]
async fn views_persist_across_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let snap = tmp.path().join("loro");
    let dev = test_device();
    let expected;
    {
        let e = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap.clone(), None)
            .await
            .unwrap();
        e.ensure_builtin_views().await.unwrap();
        e.views_upsert(user_view("v-persist", "P", "tag:p", 10))
            .await
            .unwrap();
        expected = e.views_list().await;
        assert_eq!(expected.len(), 2);
    }
    // Reopen from the same snapshot dir: the views doc loads like any
    // per-doc snapshot, and the seed stays a no-op.
    let e = LoroEngine::with_dirs(dev, Arc::new(Hlc::new(dev)), snap, None)
        .await
        .unwrap();
    e.ensure_builtin_views().await.unwrap();
    assert_eq!(e.views_list().await, expected, "registry survives restart");
}

// -----------------------------------------------------------------
// Residency audit (tesela-engc.5): lazy-load regression tests
// (tesela-qql). The full classification table of every walk over
// `self.inner.docs` lives in the bead's close note; these three
// encode the highest-severity assumptions a future evict() must not
// violate — that a note's `LoroDoc` can be dropped from
// `self.inner.docs` while its `.bin` snapshot survives on disk, and
// every one of these three call sites must keep working transparently.
// Un-ignored now that `doc_for_note_mut` / the apply_import heal gate /
// `produce_relay_updates` all lazy-load or consult a
// residency-independent signal.
// -----------------------------------------------------------------

#[tokio::test]
async fn doc_for_note_mut_must_not_recreate_evicted_note() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir)
        .await
        .unwrap();
    let note_id = [0x33; 16];
    let existing_block = [0x44; 16];

    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: existing_block,
            note_id,
            parent_block_id: None,
            order_key: "a0".into(),
            indent_level: 0,
            text: "pre-eviction content".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let before = engine.render_note(note_id).await.unwrap();
    assert!(before.contains("pre-eviction content"));

    // Simulate eviction: the note's snapshot is safely on disk (the
    // BlockUpsert above just wrote it via `save_snapshot`), but the
    // in-memory doc is dropped — exactly what a future evict() would
    // leave behind. `doc_for_note_mut` (loro_engine.rs:1587) is the
    // ONLY entry point that resolves a note's doc for a local edit.
    engine.inner.docs.write().await.remove(&note_id);

    let new_block = [0x55; 16];
    engine
        .record_local(OpPayload::BlockUpsert {
            block_id: new_block,
            note_id,
            parent_block_id: None,
            order_key: "a1".into(),
            indent_level: 0,
            text: "post-eviction content".into(),
            after_block_id: Some(existing_block),
        })
        .await
        .unwrap();

    let after = engine.render_note(note_id).await.unwrap();
    assert!(
        after.contains("pre-eviction content"),
        "doc_for_note_mut unconditionally `or_insert_with`s a FRESH empty \
         LoroDoc on a docs-map miss — an evicted note's entire prior \
         history is silently discarded on the next local edit (got {after:?})"
    );
    assert!(after.contains("post-eviction content"));
}

// NOTE: this test's ORIGINAL (tesela-engc.5) assertion expected the
// server's evicted-then-reimported block A text to survive collapse
// ("Awesome sweet") over the device's disjoint twin. That predates
// tesela-fte (`e4a61454`, landed AFTER the residency audit), which
// deleted the genuine-edit/stale-guard discriminator and made the twin
// TEXT survivor a PURE function of max-`TreeID` (peer, then counter) —
// see `ws_apply_disjoint_conflict_resolves_to_max_treeid_twin`. Since
// each engine's peer id is constant across all its own history, the
// higher-peer engine (device, 0x7f) wins EVERY disjoint-twin block's
// TEXT uniformly, so no two-engine scenario can make "server keeps A,
// device keeps B" true anymore — that combination is no longer
// reachable regardless of residency/eviction.
//
// What the heal GATE (`has_local_state`/`plan_gate`) actually protects
// is orthogonal to the text-survivor rule: it's whether the tombstoned
// LOSER's `props` are unioned onto the survivor (`reassert_prop_heals`).
// That's the meaningful, still-discriminating regression surface for
// the tesela-qql landmine: the server's own PROPERTY on its (about to
// lose) A-twin must not be silently dropped just because the note
// wasn't memory-resident when the inbound frame arrived — mirrors
// `disjoint_twins_each_with_distinct_property_both_survive`, plus the
// evict-between-edit-and-import step.
#[tokio::test]
async fn apply_import_heal_gate_must_protect_evicted_note_local_edits() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::with_snapshot_dir(sdev, Arc::new(Hlc::new(sdev)), dir)
        .await
        .unwrap();
    let ddev = DeviceId::from_bytes([0x7f; 16]);
    let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
    let note = blake3_note_id("daily-evicted");

    seed_disjoint(&server, &device, note).await;

    // Server's own genuine property on its (disjoint) twin of A — the
    // value that must survive the twin-heal's props-union reassert even
    // though pure max-`TreeID` always keeps device's TEXT as the
    // surviving node for every block in this note.
    server
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();

    // Device genuinely edits B, then exports a full snapshot (the
    // cold-launch first-push frame that triggered the incident).
    device
        .record_local(OpPayload::BlockUpsert {
            block_id: B_BID_BYTES,
            note_id: note,
            parent_block_id: None,
            order_key: "00000000".into(),
            indent_level: 0,
            text: "B device".into(),
            after_block_id: None,
        })
        .await
        .unwrap();
    let snapshot = device.export_doc_update(note, None).await.unwrap();

    // Evict the SERVER's note between its own edit and the inbound
    // frame — exactly the window the heal gate samples. The note's
    // snapshot is safely on disk; only the in-memory entry is gone.
    server.inner.docs.write().await.remove(&note);

    server.import_doc_update(note, &snapshot).await.unwrap();

    assert_eq!(
        block_prop_scalar(&server, note, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "an evicted-but-locally-edited note must still get twin-heal \
         props protection on the next Delta import — the server's own \
         property must NOT be silently dropped just because the note \
         wasn't memory-resident when the frame arrived"
    );
    let b = block_text(&server, note, B_BID_BYTES)
        .await
        .unwrap_or_default();
    assert_eq!(
        b, "B device",
        "the device's genuine edit must still apply (got {b:?})"
    );
}

#[tokio::test]
async fn produce_relay_updates_must_include_evicted_dirty_note() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir)
        .await
        .unwrap();
    let note_id = [0x77; 16];
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("evicted".into()),
            title: "Evicted".into(),
            content: "---\ntitle: Evicted\n---\n- hello\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // No broadcast cursor has been committed yet, so this note has
    // ops pending relay. Simulate eviction (the resident doc is
    // dropped; its snapshot is on disk).
    engine.inner.docs.write().await.remove(&note_id);

    let updates = engine.produce_relay_updates().await;
    assert!(
        updates.iter().any(|(id, _, _)| *id == note_id),
        "produce_relay_updates (loro_engine.rs:1197) walks \
         self.inner.docs.keys() directly — an evicted note's \
         un-broadcast local edits silently never reach the relay"
    );
}

/// tesela-engc.5 audit, highest-severity UNSTUBBED item:
/// `rebuild_index_from_docs` used to prune any index entry whose note
/// wasn't in `self.inner.docs` — safe only because it's called
/// exclusively at boot, right after eager `load_snapshots_from_dir`,
/// where the two sets are identical. Simulate the residency gap a
/// future evict() (or a partial/lazy boot) would leave: the note's
/// snapshot is safely on disk, but the in-memory doc is gone.
#[tokio::test]
async fn rebuild_index_from_docs_must_not_prune_evicted_note_with_disk_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("loro");
    let hlc = Arc::new(Hlc::new(test_device()));
    let engine = LoroEngine::with_snapshot_dir(test_device(), hlc, dir)
        .await
        .unwrap();
    let note_id = [0x66; 16];
    engine
        .record_local(OpPayload::NoteUpsert {
            note_id,
            display_alias: Some("evicted".into()),
            title: "Evicted".into(),
            content: "- hello\n".into(),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    assert_eq!(
        engine.index_entries().await.len(),
        1,
        "indexed pre-eviction"
    );

    // Simulate eviction: the snapshot is safely on disk (the NoteUpsert
    // above wrote it via `save_snapshot`), but the in-memory doc is
    // dropped — exactly what a future evict() would leave behind.
    engine.inner.docs.write().await.remove(&note_id);

    engine.rebuild_index_from_docs().await;

    let entries = engine.index_entries().await;
    assert_eq!(
        entries.len(),
        1,
        "rebuild_index_from_docs must not prune a note's index entry \
         just because it isn't memory-resident — only a note with no \
         on-disk snapshot at all is a genuine ghost (got {entries:?})"
    );
    assert_eq!(entries[0].title, "Evicted");
}

/// STEP 1 of tesela-engc.4: measure `probe_import_poison`'s real cost
/// shape on mosaic-realistic docs before deciding whether a skip is
/// worth adding. Doc sizes are derived from the live
/// `~/Library/Application Support/tesela/logseq/.tesela/loro` mosaic
/// (305 note snapshots: mean 7.1KB, median 3KB, p90 16.8KB, max 83KB).
/// Simulates a genuine two-device inbound DELTA (device 2 imports
/// device 1's snapshot, adds one block, exports `updates(&vv1)` — the
/// same shape a relay tick actually ships) alongside a full-snapshot
/// catch-up frame, and times the probe's three sub-steps against a
/// plain `doc.import` of the same bytes.
///
/// `#[ignore]`d — a manual perf probe (numbers land in the bead close
/// note / decisions.md), not a CI-gated timing assertion. Run with:
/// `cargo test -p tesela-sync --lib poison_probe_cost_measurement -- --ignored --nocapture`
#[tokio::test]
#[ignore = "manual perf measurement (tesela-engc.4), not a CI timing gate"]
async fn poison_probe_cost_measurement() {
    use std::time::{Duration, Instant};

    async fn build_note(
        device: [u8; 16],
        note_id: [u8; 16],
        block_count: usize,
        text_len: usize,
    ) -> LoroEngine {
        let hlc = Arc::new(Hlc::new(DeviceId::from_bytes(device)));
        let engine = LoroEngine::new(DeviceId::from_bytes(device), hlc);
        let filler: String = "lorem ipsum dolor sit amet consectetur "
            .repeat(text_len / 40 + 1)
            .chars()
            .take(text_len)
            .collect();
        for i in 0..block_count {
            let mut bid = [0u8; 16];
            bid[..8].copy_from_slice(&(i as u64).to_be_bytes());
            bid[15] = 1;
            upsert_block(&engine, note_id, bid, &filler, None).await;
        }
        engine
    }

    fn time_it<T>(f: impl FnOnce() -> T) -> (T, Duration) {
        let start = Instant::now();
        let out = f();
        (out, start.elapsed())
    }

    // (label, block_count, text_len) shaped to hit the mosaic's mean /
    // median / p90 / max snapshot sizes.
    let shapes: [(&str, usize, usize); 4] = [
        ("median (~3KB)", 8, 250),
        ("mean (~7KB)", 20, 250),
        ("p90 (~17KB)", 45, 250),
        ("max (~83KB)", 220, 250),
    ];

    eprintln!(
        "\nlabel            snapshot_B  delta_B   probe(delta)_us  raw_import(delta)_us  probe(snapshot)_us  raw_import(snapshot)_us"
    );
    for (label, block_count, text_len) in shapes {
        let note_id = [5u8; 16];
        let engine1 = build_note([1u8; 16], note_id, block_count, text_len).await;
        let doc1 = engine1.doc_for_note_mut(note_id).await;
        let vv1 = doc1.oplog_vv();
        let snapshot_bytes = doc1.export(ExportMode::Snapshot).unwrap();

        // A genuine inbound DELTA: device 2 imports device 1's snapshot,
        // adds ONE block (a peer edit), exports only what device 1 lacks.
        let hlc2 = Arc::new(Hlc::new(DeviceId::from_bytes([2u8; 16])));
        let engine2 = LoroEngine::new(DeviceId::from_bytes([2u8; 16]), hlc2);
        engine2
            .import_authoritative_snapshot(note_id, &snapshot_bytes)
            .await
            .unwrap();
        let mut peer_bid = [0u8; 16];
        peer_bid[15] = 2;
        upsert_block(&engine2, note_id, peer_bid, "peer edit block text", None).await;
        let doc2 = engine2.doc_for_note_mut(note_id).await;
        let delta_bytes = doc2.export(ExportMode::updates(&vv1)).unwrap();

        const N: u32 = 50;
        let mut probe_delta_total = Duration::ZERO;
        let mut raw_delta_total = Duration::ZERO;
        let mut probe_snap_total = Duration::ZERO;
        let mut raw_snap_total = Duration::ZERO;
        for _ in 0..N {
            let (_, d) = time_it(|| probe_import_poison(&doc1, &delta_bytes));
            probe_delta_total += d;
            let fork = LoroDoc::new();
            fork.import(&snapshot_bytes).unwrap();
            let (_, d) = time_it(|| fork.import(&delta_bytes));
            raw_delta_total += d;

            let (_, d) = time_it(|| probe_import_poison(&doc1, &snapshot_bytes));
            probe_snap_total += d;
            let fork2 = LoroDoc::new();
            let (_, d) = time_it(|| fork2.import(&snapshot_bytes));
            raw_snap_total += d;
        }
        eprintln!(
            "{label:<16} {:>9}B {:>7}B {:>15}us {:>19}us {:>17}us {:>21}us",
            snapshot_bytes.len(),
            delta_bytes.len(),
            (probe_delta_total / N).as_micros(),
            (raw_delta_total / N).as_micros(),
            (probe_snap_total / N).as_micros(),
            (raw_snap_total / N).as_micros(),
        );
    }
}

// Verification gap closed (audit L6, tesela-9t0): deleted-wins
// (`reconcile_tree_to_blocks`'s tombstoned-skip, ~line 3272) depends on
// tombstones surviving a GC-compacted `ExportMode::Snapshot` round-trip
// through a FRESH engine that never saw the delete op directly — only
// via the snapshot's current-state bytes. Prove it: delete a block,
// export a snapshot, import fresh on a 2nd engine, then apply a stale
// NoteUpsert on that 2nd engine whose body still carries the deleted
// bid — it must stay deleted, not resurrect.
#[tokio::test]
async fn deleted_wins_survives_snapshot_gc_round_trip() {
    let note = blake3_note_id("gc-tombstone");
    let bid = content_bid("gc-tombstone-block");

    // Engine A: create + delete a block, then export a GC-compacted
    // snapshot (ExportMode::Snapshot — the same bytes save_snapshot
    // writes; see export_doc_update's doc comment).
    let dev_a = DeviceId::from_bytes([0xa9; 16]);
    let a = LoroEngine::new(dev_a, Arc::new(Hlc::new(dev_a)));
    upsert_block(&a, note, bid, "doomed", None).await;
    a.record_local(OpPayload::BlockDelete { block_id: bid })
        .await
        .unwrap();
    let snapshot = a.export_doc_update(note, None).await.unwrap();

    // Engine B: FRESH — never saw the delete op directly, only the
    // GC-compacted snapshot's current state.
    let dev_b = DeviceId::from_bytes([0xb9; 16]);
    let b = LoroEngine::new(dev_b, Arc::new(Hlc::new(dev_b)));
    b.import_authoritative_snapshot(note, &snapshot)
        .await
        .unwrap();
    assert_eq!(
        block_text(&b, note, bid).await,
        None,
        "fresh import of the GC snapshot must land the block deleted"
    );

    // A STALE whole-content NoteUpsert on B still carries the deleted
    // bid in its body (as if authored before the delete propagated).
    let bid_uuid = uuid::Uuid::from_bytes(bid);
    b.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("gc-tombstone".into()),
        title: "GC".into(),
        content: format!("- doomed <!-- bid:{bid_uuid} -->\n"),
        created_at_millis: 2,
    })
    .await
    .unwrap();

    assert_eq!(
        block_text(&b, note, bid).await,
        None,
        "deleted-wins must survive a GC-compacted snapshot round-trip: \
         a stale NoteUpsert on a fresh engine must not resurrect a bid \
         tombstoned only in the imported snapshot's current state"
    );
}

// -----------------------------------------------------------------
// Per-note apply serialization (tesela-4ju): adversarial-review
// finding #4 on tesela-y11 asked whether `apply_import`'s
// plan→import→tombstone sequence can interleave across CONCURRENT
// applies for the SAME note (the docs-map write lock, taken only
// inside `doc_for_note_mut`, is released before the sequence starts —
// so without an additional per-note lock, two racing applies could
// interleave and the second's twins could be tombstoned by the
// first's stale plan, or vice versa). `apply_lock_for_note` +
// `apply_import` holding it for the whole body (loro_engine.rs) close
// that window. These two tests prove it at both levels: the lock
// primitive itself, and an end-to-end hammer of the public apply API.
// -----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn apply_lock_serializes_same_note_not_different_notes() {
    let engine = LoroEngine::new(test_device(), Arc::new(Hlc::new(test_device())));
    let note_a = [0xaa; 16];
    let note_b = [0xbb; 16];

    let lock_a1 = engine.apply_lock_for_note(note_a).await;
    let lock_a2 = engine.apply_lock_for_note(note_a).await;
    assert!(
        Arc::ptr_eq(&lock_a1, &lock_a2),
        "the same note_id must resolve to the SAME lock across calls, \
         or two concurrent applies for that note would each grab an \
         independent (non-serializing) mutex"
    );

    let lock_b = engine.apply_lock_for_note(note_b).await;
    assert!(
        !Arc::ptr_eq(&lock_a1, &lock_b),
        "different notes must NOT share a lock — that would serialize \
         unrelated notes' applies against each other for no reason"
    );

    // Prove actual mutual exclusion: hold note_a's lock, spawn a task
    // that also wants note_a's lock and records when it acquires it;
    // it must NOT acquire until the holder releases.
    let order = Arc::new(tokio::sync::Mutex::new(Vec::<&'static str>::new()));
    let guard = lock_a1.lock().await;
    let order2 = order.clone();
    let lock_a3 = engine.apply_lock_for_note(note_a).await;
    let waiter = tokio::spawn(async move {
        let _g = lock_a3.lock().await;
        order2.lock().await.push("waiter-acquired");
    });
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    order.lock().await.push("holder-still-held");
    drop(guard);
    waiter.await.unwrap();
    let seq = order.lock().await.clone();
    assert_eq!(
        seq,
        vec!["holder-still-held", "waiter-acquired"],
        "the waiter must not acquire note_a's apply lock while the \
         first holder is still active — mutual exclusion is broken"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn apply_import_hammer_one_note_converges_without_leftover_twins() {
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let note = blake3_note_id("apply-race-note");

    // 8 devices each independently author the SAME note body (disjoint
    // Loro lineages — same block_ids A_BID/B_BID, distinct TreeIDs per
    // device), producing 8 full-snapshot frames the server will import
    // CONCURRENTLY. Each frame carries a genuinely different text for A
    // so a corrupted/interleaved heal would be visible as garbled or
    // vanished text, not just a silently-wrong-but-plausible value.
    let mut frames = Vec::new();
    for i in 0u8..8 {
        let ddev = DeviceId::from_bytes([i + 1; 16]);
        let device = LoroEngine::new(ddev, Arc::new(Hlc::new(ddev)));
        device
            .record_local(OpPayload::NoteUpsert {
                note_id: note,
                display_alias: Some("race".into()),
                title: "Race".into(),
                content: format!(
                    "- Awesome from {i} <!-- bid:{A_BID} -->\n\
                     - B from {i} <!-- bid:{B_BID} -->\n"
                ),
                created_at_millis: 1,
            })
            .await
            .unwrap();
        frames.push(device.export_doc_update(note, None).await.unwrap());
    }

    // Hammer: import all 8 frames into the SAME server note
    // CONCURRENTLY from separate tokio tasks on a multi-thread runtime.
    // Pre-fix (no per-note apply lock) this races each frame's
    // props-plan fork against every other frame's raw import +
    // tombstone; post-fix (tesela-4ju) the per-note mutex in
    // `apply_import` forces them one at a time, so the result must be
    // identical to a sequential run: exactly one surviving node per
    // block_id, nothing vanished.
    let mut set = tokio::task::JoinSet::new();
    for bytes in frames {
        let server = server.clone();
        set.spawn(async move { server.import_doc_update(note, &bytes).await });
    }
    while let Some(res) = set.join_next().await {
        res.expect("apply task must not panic")
            .expect("concurrent import_doc_update must not error");
    }

    {
        let docs = server.inner.docs.read().await;
        let doc = docs
            .get(&note)
            .expect("note resident after concurrent imports");
        assert!(
            duplicate_block_ids(doc).is_empty(),
            "concurrent apply_import calls for one note must still \
             converge to a single surviving node per block_id — leftover \
             twins mean the plan→import→tombstone sequence interleaved \
             across callers"
        );
    }

    let a = block_text(&server, note, A_BID_BYTES).await;
    let b = block_text(&server, note, B_BID_BYTES).await;
    assert!(
        a.is_some(),
        "block A must survive concurrent hammering, not vanish entirely"
    );
    assert!(
        b.is_some(),
        "block B must survive concurrent hammering, not vanish entirely"
    );
}

// -----------------------------------------------------------------
// tesela-4ju REVIEW REJECT (2026-07-02): the per-note `apply_locks`
// guard closed apply-vs-apply, but `record_local` and
// `heal_disjoint_twins` ran their own plan/import/tombstone-shaped
// sequences WITHOUT taking it — the same interleave class stayed open
// through those two paths. Both now take the SAME per-note guard
// (`note_id_for_payload` + `apply_lock_for_note`). These two tests race
// each path against a concurrent `apply_import` for the SAME note.
// -----------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn record_local_races_apply_import_for_same_note_preserves_local_edits() {
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let note = blake3_note_id("record-vs-import-race");

    // Server already resident (so the Delta import's twin-heal plan gate
    // — `already_resident && !is_views` — is active): exactly the
    // residency this bead's finding is about.
    server
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("race".into()),
            title: "Race".into(),
            content: format!(
                "- Server original <!-- bid:{A_BID} -->\n\
                 - B server <!-- bid:{B_BID} -->\n"
            ),
            created_at_millis: 1,
        })
        .await
        .unwrap();

    // A peer authors the SAME note on a disjoint Loro lineage (fresh
    // TreeIDs for the same block_ids) with genuinely different text —
    // importing it mints server-side twins that `apply_import` must
    // resolve concurrently with the local edits below.
    let pdev = DeviceId::from_bytes([0x7f; 16]);
    let peer = LoroEngine::new(pdev, Arc::new(Hlc::new(pdev)));
    peer.record_local(OpPayload::NoteUpsert {
        note_id: note,
        display_alias: Some("race".into()),
        title: "Race".into(),
        content: format!(
            "- Peer incoming <!-- bid:{A_BID} -->\n\
             - B peer <!-- bid:{B_BID} -->\n"
        ),
        created_at_millis: 1,
    })
    .await
    .unwrap();
    let peer_frame = peer.export_doc_update(note, None).await.unwrap();

    // Race: N concurrent LOCAL edits (`record_local`) against block A's
    // "race_tag" list, plus the ONE inbound import, all launched
    // together for the SAME note. The per-note `apply_locks` guard
    // serializes them into SOME total order (never interleaved
    // mid-sequence) — regardless of that order, every local AddToList
    // must survive: applied directly to the still-live node if it runs
    // AFTER the import's tombstone, or captured by the props-plan union
    // fork (`peer_genuine_block_changes`/`twin_winners_for`) and
    // re-asserted onto the survivor if it runs BEFORE the import.
    const N: u8 = 6;
    let mut set = tokio::task::JoinSet::new();
    for i in 0..N {
        let server = server.clone();
        set.spawn(async move {
            server
                .record_local(OpPayload::BlockPropertySet {
                    note_id: note,
                    block_id: A_BID_BYTES,
                    key: "race_tag".into(),
                    value: PropOp::AddToList(PropScalar::Text(format!("local-{i}"))),
                })
                .await
                .map(|_| ())
        });
    }
    {
        let server = server.clone();
        let peer_frame = peer_frame.clone();
        set.spawn(async move { server.import_doc_update(note, &peer_frame).await });
    }
    while let Some(res) = set.join_next().await {
        res.expect("race task must not panic")
            .expect("record_local/import_doc_update must not error");
    }

    {
        let docs = server.inner.docs.read().await;
        let doc = docs.get(&note).expect("note resident after the race");
        assert!(
            duplicate_block_ids(doc).is_empty(),
            "record_local racing apply_import for the same note must still \
             converge to a single surviving node per block_id"
        );
    }

    let mut tags: Vec<String> = block_prop_list(&server, note, A_BID_BYTES, "race_tag")
        .await
        .into_iter()
        .map(|s| match s {
            PropScalar::Text(t) => t,
            other => format!("{other:?}"),
        })
        .collect();
    tags.sort();
    let expected: Vec<String> = (0..N).map(|i| format!("local-{i}")).collect();
    assert_eq!(
        tags, expected,
        "every concurrent record_local AddToList must survive a racing \
         apply_import for the SAME note — a dropped entry here means the \
         local edit landed between apply_import's props-plan fork and its \
         twin tombstone and got silently discarded (the tesela-4ju REVIEW \
         REJECT finding this test guards)"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn heal_disjoint_twins_races_apply_import_for_same_note_without_corruption() {
    let sdev = DeviceId::from_bytes([0x5e; 16]);
    let server = LoroEngine::new(sdev, Arc::new(Hlc::new(sdev)));
    let note = blake3_note_id("heal-vs-import-race");
    const C_BID_BYTES: [u8; 16] = [0x0c; 16];

    // Seed block A via the normal self-healing path (single live node —
    // no apply path ever leaves a standing twin post-tesela-fte, per
    // relay_inbound_rebase.rs's c14 comment).
    server
        .record_local(OpPayload::NoteUpsert {
            note_id: note,
            display_alias: Some("race".into()),
            title: "Race".into(),
            content: format!("- Base A <!-- bid:{A_BID} -->\n"),
            created_at_millis: 1,
        })
        .await
        .unwrap();
    server
        .record_local(OpPayload::BlockPropertySet {
            note_id: note,
            block_id: A_BID_BYTES,
            key: "status".into(),
            value: PropOp::SetScalar(PropScalar::Text("doing".into())),
        })
        .await
        .unwrap();

    // Fabricate a GENUINE standing twin for A directly on the doc,
    // bypassing every self-healing apply path — the only realistic
    // source of a persistent twin (legacy pre-fix `.bin` residue loaded
    // from disk). Gives `heal_disjoint_twins` real work, so this test
    // exercises its ACTUAL plan→tombstone→reassert body racing a
    // concurrent `apply_import`, not a no-op scan.
    {
        let doc = server.doc_for_note_mut(note).await;
        let tree = doc.get_tree("blocks");
        let raw_twin = tree.create(TreeParentId::Root).unwrap();
        let meta = tree.get_meta(raw_twin).unwrap();
        meta.insert("block_id", hex_id(&A_BID_BYTES).as_str())
            .unwrap();
        meta.insert("indent_level", 0i64).unwrap();
        meta.insert("parent", "").unwrap();
        write_block_text(&meta, "raw twin residue").unwrap();
        let (props, prop_keys) = prop_containers::node_prop_containers(&meta).unwrap();
        apply_prop_op(
            &props,
            &prop_keys,
            "priority",
            &PropOp::SetScalar(PropScalar::Int(3)),
        )
        .unwrap();
        doc.commit();
    }
    assert_eq!(
        duplicate_block_ids(&server.doc_for_note_mut(note).await).len(),
        1,
        "fixture setup must actually produce a standing twin for A \
         before the race"
    );

    // Pad the tree with MANY additional single-node (non-twin) blocks
    // so `twin_winners_for`'s doc-wide scan and `tombstone_duplicate_
    // twins`'s own scan take long enough in wall-clock time to give the
    // concurrent `record_local` writers below (spawned alongside the
    // heal/import race) a REAL chance to land their write on block A's
    // about-to-be-tombstoned node between heal's plan-fork read of A's
    // props and its tombstone call. With only A's twin present, that
    // window is a handful of CPU instructions wide and effectively
    // unhittable by scheduling luck alone — created AFTER A (so the
    // scan processes A early and still has to plow through every pad
    // node before `twin_winners_for` returns and `tombstone_duplicate_
    // twins` runs), this widens it to real, hittable microseconds.
    const PAD_BLOCKS: u32 = 1000;
    {
        let doc = server.doc_for_note_mut(note).await;
        let tree = doc.get_tree("blocks");
        for i in 0..PAD_BLOCKS {
            let mut pad_bid = [0xf0u8; 16];
            pad_bid[14..16].copy_from_slice(&(i as u16).to_be_bytes());
            let node = tree.create(TreeParentId::Root).unwrap();
            let meta = tree.get_meta(node).unwrap();
            meta.insert("block_id", hex_id(&pad_bid).as_str()).unwrap();
            meta.insert("indent_level", 0i64).unwrap();
            meta.insert("parent", "").unwrap();
            write_block_text(&meta, "pad").unwrap();
        }
        doc.commit();
    }

    // A peer concurrently authors a genuinely NEW block (disjoint from
    // A) on the SAME note — the concurrent inbound import this races
    // against the heal.
    let pdev = DeviceId::from_bytes([0x7f; 16]);
    let peer = LoroEngine::new(pdev, Arc::new(Hlc::new(pdev)));
    peer.record_local(OpPayload::BlockUpsert {
        block_id: C_BID_BYTES,
        note_id: note,
        parent_block_id: None,
        order_key: "00000000".into(),
        indent_level: 0,
        text: "Peer concurrent block C".into(),
        after_block_id: None,
    })
    .await
    .unwrap();
    let peer_frame = peer.export_doc_update(note, None).await.unwrap();

    // Race: `heal_disjoint_twins()` (sweeping every resident note,
    // including this one) concurrently with `import_doc_update` for the
    // SAME note, PLUS N concurrent `record_local` edits to block A's
    // props — mirroring
    // `record_local_races_apply_import_for_same_note_preserves_local_edits`.
    // The STATIC pre-race props (`status`/`priority` above) alone don't
    // discriminate the reverted lock: `heal_disjoint_twins`'s plan and
    // `apply_import`'s own plan (`peer_genuine_block_changes` →
    // `twin_winners_for`) are both PURE functions of the same
    // (unchanging) twin set, so either racer reasserts them identically
    // regardless of interleaving — there's no genuinely racing WRITE for
    // a static fixture to catch. A CONCURRENT write can land on the twin
    // node about to be tombstoned AFTER heal's plan-fork already
    // captured a stale snapshot: with the per-note lock intact, `heal_
    // disjoint_twins` holds it across its whole plan→tombstone→reassert
    // body, so no `record_local` for this note can land mid-sequence;
    // with the lock reverted, heal never blocks the other lock holders
    // and the drop window is real (tesela-xh4 REVIEW REJECT,
    // 2026-07-02).
    const N: u8 = 20;
    let mut local_set = tokio::task::JoinSet::new();
    for i in 0..N {
        let server = server.clone();
        local_set.spawn(async move {
            server
                .record_local(OpPayload::BlockPropertySet {
                    note_id: note,
                    block_id: A_BID_BYTES,
                    key: "race_tag".into(),
                    value: PropOp::AddToList(PropScalar::Text(format!("local-{i}"))),
                })
                .await
                .map(|_| ())
        });
    }
    let heal_task = {
        let server = server.clone();
        tokio::spawn(async move { server.heal_disjoint_twins().await })
    };
    let import_task = {
        let server = server.clone();
        tokio::spawn(async move { server.import_doc_update(note, &peer_frame).await })
    };
    while let Some(res) = local_set.join_next().await {
        res.expect("race_local task must not panic")
            .expect("concurrent record_local must not error");
    }
    heal_task.await.expect("heal task must not panic");
    import_task
        .await
        .expect("import task must not panic")
        .expect("concurrent import_doc_update must not error");

    {
        let docs = server.inner.docs.read().await;
        let doc = docs.get(&note).expect("note resident after the race");
        assert!(
            duplicate_block_ids(doc).is_empty(),
            "heal_disjoint_twins racing a concurrent apply_import for the \
             SAME note must still converge to a single surviving node per \
             block_id — a leftover twin means the two sequences \
             interleaved (the tesela-4ju REVIEW REJECT finding this test \
             guards)"
        );
    }

    let a = block_text(&server, note, A_BID_BYTES).await;
    let c = block_text(&server, note, C_BID_BYTES).await;
    assert!(
        a.is_some(),
        "block A must survive the heal, not vanish entirely"
    );
    assert_eq!(
        c.as_deref(),
        Some("Peer concurrent block C"),
        "the concurrent import's genuinely new block C must land intact, \
         not be corrupted/dropped by the racing heal"
    );
    assert_eq!(
        block_prop_scalar(&server, note, A_BID_BYTES, "status").await,
        Some(PropScalar::Text("doing".into())),
        "the pre-existing A twin's scalar property must survive the racing heal"
    );
    assert_eq!(
        block_prop_scalar(&server, note, A_BID_BYTES, "priority").await,
        Some(PropScalar::Int(3)),
        "the fabricated A twin's distinct scalar property must be reasserted \
         onto the survivor by the racing heal"
    );
    let mut tags: Vec<String> = block_prop_list(&server, note, A_BID_BYTES, "race_tag")
        .await
        .into_iter()
        .map(|s| match s {
            PropScalar::Text(t) => t,
            other => format!("{other:?}"),
        })
        .collect();
    tags.sort();
    let mut expected: Vec<String> = (0..N).map(|i| format!("local-{i}")).collect();
    expected.sort();
    assert_eq!(
        tags, expected,
        "every concurrent record_local AddToList onto block A must survive \
         a racing heal_disjoint_twins for the SAME note — a dropped entry \
         here means the local edit landed between heal's plan-fork \
         (twin_winners_for) and its twin tombstone/reassert and got \
         silently discarded (the tesela-xh4 REVIEW REJECT finding this \
         test guards)"
    );
}
