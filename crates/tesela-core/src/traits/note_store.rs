//! NoteStore trait for abstracting note storage backends

use async_trait::async_trait;
use chrono::NaiveDate;
use std::path::Path;

use crate::daily::DailyNoteConfig;
use crate::error::Result;
use crate::note::{Note, NoteId};

#[async_trait]
pub trait NoteStore: Send + Sync {
    async fn get(&self, id: &NoteId) -> Result<Option<Note>>;
    async fn get_by_title(&self, title: &str) -> Result<Option<Note>>;
    async fn create(&self, title: &str, content: &str, tags: &[&str]) -> Result<Note>;
    async fn update(&self, note: &Note) -> Result<()>;
    async fn delete(&self, id: &NoteId) -> Result<()>;
    async fn list(
        &self,
        tag_filter: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Note>>;
    async fn daily_note(
        &self,
        date: Option<NaiveDate>,
        config: &DailyNoteConfig,
    ) -> Result<Note>;
    async fn mosaic_root(&self) -> &Path;
}
