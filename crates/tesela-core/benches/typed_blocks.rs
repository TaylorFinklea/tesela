//! `SqliteIndex::get_typed_blocks` — the unbounded `SELECT id, title,
//! body FROM notes WHERE body LIKE ? OR tags LIKE ?` path in
//! `crates/tesela-core/src/db/sqlite.rs:642`. Identified as a likely
//! regression spot during Phase 14 planning (no LIMIT, full-scan on
//! tag match). Sets a baseline so a future "let's just add a few more
//! filters" change doesn't silently 10x this.
//!
//! `cargo bench --bench typed_blocks -p tesela-core`

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::Indexer,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
};
use tesela_fixtures::MosaicBuilder;
use tokio::runtime::Runtime;

fn get_typed_blocks_tasks(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // 500-note mosaic with 200 task blocks — matches the "user has a
    // serious GTD setup" scale.
    let mosaic = MosaicBuilder::new()
        .seed(42)
        .daily_notes(420)
        .pages(80)
        .tasks(200)
        .backlinks_per_note(1, 4)
        .build()
        .unwrap();

    let db_path = mosaic.path.join(".tesela").join("tesela.db");
    let index = rt.block_on(async { SqliteIndex::open(&db_path).await.unwrap() });

    // One-shot indexing so the bench measures the query, not the
    // index-build.
    rt.block_on(async {
        let store = Arc::new(FsNoteStore::new(
            mosaic.path.clone(),
            Config::default().storage,
        ));
        let index = Arc::new(index);
        let store_dyn: Arc<dyn NoteStore> = Arc::clone(&store) as Arc<dyn NoteStore>;
        let index_dyn: Arc<dyn SearchIndex> = Arc::clone(&index) as Arc<dyn SearchIndex>;
        let graph_dyn: Arc<dyn LinkGraph> = Arc::clone(&index) as Arc<dyn LinkGraph>;
        let indexer = Indexer::new(store_dyn, index_dyn, graph_dyn);
        indexer.initial_index().await.unwrap();
    });

    // Reopen the index for the bench loop — Arc-shared with the index
    // above would also work, but holding it via a fresh open keeps the
    // bench reproducible if we ever start mutating in the loop.
    let index = rt.block_on(async { SqliteIndex::open(&db_path).await.unwrap() });

    let mut group = c.benchmark_group("core/types/get_typed_blocks");
    group.bench_function("Task_500_notes_200_tasks", |b| {
        b.iter(|| {
            rt.block_on(async {
                let blocks = index.get_typed_blocks("Task").await.unwrap();
                criterion::black_box(blocks);
            })
        });
    });
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = get_typed_blocks_tasks
}
criterion_main!(benches);
