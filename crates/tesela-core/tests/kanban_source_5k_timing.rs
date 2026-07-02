//! tesela-ya4.1 — manual before/after timing check (acceptance criterion 6):
//! does moving KanbanBoard off `get_typed_blocks` onto
//! `executeQuery("tag:X kind:block")` re-trigger full-corpus reparse cost
//! now that tesela-sclr.2's per-note parsed-blocks cache has landed?
//!
//! `get_typed_blocks` is NOT covered by the sclr.2 cache (it calls
//! `crate::block::parse_blocks` directly — see `db/sqlite.rs`); every call
//! reparses every matched note's body from scratch, cold or warm alike.
//! `execute_query` IS covered (`parsed_blocks_cached`), so a repeated call
//! against an unchanged corpus should be dramatically cheaper than the
//! first.
//!
//! `#[ignore]`d — this builds + indexes a 5000-note synthetic mosaic
//! (tens of seconds), too slow for the default `cargo test` gate. Run
//! explicitly for the design_notes evidence:
//!   cargo test -p tesela-core --test kanban_source_5k_timing -- --ignored --nocapture

use std::sync::Arc;
use std::time::Instant;

use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::Indexer,
    query::parse_query,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
};
use tesela_fixtures::MosaicBuilder;

#[tokio::test]
#[ignore]
async fn kanban_source_5k_before_after() {
    // Same shape as the sclr.2 `inbox_query_5000_notes` bench fixture —
    // ~5000 notes, 2000 of them Task blocks.
    let mosaic = MosaicBuilder::new()
        .seed(42)
        .daily_notes(4000)
        .pages(900)
        .tasks(2000)
        .backlinks_per_note(1, 5)
        .deep_pages(10)
        .build()
        .unwrap();

    let db_path = mosaic.path.join(".tesela").join("tesela.db");
    let index = Arc::new(SqliteIndex::open(&db_path).await.unwrap());
    let store = Arc::new(FsNoteStore::new(
        mosaic.path.clone(),
        Config::default().storage,
    ));
    let store_dyn: Arc<dyn NoteStore> = store as Arc<dyn NoteStore>;
    let index_dyn: Arc<dyn SearchIndex> = Arc::clone(&index) as Arc<dyn SearchIndex>;
    let graph_dyn: Arc<dyn LinkGraph> = Arc::clone(&index) as Arc<dyn LinkGraph>;
    let indexer = Indexer::new(store_dyn, index_dyn, graph_dyn);
    indexer.initial_index().await.unwrap();

    // ── OLD kanban source: get_typed_blocks("Task") — uncached ──────────
    let t0 = Instant::now();
    let old_first = index.get_typed_blocks("Task").await.unwrap();
    let old_cold_ms = t0.elapsed().as_millis();

    let t1 = Instant::now();
    let old_second = index.get_typed_blocks("Task").await.unwrap();
    let old_warm_ms = t1.elapsed().as_millis();

    // ── NEW kanban source: executeQuery("tag:Task kind:block") — cached ──
    let query = parse_query("tag:Task kind:block");
    let t2 = Instant::now();
    let new_first = index.execute_query(&query, None, None).await.unwrap();
    let new_cold_ms = t2.elapsed().as_millis();

    let t3 = Instant::now();
    let new_second = index.execute_query(&query, None, None).await.unwrap();
    let new_warm_ms = t3.elapsed().as_millis();

    println!(
        "kanban_source_5k_before_after: \
         get_typed_blocks(OLD) cold={old_cold_ms}ms warm={old_warm_ms}ms \
         (n={}/{}) | executeQuery(NEW) cold={new_cold_ms}ms warm={new_warm_ms}ms",
        old_first.len(),
        old_second.len(),
    );

    let new_count: usize = new_first.groups.iter().map(|g| g.items.len()).sum();
    let new_count_2: usize = new_second.groups.iter().map(|g| g.items.len()).sum();
    assert!(new_count > 0 && new_count_2 == new_count);
    assert!(!old_first.is_empty() && old_second.len() == old_first.len());
}
