//! App-level regression for the journal data path: a real on-disk
//! `YYYY-MM-DD.md` note WITHOUT the `daily` tag in its frontmatter must
//! still be returned by the daily filter, not silently dropped or
//! replaced by an empty synthetic gap day.
//!
//! The journal feeds off `GET /notes?tag=daily` (the same
//! `FsNoteStore::list` call the route handler makes). The
//! `matches_tag_filter` predicate in `tesela-core::storage::filesystem`
//! already accepts a date-slug id under the `daily` tag (covered by the
//! unit test
//! `storage::filesystem::tests::test_daily_filter_includes_date_slug_notes_without_daily_tag`);
//! this end-to-end test exercises the SAME predicate through the real
//! `tesela-server` binary so a future server-side filter or
//! materialization layer can't quietly regress the journal path.
//!
//! Fixture shape:
//! - `2026-06-10.md` — canonical date slug, body blocks, NO `daily` tag.
//! - `2026-06-11.md` — canonical date slug, body blocks, WITH `tags: [daily]`.
//! - `2026-06-12.md` — canonical date slug, body blocks, but a DIFFERENT
//!   tag (`personal`); the date slug alone must include it under
//!   `?tag=daily`.
//! - `regular-note.md` — not a date slug, MUST be excluded.
//!
//! Skipped on non-Unix (spawns the server binary, SIGTERMs to shut down).

#![cfg(unix)]

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use tempfile::TempDir;

#[path = "common/mod.rs"]
mod common;
use common::ServerGuard;

fn make_fixture_mosaic(root: &Path) -> std::io::Result<()> {
    fs::create_dir_all(root.join("notes"))?;
    fs::create_dir_all(root.join("attachments"))?;
    fs::create_dir_all(root.join(".tesela"))?;
    fs::write(
        root.join(".tesela/config.toml"),
        "[backup]\nauto_on_quit = false\n",
    )?;
    // Date-slug daily — NO `daily` tag in frontmatter. The body blocks
    // are the entire reason the journal would render this day.
    fs::write(
        root.join("notes/2026-06-10.md"),
        "---\n\
         title: 2026-06-10\n\
         ---\n\
         \n\
         - visible journal block from a date-slug without the daily tag\n\
         - second block, still no daily tag\n\
         ",
    )?;
    // Control: same date-slug shape, but with the `daily` tag set.
    fs::write(
        root.join("notes/2026-06-11.md"),
        "---\n\
         title: 2026-06-11\n\
         tags: [daily]\n\
         ---\n\
         \n\
         - tagged daily block\n\
         ",
    )?;
    // Date-slug that should STILL be included under the daily filter
    // even though it carries a different tag — proves the date-slug
    // inclusion is a fallback, not "tag matches exactly".
    fs::write(
        root.join("notes/2026-06-12.md"),
        "---\n\
         title: 2026-06-12\n\
         tags: [personal]\n\
         ---\n\
         \n\
         - personal-titled day, but still a canonical date slug\n\
         ",
    )?;
    // Non-date-slug note — must NOT show up in the daily filter.
    fs::write(
        root.join("notes/regular-note.md"),
        "---\n\
         title: regular-note\n\
         ---\n\
         \n\
         - not a daily, no date slug\n\
         ",
    )?;
    Ok(())
}

fn spawn_server_child(mosaic: &Path, addr: &str) -> Child {
    Command::new(common::binary_path())
        .current_dir(mosaic)
        .env("TESELA_SERVER_BIND", addr)
        .env("TESELA_DISABLE_MDNS", "1")
        .env("TESELA_DISABLE_PEER_SYNC", "1")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn tesela-server")
}

struct Harness {
    base: String,
    _server: ServerGuard,
    _temp: TempDir,
}

fn boot() -> Harness {
    let temp = TempDir::new().unwrap();
    let mosaic = temp.path().join("mosaic");
    make_fixture_mosaic(&mosaic).unwrap();

    let (child, _addr, base) = common::spawn_with_retry(Duration::from_secs(15), |addr| {
        spawn_server_child(&mosaic, addr)
    });
    Harness {
        base,
        _server: ServerGuard(Some(child)),
        _temp: temp,
    }
}

