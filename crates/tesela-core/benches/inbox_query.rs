//! Inbox ambient query (`kind:block -has:status`) cost, before/after the
//! per-note parsed-blocks cache added for tesela-sclr.2.
//!
//! The Inbox is the default ambient and its DSL carries no positive
//! `tag:` filter, so `execute_block_query` always falls into the "SELECT
//! id, title, body, note_type FROM notes" full-scan branch — pre-fix,
//! every single call to that branch also reparsed EVERY note's body from
//! scratch (regex-based `parse_blocks`), even when nothing had changed
//! since the last call (see `crates/tesela-core/src/db/sqlite.rs`,
//! `execute_block_query` / `parsed_blocks_cached`).
//!
//! `cold` measures a freshly-opened `SqliteIndex` (empty `blocks_cache`)
//! against already-populated notes — a cache MISS does exactly what the
//! old unconditional `parse_blocks` call did, plus a cheap SHA-256 +
//! mutex, so this number is directly comparable to the pre-fix
//! per-call cost. `warm` reuses one `SqliteIndex` across the whole
//! benchmark loop and measures the REPEATED identical query — the
//! steady-state cost of an Inbox refresh after a save touches ONE other
//! note (WS invalidation → invalidateQueries → refetch), which is the
//! case this fix targets.
//!
//! `cargo bench --bench inbox_query -p tesela-core`

use std::path::Path;
use std::sync::Arc;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use tesela_core::{
    config::Config,
    db::SqliteIndex,
    indexer::Indexer,
    query::parse_query,
    storage::filesystem::FsNoteStore,
    traits::{link_graph::LinkGraph, note_store::NoteStore, search_index::SearchIndex},
};
use tesela_fixtures::{MosaicBuilder, MosaicHandle};
use tokio::runtime::Runtime;

/// Open (or create) the index at `mosaic_path` and run `initial_index` so
/// benches measure the query, not the index-build.
async fn build_and_index(mosaic_path: &Path) {
    let db_path = mosaic_path.join(".tesela").join("tesela.db");
    let index = Arc::new(SqliteIndex::open(&db_path).await.unwrap());
    let store = Arc::new(FsNoteStore::new(
        mosaic_path.to_path_buf(),
        Config::default().storage,
    ));
    let store_dyn: Arc<dyn NoteStore> = store as Arc<dyn NoteStore>;
    let index_dyn: Arc<dyn SearchIndex> = Arc::clone(&index) as Arc<dyn SearchIndex>;
    let graph_dyn: Arc<dyn LinkGraph> = Arc::clone(&index) as Arc<dyn LinkGraph>;
    let indexer = Indexer::new(store_dyn, index_dyn, graph_dyn);
    indexer.initial_index().await.unwrap();
}

fn bench_inbox(c: &mut Criterion, group_name: &str, mosaic: &MosaicHandle) {
    let rt = Runtime::new().unwrap();
    let query = parse_query("kind:block -has:status");
    let db_path = mosaic.path.join(".tesela").join("tesela.db");

    rt.block_on(build_and_index(&mosaic.path));

    let mut group = c.benchmark_group(group_name);
    group.sample_size(20);

    // "cold": a fresh SqliteIndex per iteration — empty blocks_cache, so
    // every note in the corpus gets reparsed. Mirrors the pre-fix cost of
    // ANY call to this query path.
    group.bench_function("cold_every_note_reparsed", |b| {
        b.iter_batched(
            || rt.block_on(async { SqliteIndex::open(&db_path).await.unwrap() }),
            |index| {
                rt.block_on(async {
                    let result = index.execute_query(&query, None, None).await.unwrap();
                    criterion::black_box(result);
                })
            },
            BatchSize::PerIteration,
        );
    });

    // "warm": one SqliteIndex reused for the whole loop, cache primed by
    // a throwaway call before measurement starts. Mirrors the realistic
    // steady-state Inbox-refresh cost post-fix.
    let warm_index = rt.block_on(async { SqliteIndex::open(&db_path).await.unwrap() });
    rt.block_on(async {
        warm_index.execute_query(&query, None, None).await.unwrap();
    });
    group.bench_function("warm_cache_hit", |b| {
        b.iter(|| {
            rt.block_on(async {
                let result = warm_index.execute_query(&query, None, None).await.unwrap();
                criterion::black_box(result);
            })
        });
    });

    group.finish();
}

/// ~552-note preset — matches the user's actual Logseq import scale
/// (tesela-sclr.2's named target).
fn inbox_query_552_notes(c: &mut Criterion) {
    let mosaic = MosaicBuilder::new()
        .seed(42)
        .daily_notes(420)
        .pages(80)
        .tasks(200)
        .backlinks_per_note(1, 5)
        .deep_pages(3)
        .build()
        .unwrap();
    bench_inbox(c, "core/query/inbox_552_notes", &mosaic);
}

/// ~5000-note synthetic mosaic — headroom check for much larger graphs.
fn inbox_query_5000_notes(c: &mut Criterion) {
    let mosaic = MosaicBuilder::new()
        .seed(42)
        .daily_notes(4000)
        .pages(900)
        .tasks(2000)
        .backlinks_per_note(1, 5)
        .deep_pages(10)
        .build()
        .unwrap();
    bench_inbox(c, "core/query/inbox_5000_notes", &mosaic);
}

criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = inbox_query_552_notes, inbox_query_5000_notes
}
criterion_main!(benches);
