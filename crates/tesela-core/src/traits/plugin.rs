//! Plugin trait for Tesela extensions (Phase 6 expands this)

use crate::error::Result;
use crate::note::{Note, NoteId};

pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    fn version(&self) -> &str;
    fn on_note_created(&self, _note: &Note) -> Result<()> {
        Ok(())
    }
    fn on_note_updated(&self, _note: &Note) -> Result<()> {
        Ok(())
    }
    fn on_note_deleted(&self, _id: &NoteId) -> Result<()> {
        Ok(())
    }
}
