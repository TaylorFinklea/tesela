//! Shared NoteUpsert hydration and the Logseq import writer.

use tesela_core::import_logseq::{ImportNoteWrite, ImportNoteWriter};

use crate::{ContentHash, OpPayload, SyncEngine, SyncResult};

fn hydration_payload(note_id: [u8; 16], slug: &str, content: &str) -> OpPayload {
    let title = tesela_core::storage::markdown::parse_frontmatter(content)
        .ok()
        .and_then(|(metadata, _)| metadata.title)
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| slug.to_string());
    OpPayload::NoteUpsert {
        note_id,
        display_alias: Some(slug.to_string()),
        title,
        content: content.to_string(),
        created_at_millis: 0,
    }
}

/// Hydrate one materialized note body into the addressed engine document.
pub async fn hydrate_note(
    engine: &dyn SyncEngine,
    note_id: [u8; 16],
    slug: &str,
    content: &str,
) -> SyncResult<ContentHash> {
    engine
        .record_local(hydration_payload(note_id, slug, content))
        .await
}

/// Import writer that makes the sync engine the sole note-file writer.
pub struct EngineImportNoteWriter<'a> {
    engine: &'a dyn SyncEngine,
}

impl<'a> EngineImportNoteWriter<'a> {
    /// Borrow an engine for the duration of one import apply.
    pub fn new(engine: &'a dyn SyncEngine) -> Self {
        Self { engine }
    }
}

#[async_trait::async_trait]
impl ImportNoteWriter for EngineImportNoteWriter<'_> {
    async fn write_note(
        &mut self,
        target_id: &str,
        _target_path: &std::path::Path,
        content: &str,
    ) -> anyhow::Result<()> {
        hydrate_note(
            self.engine,
            tesela_core::stable_uuid_from_slug(target_id),
            target_id,
            content,
        )
        .await?;
        Ok(())
    }

    async fn write_notes(&mut self, writes: &[ImportNoteWrite]) -> Vec<anyhow::Result<()>> {
        let payloads = writes
            .iter()
            .map(|write| {
                hydration_payload(
                    tesela_core::stable_uuid_from_slug(&write.target_id),
                    &write.target_id,
                    &write.content,
                )
            })
            .collect();
        self.engine
            .record_local_batch(payloads)
            .await
            .into_iter()
            .map(|result| result.map(|_| ()).map_err(anyhow::Error::new))
            .collect()
    }
}
