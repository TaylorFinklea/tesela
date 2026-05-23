use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use tesela_core::query::AgendaRow;
use tesela_core::traits::search_index::SearchIndex;

use crate::{error::AppResult, state::AppState};

#[derive(Debug, Deserialize)]
pub struct AgendaQuery {
    pub from: String,
    pub to: String,
    #[serde(default)]
    pub include_done: bool,
}

/// POST /agenda — return agenda rows in [from, to] (inclusive, YYYY-MM-DD).
pub async fn post_agenda(
    State(s): State<Arc<AppState>>,
    Json(body): Json<AgendaQuery>,
) -> AppResult<Json<Vec<AgendaRow>>> {
    let rows = s
        .index
        .agenda_blocks(&body.from, &body.to, body.include_done)
        .await?;
    Ok(Json(rows))
}

#[cfg(test)]
mod tests {
    use tesela_core::{db::SqliteIndex, note::NoteMetadata, note::NoteId, Note};
    use tesela_core::traits::search_index::SearchIndex as _;
    use chrono::Utc;
    use std::path::PathBuf;

    fn make_test_note(id: &str, title: &str, body: &str) -> Note {
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
            path: PathBuf::from(format!("notes/{}.md", id)),
            checksum: format!("checksum-{}", id),
            created_at: Utc::now(),
            modified_at: Utc::now(),
            attachments: vec![],
        }
    }

    #[tokio::test]
    async fn post_agenda_returns_rows_in_window() {
        let index = SqliteIndex::open_in_memory().await.unwrap();

        // Seed: a recurring weekly task `weekly review`, scheduled 2026-05-22,
        // recurring:: weekly, tag:Task, status:: todo
        let note = make_test_note(
            "agenda-weekly",
            "Weekly Review Note",
            "- weekly review\n  scheduled:: 2026-05-22\n  recurring:: weekly\n  tags:: Task\n  status:: todo",
        );
        index.reindex(&note).await.unwrap();

        let rows = index
            .agenda_blocks("2026-05-22", "2026-06-12", false)
            .await
            .unwrap();

        // Weekly anchor 2026-05-22 + 3 projections (05-29, 06-05, 06-12) = 4
        assert_eq!(rows.len(), 4, "expected 4 rows: got {rows:?}");
        assert_eq!(rows[0].occurrence_date, "2026-05-22");
        assert!(rows[0].is_anchor);
    }
}
