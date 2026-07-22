//! LinkGraph trait for abstracting link/backlink storage

use async_trait::async_trait;

use crate::error::Result;
use crate::link::{GraphEdge, Link, RelationBacklink, RelationEdge};
use crate::note::{NoteId, PageId};

#[async_trait]
pub trait LinkGraph: Send + Sync {
    async fn get_backlinks(&self, id: &NoteId) -> Result<Vec<Link>>;
    async fn get_forward_links(&self, id: &NoteId) -> Result<Vec<Link>>;
    async fn get_all_edges(&self) -> Result<Vec<GraphEdge>>;
    async fn update_links(&self, id: &NoteId, links: &[Link]) -> Result<()>;
    async fn remove_links(&self, id: &NoteId) -> Result<()>;

    async fn upsert_relation_edge(&self, _edge: &RelationEdge) -> Result<()> {
        Ok(())
    }

    async fn remove_relation_edge(
        &self,
        _source_page_id: PageId,
        _source_block_id: Option<&str>,
        _property_key: &str,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_relation_backlinks(&self, _target: PageId) -> Result<Vec<RelationBacklink>> {
        Ok(Vec::new())
    }

    /// Remove every rebuildable relation edge emitted by one source note.
    async fn remove_relation_edges_for_note(&self, _source_note_id: &str) -> Result<()> {
        Ok(())
    }
}
