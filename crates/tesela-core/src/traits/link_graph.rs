//! LinkGraph trait for abstracting link/backlink storage

use async_trait::async_trait;

use crate::error::Result;
use crate::link::{GraphEdge, Link};
use crate::note::NoteId;

#[async_trait]
pub trait LinkGraph: Send + Sync {
    async fn get_backlinks(&self, id: &NoteId) -> Result<Vec<Link>>;
    async fn get_forward_links(&self, id: &NoteId) -> Result<Vec<Link>>;
    async fn get_all_edges(&self) -> Result<Vec<GraphEdge>>;
    async fn update_links(&self, id: &NoteId, links: &[Link]) -> Result<()>;
    async fn remove_links(&self, id: &NoteId) -> Result<()>;
}
