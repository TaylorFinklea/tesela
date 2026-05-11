//! End-to-end backup + roundtrip-validation timing on a synthetic
//! mosaic. Catches regressions in manifest hashing, atomic-rename
//! costs, and the validation read-back loop.
//!
//! `cargo bench --bench backup -p tesela-backup`

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use tesela_backup::{backup, BackupOptions, Destination, GfsPolicy, ManifestEncryption};
use tesela_fixtures::MosaicBuilder;

fn backup_full_validate(c: &mut Criterion) {
    let mut group = c.benchmark_group("backup/full_validate");

    for (label, dailies, pages) in &[
        ("tiny_30", 20usize, 8usize),
        ("medium_500", 420usize, 80usize),
    ] {
        group.bench_with_input(BenchmarkId::from_parameter(label), label, |b, _| {
            // Rebuild the source mosaic per-batch so we measure only
            // the backup itself, not setup.
            b.iter_batched(
                || {
                    MosaicBuilder::new()
                        .seed(42)
                        .daily_notes(*dailies)
                        .pages(*pages)
                        .tasks(20)
                        .build()
                        .unwrap()
                },
                |mosaic| {
                    let outcome = backup(
                        &mosaic.path,
                        BackupOptions {
                            destination: Destination::Local,
                            validate: true,
                            extra_files: Vec::new(),
                            retention: Some(GfsPolicy::default()),
                            encryption: ManifestEncryption::None,
                        },
                    )
                    .expect("backup");
                    criterion::black_box(outcome);
                    drop(mosaic);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = backup_full_validate
}
criterion_main!(benches);
