use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn tesela(dir: &TempDir) -> Command {
    let mut cmd = Command::cargo_bin("tesela").unwrap();
    cmd.arg("--mosaic").arg(dir.path());
    cmd
}

fn init_mosaic(dir: &TempDir) {
    Command::cargo_bin("tesela")
        .unwrap()
        .arg("init")
        .arg(dir.path())
        .assert()
        .success();
}

#[test]
fn test_init_creates_structure() {
    let tmp = TempDir::new().unwrap();
    Command::cargo_bin("tesela")
        .unwrap()
        .arg("init")
        .arg(tmp.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized mosaic"));

    assert!(tmp.path().join(".tesela").exists());
    assert!(tmp.path().join("notes").exists());
    assert!(tmp.path().join("attachments").exists());
    assert!(tmp.path().join(".tesela").join("config.toml").exists());
    assert!(tmp.path().join(".tesela").join("tesela.db").exists());
}

#[test]
fn test_new_and_list() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    // Create note
    tesela(&tmp)
        .args(["new", "My Test Note", "--tags", "test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("My Test Note"));

    // List shows it
    tesela(&tmp)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("My Test Note"));
}

#[test]
fn test_new_and_search() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["new", "Searchable Note", "--content", "unique-keyword-xyz"])
        .assert()
        .success();

    tesela(&tmp)
        .args(["search", "unique-keyword-xyz"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Searchable Note"));
}

#[test]
fn test_cat_by_title() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["new", "Cat Test", "--content", "hello world content"])
        .assert()
        .success();

    tesela(&tmp)
        .args(["cat", "Cat Test"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello world content"));
}

#[test]
fn test_list_after_init_shows_only_widgets() {
    // `tesela init` seeds 9 system Query widget pages (Phase 13 follow-up:
    // see `tesela_core::system_widgets::seed`). So a freshly-init'd
    // mosaic isn't empty — it has Dailies / Pages / Tasks / Projects /
    // People / Inbox / Calendar / Recent / Pinned.
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("dailies"))
        .stdout(predicate::str::contains("tasks"))
        .stdout(predicate::str::contains("pages"));
}

#[test]
fn test_list_with_tag_filter() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["new", "Alpha Note", "--tags", "alpha"])
        .assert()
        .success();

    tesela(&tmp)
        .args(["new", "Beta Note", "--tags", "beta"])
        .assert()
        .success();

    // Filter by alpha tag
    tesela(&tmp)
        .args(["list", "--tag", "alpha"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Alpha Note"))
        .stdout(predicate::str::contains("Beta Note").not());
}

#[test]
fn test_daily_creates_note() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["daily", "2026-03-18"])
        .assert()
        .success()
        .stdout(predicate::str::contains("2026-03-18"));
}

#[test]
fn test_export_markdown() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["new", "Export Test", "--content", "export body"])
        .assert()
        .success();

    tesela(&tmp)
        .args(["export-note", "Export Test", "--format", "markdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Export Test"));
}

#[test]
fn test_export_mosaic_full() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["new", "FullModeNote", "--content", "hello world"])
        .assert()
        .success();

    let out = tmp.path().join("export-out");
    tesela(&tmp)
        .args(["export", out.to_str().unwrap(), "--mode", "full"])
        .assert()
        .success()
        .stdout(predicate::str::contains("full mode"));

    // The exported notes/ directory should contain the note we just created.
    let mut found = false;
    for entry in std::fs::read_dir(out.join("notes")).unwrap() {
        let entry = entry.unwrap();
        let body = std::fs::read_to_string(entry.path()).unwrap();
        if body.contains("hello world") {
            found = true;
        }
    }
    assert!(found, "expected exported notes/ to contain the new note");
}

#[test]
fn test_reindex() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["new", "Reindex Note"])
        .assert()
        .success();

    tesela(&tmp)
        .arg("reindex")
        .assert()
        .success()
        .stdout(predicate::str::contains("Indexed"));
}

#[test]
fn test_completions() {
    Command::cargo_bin("tesela")
        .unwrap()
        .args(["completions", "bash"])
        .assert()
        .success();
}

