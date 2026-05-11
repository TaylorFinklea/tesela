//! Indexer hot path — runs on every server startup and after `tesela
//! reindex`. The most painful "your data got bigger" regression
//! surface, so it gets the most coverage.
//!
//! `cargo bench --bench index -p tesela-core`

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::Indexer,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
};
use tesela_fixtures::MosaicBuilder;
use tokio::runtime::Runtime;

fn initial_index_at_scale(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("core/index/initial_index");
    // Three scale points — keeps the bench fast on tiny but still
    // surfaces O(n²) regressions at larger sizes. Scale numbers
    // mirror MosaicBuilder presets, kept inline so they're visible.
    for (label, dailies, pages) in &[
        ("tiny_30", 20usize, 8usize),
        ("medium_500", 420usize, 80usize),
    ] {
        let total_notes = dailies + pages;
        group.throughput(Throughput::Elements(total_notes as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(label),
            label,
            |b, _| {
                b.iter_batched(
                    || {
                        // Build a fresh mosaic + empty SQLite per iteration so
                        // we measure cold-start indexing every time. Setup
                        // cost is excluded from the timer via iter_batched.
                        let mosaic = MosaicBuilder::new()
                            .seed(42)
                            .daily_notes(*dailies)
                            .pages(*pages)
                            .tasks(0)
                            .backlinks_per_note(0, 2)
                            .build()
                            .unwrap();
                        let db_path = mosaic.path.join(".tesela").join("tesela.db");
                        rt.block_on(async {
                            let index = SqliteIndex::open(&db_path).await.unwrap();
                            let store = FsNoteStore::new(
                                mosaic.path.clone(),
                                Config::default().storage,
                            );
                            (mosaic, store, index)
                        })
                    },
                    |(mosaic, store, index)| {
                        rt.block_on(async {
                            let store = Arc::new(store);
                            let index = Arc::new(index);
                            let store_dyn: Arc<dyn NoteStore> =
                                Arc::clone(&store) as Arc<dyn NoteStore>;
                            let index_dyn: Arc<dyn SearchIndex> =
                                Arc::clone(&index) as Arc<dyn SearchIndex>;
                            let graph_dyn: Arc<dyn LinkGraph> =
                                Arc::clone(&index) as Arc<dyn LinkGraph>;
                            let indexer = Indexer::new(store_dyn, index_dyn, graph_dyn);
                            indexer.initial_index().await.unwrap();
                            // Keep the mosaic alive until after the work runs.
                            drop(mosaic);
                        })
                    },
                    criterion::BatchSize::SmallInput,
                );
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = initial_index_at_scale
}
criterion_main!(benches);