async fn list_with_tag(client: &reqwest::Client, base: &str, tag: &str) -> Vec<serde_json::Value> {
    let url = format!("{base}/notes?tag={tag}&limit=100");
    client
        .get(&url)
        .send()
        .await
        .expect("GET /notes?tag=…")
        .error_for_status()
        .expect("GET /notes ok")
        .json::<Vec<serde_json::Value>>()
        .await
        .expect("notes json")
}

/// A canonical `YYYY-MM-DD.md` daily without the `daily` tag in its
/// frontmatter must still come through the journal data path
/// (`GET /notes?tag=daily`) with its body blocks intact — proving the
/// date-slug inclusion in `FsNoteStore::matches_tag_filter` reaches the
/// end-to-end HTTP surface unchanged, and the journal view won't fall
/// back to a synthetic empty gap day for it.
#[tokio::test(flavor = "current_thread")]
async fn daily_filter_includes_date_slug_daily_without_tag() {
    let h = boot();
    let client = reqwest::Client::new();

    let notes = list_with_tag(&client, &h.base, "daily").await;
    let ids: Vec<&str> = notes.iter().filter_map(|n| n["id"].as_str()).collect();

    // The fixture has FOUR .md files. The daily filter must return the
    // three date-slug notes (with or without the explicit `daily` tag)
    // and must NOT return `regular-note`.
    assert!(
        ids.contains(&"2026-06-10"),
        "date-slug note WITHOUT `daily` tag must come through the daily \
         filter; got ids: {ids:?}"
    );
    assert!(
        ids.contains(&"2026-06-11"),
        "date-slug note WITH `tags: [daily]` must come through; got ids: {ids:?}"
    );
    assert!(
        ids.contains(&"2026-06-12"),
        "date-slug note with a DIFFERENT tag must still come through the \
         daily filter; got ids: {ids:?}"
    );
    assert!(
        !ids.contains(&"regular-note"),
        "non-date-slug note must NOT come through the daily filter; got ids: {ids:?}"
    );

    // Body-content fidelity: the date-slug note with no `daily` tag
    // must carry its actual body blocks through the route, so the
    // journal renders real content for that day (not an empty gap).
    let slug10 = notes
        .iter()
        .find(|n| n["id"].as_str() == Some("2026-06-10"))
        .expect("2026-06-10 in response");
    let body10 = slug10["body"].as_str().unwrap_or_default();
    let content10 = slug10["content"].as_str().unwrap_or_default();
    assert!(
        body10.contains("visible journal block from a date-slug without the daily tag")
            && body10.contains("second block, still no daily tag"),
        "date-slug body blocks must survive end-to-end; got body: {body10:?}"
    );
    assert!(
        content10.contains("visible journal block from a date-slug without the daily tag"),
        "date-slug full content must include the body blocks; got content: {content10:?}"
    );
    // No `daily` tag in the frontmatter of the fixture — the test
    // passes only if the date slug alone is enough.
    let meta_tags = slug10["metadata"]["tags"]
        .as_array()
        .map(|a| a.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>())
        .unwrap_or_default();
    assert!(
        !meta_tags.contains(&"daily"),
        "fixture invariant: 2026-06-10.md MUST lack the `daily` tag in \
         frontmatter; got metadata.tags: {meta_tags:?}"
    );

    // Sanity check: a non-daily tag filter excludes the date-slug
    // notes. This proves the daily-filter inclusion is tag-aware, not
    // a blanket "return everything".
    let personal = list_with_tag(&client, &h.base, "personal").await;
    let personal_ids: Vec<&str> = personal.iter().filter_map(|n| n["id"].as_str()).collect();
    assert_eq!(
        personal_ids,
        vec!["2026-06-12"],
        "the `personal` tag filter must return only the one note that \
         carries that tag, not the date-slug fallback"
    );
}
