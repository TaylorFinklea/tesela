//! tesela-ya4.1 — decides whether `SqliteIndex::get_typed_blocks` is a safe
//! drop-in for the generalized kanban block source (spec decision 2: "Keep
//! `get_typed_blocks` as an internal optimization ONLY if it returns
//! byte-identical membership to `executeQuery("tag:X kind:block")`;
//! otherwise retire the divergent path").
//!
//! It is NOT equivalent: `get_typed_blocks`'s membership check is
//! `block.tags` only (`db/sqlite.rs`), while `executeQuery`'s `tag:` filter
//! also matches `inherited_tags` (`query.rs::filter_matches`'s
//! `include_inherited` branch) — a block nested under a tagged parent is
//! included by one and excluded by the other. This test file is the
//! evidence for that call: KanbanBoard.svelte (this bead) switches to
//! `executeQuery` as its sole block source instead of `getTypedBlocks`.
//! `get_typed_blocks` itself is left in place — TagTable / backlinks-of-tag /
//! instances-of-tag / TagPageRenderer still call it and are out of this
//! bead's scope (data-layer-only for kanban).

use tesela_core::note::{Note, NoteId, NoteMetadata};
use tesela_core::query::parse_query;
use tesela_core::traits::search_index::SearchIndex;
use tesela_core::SqliteIndex;

fn make_note(id: &str, title: &str, body: &str) -> Note {
    Note {
        id: NoteId::new(id),
        title: title.to_string(),
        content: format!("# {}\n\n{}", title, body),
        body: body.to_string(),
        metadata: NoteMetadata {
            title: None,
            tags: vec![],
            aliases: vec![],
            note_type: None,
            custom: Default::default(),
            created: None,
            modified: None,
        },
        path: std::path::PathBuf::from(format!("notes/{id}.md")),
        checksum: format!("checksum-{id}"),
        created_at: chrono::Utc::now(),
        modified_at: chrono::Utc::now(),
        attachments: vec![],
    }
}

/// A block nested under a `#Task`-tagged parent inherits the tag for
/// `executeQuery`'s `tag:` predicate but not for `get_typed_blocks`'s
/// membership check — the two sources disagree on which blocks belong to
/// the type.
#[tokio::test]
async fn get_typed_blocks_diverges_from_execute_query_on_inherited_tags() {
    let index = SqliteIndex::open_in_memory().await.unwrap();
    let note = make_note(
        "equiv-1",
        "Equivalence Note",
        "- Parent task #Task\n  - Child subtask\n    status:: todo",
    );
    index.reindex(&note).await.unwrap();

    let typed = index.get_typed_blocks("Task").await.unwrap();
    let typed_texts: Vec<&str> = typed.iter().map(|b| b.text.as_str()).collect();

    let parsed = parse_query("tag:Task kind:block");
    let result = index.execute_query(&parsed, None, None).await.unwrap();
    let query_texts: Vec<&str> = result
        .groups
        .iter()
        .flat_map(|g| g.items.iter())
        .map(|i| i.text.as_str())
        .collect();

    assert_eq!(
        typed_texts,
        vec!["Parent task"],
        "get_typed_blocks should only see the directly-tagged block: {typed_texts:?}"
    );
    assert!(
        query_texts.contains(&"Child subtask"),
        "executeQuery(\"tag:Task kind:block\") should include the inherited-tag \
         child — if it doesn't, the divergence documented here has closed and \
         this test (and the ya4.1 design_notes call) needs to be revisited: \
         {query_texts:?}"
    );
    assert_ne!(
        typed_texts.len(),
        query_texts.len(),
        "get_typed_blocks and executeQuery(\"tag:X kind:block\") are NOT \
         byte-equivalent in membership (inherited-tag blocks) — kanban must \
         not rely on get_typed_blocks as its block source (spec decision 2)"
    );
}

/// The flat (no nesting/inheritance) case IS membership- and property-
/// equivalent between the two sources — documents the common case
/// alongside the divergent one above so the divergence isn't overstated.
#[tokio::test]
async fn get_typed_blocks_matches_execute_query_in_the_flat_case() {
    let index = SqliteIndex::open_in_memory().await.unwrap();
    let note = make_note(
        "equiv-2",
        "Flat Note",
        "- Alpha task #Task\n  status:: todo\n- Beta task #Task\n  status:: doing",
    );
    index.reindex(&note).await.unwrap();

    let typed = index.get_typed_blocks("Task").await.unwrap();
    let mut typed_rows: Vec<(String, Option<String>)> = typed
        .iter()
        .map(|b| (b.text.clone(), b.properties.get("status").cloned()))
        .collect();
    typed_rows.sort();

    let parsed = parse_query("tag:Task kind:block");
    let result = index.execute_query(&parsed, None, None).await.unwrap();
    let mut query_rows: Vec<(String, Option<String>)> = result
        .groups
        .iter()
        .flat_map(|g| g.items.iter())
        .map(|i| (i.text.clone(), i.properties.get("status").cloned()))
        .collect();
    query_rows.sort();

    assert_eq!(
        typed_rows, query_rows,
        "flat (non-nested) tagged blocks should agree on membership + properties"
    );
}
