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
fn test_list_empty() {
    let tmp = TempDir::new().unwrap();
    init_mosaic(&tmp);

    tesela(&tmp)
        .arg("list")
        .assert()
        .success()
        .stdout(predicate::str::contains("No notes found"));
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
        .args(["export", "Export Test", "--format", "markdown"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Export Test"));
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
