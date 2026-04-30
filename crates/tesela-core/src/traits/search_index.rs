//! SearchIndex trait for abstracting search backends

use async_trait::async_trait;

use crate::error::Result;
use crate::note::{Note, NoteId, SearchHit};
use crate::query::{ParsedQuery, QueryResult};

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
}
