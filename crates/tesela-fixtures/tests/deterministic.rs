//! Same seed + same builder config ⇒ byte-identical output. Critical
//! for benchmarks: a flaky fixture would mask real perf signal.

use std::fs;
use std::path::Path;
use tempfile::TempDir;
use tesela_fixtures::MosaicBuilder;

fn hash_dir(path: &Path) -> Vec<(String, u64)> {
    let mut out = Vec::new();
    for entry in walkdir_lite(path) {
        if entry.is_file() {
            let bytes = fs::read(&entry).unwrap();
            let mut h: u64 = 0xcbf29ce484222325;
            for b in &bytes {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100000001b3);
            }
            let rel = entry
                .strip_prefix(path)
                .unwrap_or(&entry)
                .to_string_lossy()
                .into_owned();
            out.push((rel, h));
        }
    }
    out.sort();
    out
}

fn walkdir_lite(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(p) = stack.pop() {
        if p.is_dir() {
            for entry in fs::read_dir(&p).unwrap().flatten() {
                stack.push(entry.path());
            }
        } else if p.is_file() {
            out.push(p);
        }
    }
    out
}

#[test]
fn same_seed_byte_identical() {
    let t1 = TempDir::new().unwrap();
    let t2 = TempDir::new().unwrap();

    MosaicBuilder::new()
        .seed(42)
        .daily_notes(20)
        .pages(5)
        .tasks(8)
        .build_at(t1.path())
        .unwrap();

    MosaicBuilder::new()
        .seed(42)
        .daily_notes(20)
        .pages(5)
        .tasks(8)
        .build_at(t2.path())
        .unwrap();

    let h1 = hash_dir(t1.path());
    let h2 = hash_dir(t2.path());
    assert_eq!(h1.len(), h2.len(), "file count differs");
    for ((p1, _), (p2, _)) in h1.iter().zip(h2.iter()) {
        assert_eq!(p1, p2, "paths differ");
    }
    // Today's date is part of every daily filename, so the
    // *.md filenames are stable. We hash only files that are not
    // .tesela/tesela.db (no db is created here anyway) and not
    // .tesela/config.toml (which contains no time fields and is
    // stable).
    for ((p1, _), (p2, _)) in h1.iter().zip(h2.iter()) {
        let h1 = h1.iter().find(|(p, _)| p == p1).unwrap().1;
        let h2 = h2.iter().find(|(p, _)| p == p2).unwrap().1;
        assert_eq!(h1, h2, "{} content differs across runs", p1);
    }
}

#[test]
fn different_seed_produces_different_output() {
    let t1 = TempDir::new().unwrap();
    let t2 = TempDir::new().unwrap();

    MosaicBuilder::new()
        .seed(1)
        .daily_notes(20)
        .pages(5)
        .build_at(t1.path())
        .unwrap();
    MosaicBuilder::new()
        .seed(2)
        .daily_notes(20)
        .pages(5)
        .build_at(t2.path())
        .unwrap();

    let h1 = hash_dir(t1.path());
    let h2 = hash_dir(t2.path());
    let h1_total: u64 = h1.iter().fold(0u64, |acc, (_, h)| acc.wrapping_add(*h));
    let h2_total: u64 = h2.iter().fold(0u64, |acc, (_, h)| acc.wrapping_add(*h));
    assert_ne!(h1_total, h2_total, "different seeds produced identical mosaics");
}
