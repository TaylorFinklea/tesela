//! Sanity-check the shape of generated mosaics — note count matches
//! config, files actually land on disk, frontmatter is parseable, etc.

use std::fs;
use tesela_fixtures::{tiny, MosaicBuilder};

#[test]
fn tiny_preset_produces_expected_layout() {
    let m = tiny().build().unwrap();
    // Notes dir exists + has the expected mix of files.
    let notes_dir = m.path.join("notes");
    assert!(notes_dir.exists(), "notes/ should exist");
    let count = fs::read_dir(&notes_dir).unwrap().count();
    // 20 dailies + 8 pages + 9 system widgets + 5 built-in type pages = 42.
    assert!(
        count >= 38 && count <= 45,
        "expected ~42 entries in notes/, got {}",
        count
    );
    // System widgets seeded.
    assert!(notes_dir.join("dailies.md").exists());
    assert!(notes_dir.join("tasks.md").exists());
    // Config + attachments dirs exist.
    assert!(m.path.join(".tesela").join("config.toml").exists());
    assert!(m.path.join("attachments").exists());
}

#[test]
fn explicit_counts_are_honored() {
    let m = MosaicBuilder::new()
        .seed(7)
        .daily_notes(50)
        .pages(15)
        .tasks(0) // disable tasks for a clean count
        .build()
        .unwrap();
    assert_eq!(m.stats.daily_notes, 50);
    assert_eq!(m.stats.pages, 15);
    assert_eq!(m.stats.notes, 65);
    assert_eq!(m.stats.tasks, 0);
    // Every daily file should land on disk with parseable frontmatter.
    for entry in fs::read_dir(m.path.join("notes")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.contains('-') {
            continue;
        }
        let body = fs::read_to_string(&path).unwrap();
        assert!(body.starts_with("---\n"), "{} missing frontmatter", name);
        assert!(body.contains("title: "), "{} missing title", name);
    }
}

#[test]
fn task_density_is_respected() {
    let m = MosaicBuilder::new()
        .seed(13)
        .daily_notes(40)
        .pages(0)
        .tasks(15)
        .build()
        .unwrap();
    // Generated task count should match the budget (or slightly fewer
    // if the random draw didn't surface 15 task opportunities — but
    // with 40 dailies at 20% task chance, 15 is achievable).
    assert!(
        m.stats.tasks <= 15 && m.stats.tasks >= 5,
        "expected tasks between 5..=15, got {}",
        m.stats.tasks
    );
}

#[test]
fn backlinks_are_realistic() {
    let m = MosaicBuilder::new()
        .seed(11)
        .daily_notes(30)
        .pages(10)
        .backlinks_per_note(2, 4)
        .build()
        .unwrap();
    // With 40 notes × ~3 explicit backlinks each = 120+ links, plus
    // the inline ones block_line adds randomly.
    assert!(
        m.stats.links >= 40,
        "expected ≥40 wikilinks across the mosaic, got {}",
        m.stats.links
    );
}
