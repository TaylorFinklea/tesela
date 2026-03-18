//! SearchIndex trait for abstracting search backends

use async_trait::async_trait;

use crate::error::Result;
use crate::note::{Note, NoteId, SearchHit};

#[async_trait]
pub trait SearchIndex: Send + Sync {
    async fn search(&self, query: &str, limit: usize, offset: usize) -> Result<Vec<SearchHit>>;
    async fn suggest(&self, partial: &str) -> Result<Vec<String>>;
    async fn reindex(&self, note: &Note) -> Result<()>;
    async fn remove(&self, id: &NoteId) -> Result<()>;
    async fn rebuild(&self) -> Result<usize>;
}
