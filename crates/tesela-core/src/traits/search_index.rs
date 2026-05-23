//! SearchIndex trait for abstracting search backends

use async_trait::async_trait;

use crate::error::Result;
use crate::note::{Note, NoteId, NoteVersion, SearchHit};
use crate::query::{AgendaRow, CalendarMarks, ParsedQuery, QueryResult};

#[async_trait]
pub trait SearchIndex: Send + Sync {
    async fn search(&self, query: &str, limit: usize, offset: usize) -> Result<Vec<SearchHit>>;
    async fn suggest(&self, partial: &str) -> Result<Vec<String>>;
    async fn reindex(&self, note: &Note) -> Result<()>;
    async fn remove(&self, id: &NoteId) -> Result<()>;
    async fn rebuild(&self) -> Result<usize>;

    /// Execute a parsed [`ParsedQuery`] and return [`QueryResult`] grouped/sorted.
    /// `group` is a property/metadata key (or one of `"status"`, `"priority"`,
    /// etc.) — when `None` the result has a single ungrouped bucket. `sort` is
    /// a comma-separated `key [asc|desc]` list applied within each group.
    async fn execute_query(
        &self,
        query: &ParsedQuery,
        group: Option<&str>,
        sort: Option<&str>,
    ) -> Result<QueryResult>;

    /// Compute calendar markers for the rail's mini calendar (Phase 9.2).
    /// `from` and `to` are inclusive `YYYY-MM-DD` boundaries (typically the
    /// first and last day of the visible month). Implementations should be
    /// cheap — drives the calendar widget's per-day dot rendering.
    async fn calendar_marks(&self, from: &str, to: &str) -> Result<CalendarMarks>;

    /// Return the agenda rows in [from, to] (inclusive both ends, `YYYY-MM-DD`).
    /// Expands recurring blocks via `recurrence::advance` so each projected
    /// future occurrence within the window is its own row. Done tasks are
    /// excluded unless `include_done` is true. Sorted by
    /// (occurrence_date, occurrence_time, block_id).
    async fn agenda_blocks(
        &self,
        from: &str,
        to: &str,
        include_done: bool,
    ) -> Result<Vec<AgendaRow>>;

    /// Append a new version row for the given note (Phase 9.3). `prev_content`
    /// is the pre-PUT content (or `None` for the very first version). The
    /// implementation assigns the next version_number and prunes oldest beyond
    /// `cap`.
    async fn record_version(
        &self,
        note_id: &NoteId,
        prev_content: Option<&str>,
        new_content: &str,
        cap: usize,
    ) -> Result<i64>;

    /// List versions for a note, newest first. `limit` caps the number returned.
    async fn list_versions(&self, note_id: &NoteId, limit: usize) -> Result<Vec<NoteVersion>>;

    /// Fetch a single version by its primary-key id.
    async fn get_version(&self, version_id: i64) -> Result<Option<NoteVersion>>;
}
