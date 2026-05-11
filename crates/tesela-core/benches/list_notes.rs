//! `NoteStore::list` throughput at various limits — verifies pagination
//! cost stays proportional to the requested `limit`, not the total
//! mosaic size. Regression here would mean a fix to the recent
//! JournalView pagination change (commit cc63437) reverted.
//!
//! `cargo bench --bench list_notes -p tesela-core`

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tesela_core::{
    config::Config, storage::filesystem::FsNoteStore, traits::note_store::NoteStore,
};
use tesela_fixtures::MosaicBuilder;
use tokio::runtime::Runtime;

fn list_at_varied_limits(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    // One shared 500-note mosaic — all sub-benches reuse it. Building
    // a mosaic each iteration would dwarf the operation under test.
    let mosaic = MosaicBuilder::new()
        .seed(42)
        .daily_notes(420)
        .pages(80)
        .tasks(0)
        .backlinks_per_note(0, 2)
        .build()
        .unwrap();
    let store = FsNoteStore::new(mosaic.path.clone(), Config::default().storage);

    let mut group = c.benchmark_group("core/list/notes");
    for limit in &[30usize, 60, 100, 500, usize::MAX] {
        let label = if *limit == usize::MAX {
            "max".to_string()
        } else {
            limit.to_string()
        };
        group.bench_with_input(BenchmarkId::from_parameter(&label), limit, |b, &limit| {
            b.iter(|| {
                rt.block_on(async {
                    let _ = store.list(None, limit, 0).await.unwrap();
                })
            });
        });
    }
    group.finish();

    // Daily-filtered list — the JournalView's actual query shape.
    let mut group = c.benchmark_group("core/list/tag_daily");
    for limit in &[30usize, 60, 200] {
        group.bench_with_input(
            BenchmarkId::from_parameter(limit),
            limit,
            |b, &limit| {
                b.iter(|| {
                    rt.block_on(async {
                        let _ = store.list(Some("daily"), limit, 0).await.unwrap();
                    })
                });
            },
        );
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(20);
    targets = list_at_varied_limits
}
criterion_main!(benches);