/// Trust artifact: full end-to-end flow proving an imported Logseq
/// graph round-trips losslessly through the backup pipeline.
///
/// Logseq vault → `import-logseq` → mosaic → `backup` → wipe → `restore`
/// → byte-exact diff against the imported mosaic.
///
/// If this test ever fails, the user can't trust the system with real
/// notes — that's by design: it's the gate.
#[test]
fn logseq_import_backup_restore_byte_exact_round_trip() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("logseq-source");
    let mosaic = tmp.path().join("mosaic");
    let backups = tmp.path().join("backups");
    let restored_parent = tmp.path().join("restored-parent");

    // 1. Build a Logseq fixture vault that exercises the features
    //    found in the user's real graph.
    std::fs::create_dir_all(source.join("journals")).unwrap();
    std::fs::create_dir_all(source.join("pages")).unwrap();
    std::fs::create_dir_all(source.join("assets")).unwrap();
    std::fs::write(
        source.join("journals/2026_05_19.md"),
        "- TODO [#A] Write tests\n  SCHEDULED: <2026-05-20 Wed>\n- DONE Eat lunch\n",
    )
    .unwrap();
    std::fs::write(
        source.join("pages/Coverage.md"),
        "title:: Coverage\n- Hello [[Foo]] with #tag\n- ![img](../assets/diagram.png)\n- See ((675f6317-aaa6-4301-8ebb-df2b414dec4c))\n",
    )
    .unwrap();
    std::fs::write(source.join("pages/Foo.md"), "title:: Foo\n- regular page\n").unwrap();
    std::fs::write(
        source.join("pages/Parent___Child.md"),
        "- Nested namespace page\n",
    )
    .unwrap();
    std::fs::write(source.join("assets/diagram.png"), b"\x89PNG\r\nfake").unwrap();

    // 2. init mosaic + import the Logseq vault.
    init_mosaic(&TempDir::new_in(tmp.path()).unwrap()); // creates a dummy mosaic elsewhere to seed env state; ignore
    Command::cargo_bin("tesela")
        .unwrap()
        .arg("init")
        .arg(&mosaic)
        .assert()
        .success();
    Command::cargo_bin("tesela")
        .unwrap()
        .arg("--mosaic")
        .arg(&mosaic)
        .arg("import-logseq")
        .arg("--source")
        .arg(&source)
        .assert()
        .success();

    // 3. Backup (default local destination — `--output` outside the
    //    mosaic triggers auto-encryption which needs a Keychain
    //    identity; that path is round-tripped at the library level
    //    so the CLI test stays portable).
    let _ = &backups; // silence unused-var for the explicit-path variant.
    Command::cargo_bin("tesela")
        .unwrap()
        .arg("--mosaic")
        .arg(&mosaic)
        .arg("backup")
        .arg("--no-prune")
        .assert()
        .success();

    // 4. Locate the timestamped backup directory under the default
    //    in-mosaic location.
    let default_backups = mosaic.join(".tesela").join("backups");
    let backup_dir = std::fs::read_dir(&default_backups)
        .unwrap()
        .filter_map(|e| e.ok())
        .find(|e| {
            e.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && e.file_name().to_string_lossy().starts_with("backup-")
        })
        .expect("backup directory created")
        .path();

    // 5. Restore into a sibling location of the (still-intact) mosaic.
    //    The CLI restores adjacent to --mosaic when --in-place is not
    //    set, producing a directory like `<mosaic>.restored-<ts>`.
    std::fs::create_dir_all(&restored_parent).unwrap();
    Command::cargo_bin("tesela")
        .unwrap()
        .arg("--mosaic")
        .arg(restored_parent.join("dest"))
        .arg("restore")
        .arg(&backup_dir)
        .assert()
        .success();

    // Restore writes to `<mosaic-basename>-restored` next to the
    // mosaic path we passed (per `tesela_backup::restore` default).
    let restored = restored_parent.join("dest-restored");
    assert!(
        restored.exists(),
        "expected restore target at {}",
        restored.display()
    );

    // 6. Byte-exact diff across the captured set.
    let captured = |rel: &std::path::Path| -> bool {
        let s = rel.to_string_lossy();
        s.starts_with("notes/")
            || s.starts_with("attachments/")
            || s.starts_with("templates/")
            || s == ".tesela/config.toml"
    };
    let mut captured_files: Vec<std::path::PathBuf> = walkdir::WalkDir::new(&mosaic)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().strip_prefix(&mosaic).unwrap().to_path_buf())
        .filter(|rel| captured(rel))
        .collect();
    captured_files.sort();
    assert!(
        !captured_files.is_empty(),
        "fixture import should produce captured files"
    );

    for rel in &captured_files {
        let orig = std::fs::read(mosaic.join(rel))
            .unwrap_or_else(|e| panic!("read original {}: {}", rel.display(), e));
        let rest = std::fs::read(restored.join(rel)).unwrap_or_else(|e| {
            panic!(
                "restore missing {} (or unreadable): {}",
                rel.display(),
                e
            )
        });
        assert_eq!(orig, rest, "byte mismatch in {}", rel.display());
    }

    // Sanity check: the imported note bodies show the expected Logseq
    // conversions made it through both pipelines. Coverage.md holds
    // the link/asset/block-ref features; the journal holds tasks.
    let coverage = std::fs::read_to_string(restored.join("notes/coverage.md")).unwrap();
    assert!(coverage.contains("[[Foo]]"), "wikilink lost");
    assert!(
        coverage.contains("../attachments/"),
        "asset URL not rewritten\n{}",
        coverage
    );
    assert!(
        coverage.contains("((675f6317-"),
        "block ref uuid lost in round trip\n{}",
        coverage
    );

    let journal = std::fs::read_to_string(restored.join("notes/2026-05-19.md")).unwrap();
    assert!(journal.contains("status:: todo"), "TODO state lost\n{}", journal);
    assert!(journal.contains("status:: done"), "DONE state lost");
    assert!(journal.contains("priority:: high"), "priority [#A] lost");
    assert!(journal.contains("scheduled:: 2026-05-20"), "SCHEDULED date lost");
}

#[test]
fn test_cat_nonexistent() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .args(["cat", "nonexistent-note"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("Note not found"));
}
